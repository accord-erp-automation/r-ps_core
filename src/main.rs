use std::env;
use std::net::Ipv4Addr;
use std::process;
use std::sync::Arc;
use std::thread;

use rp_scale::print::PrinterKind;
use rp_scale::scale::{SerialReader, SerialReaderConfig};
use rp_scale::service::{
    DiscoveryRuntimeState, DiscoverySocketConfig, MobileHttpState, MobileServiceConfig,
    MonitorRuntimeState, PrintExecutorMode, ServiceIdentity, bind_mobile_http_listener,
    bonjour_config, collect_discovery_broadcast_targets, device_executor_from_env,
    parse_candidate_ports, print_executor_mode_from_env, register_bonjour_service,
    resolve_usblp_device_by_serial, serve_discovery, serve_mobile_http,
    simulated_executor_from_env,
};

fn main() {
    if env::args().nth(1).as_deref() == Some("serve") {
        if let Err(err) = serve() {
            eprintln!("rp-scale mobile service error: {err}");
            process::exit(1);
        }
        return;
    }

    println!("rp-scale: scale migration workspace");
    println!("run `rp-scale serve` to start the GScale-compatible mobile API");
}

fn serve() -> std::io::Result<()> {
    let active_printer = active_printer_from_env();
    let listen_addr = env::var("RP_SCALE_MOBILE_API_ADDR").unwrap_or_default();
    let candidate_ports = env::var("RP_SCALE_MOBILE_API_CANDIDATE_PORTS").unwrap_or_default();
    let server_name = env::var("RP_SCALE_SERVER_NAME").unwrap_or_else(|_| "rp-scale".to_string());
    let config =
        mobile_service_config_from_env("0.0.0.0", &listen_addr, &candidate_ports, &server_name);
    let server_ref =
        env::var("RP_SCALE_SERVER_REF").unwrap_or_else(|_| config.default_server_ref());
    let identity = ServiceIdentity::new(&config.server_name, &server_ref, "RP Scale", "operator");
    let zebra_device = printer_device_from_env("RP_SCALE_ZEBRA_DEVICE", "RP_SCALE_ZEBRA_SERIAL");
    let godex_device = printer_device_from_env("RP_SCALE_GODEX_DEVICE", "RP_SCALE_GODEX_SERIAL");
    let monitor =
        MonitorRuntimeState::with_printer_devices(zebra_device.clone(), godex_device.clone());
    start_scale_reader_from_env(monitor.clone());
    let mut http_state =
        MobileHttpState::from_config(&config, identity.clone(), active_printer, monitor);
    if let Ok(mode) = env::var("RP_SCALE_PRINT_EXECUTOR") {
        match print_executor_mode_from_env(&mode)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?
        {
            Some(PrintExecutorMode::Simulated) => {
                if let Some(executor) = simulated_executor_from_env(&mode) {
                    http_state = http_state.with_print_executor(Arc::new(executor));
                }
            }
            Some(PrintExecutorMode::Device) => {
                if let Some(executor) = device_executor_from_env(
                    &mode,
                    zebra_device.as_deref(),
                    godex_device.as_deref(),
                ) {
                    http_state = http_state.with_print_executor(Arc::new(executor));
                }
            }
            None => {}
        }
    }
    let discovery_state = DiscoveryRuntimeState::from_config(&config, identity.clone());
    let _bonjour = match register_bonjour_service(&bonjour_config(
        &identity,
        &config.server_name,
        config.http_port(),
    )) {
        Ok(service) => Some(service),
        Err(err) => {
            eprintln!("rp-scale bonjour warning: {err}");
            None
        }
    };

    let discovery_targets = collect_discovery_broadcast_targets(0);
    let discovery_config =
        DiscoverySocketConfig::with_socket_targets(Ipv4Addr::UNSPECIFIED, 0, discovery_targets);
    thread::spawn(move || {
        if let Err(err) = serve_discovery(discovery_config, discovery_state) {
            eprintln!("rp-scale discovery warning: {err}");
        }
    });

    let listener = bind_mobile_http_listener(&config.listen_addr)?;
    println!(
        "rp-scale mobile API listening on {} printer={}",
        config.listen_addr,
        active_printer.as_str()
    );
    serve_mobile_http(listener, http_state)
}

fn active_printer_from_env() -> PrinterKind {
    env::var("RP_SCALE_PRINTER")
        .ok()
        .and_then(|value| PrinterKind::normalize_request(&value))
        .unwrap_or(PrinterKind::Zebra)
}

fn printer_device_from_env(device_var: &str, serial_var: &str) -> Option<String> {
    let serial = env::var(serial_var).unwrap_or_default();
    if !serial.trim().is_empty() {
        let resolved = resolve_usblp_device_by_serial(&serial);
        if resolved.is_none() {
            eprintln!("{serial_var}={serial} did not match any GoDEX usblp device");
        }
        return resolved.map(|path| path.to_string_lossy().to_string());
    }
    env::var(device_var)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn mobile_service_config_from_env(
    listen_host: &str,
    explicit_listen_addr: &str,
    raw_candidate_ports: &str,
    server_name: &str,
) -> MobileServiceConfig {
    let candidate_ports = if raw_candidate_ports.trim().is_empty() {
        vec![]
    } else {
        parse_candidate_ports(raw_candidate_ports)
    };
    MobileServiceConfig::new(
        listen_host,
        explicit_listen_addr,
        candidate_ports,
        server_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_uses_candidate_ports_override() {
        let config = mobile_service_config_from_env("127.0.0.1", "", "41000, 41001", "rp-test");

        assert_eq!(config.candidate_ports, vec![41000, 41001]);
        assert_eq!(config.server_name, "rp-test");
    }
}

fn start_scale_reader_from_env(monitor: MonitorRuntimeState) {
    let Ok(device) = env::var("RP_SCALE_SCALE_DEVICE") else {
        return;
    };
    let device = device.trim().to_string();
    if device.is_empty() {
        return;
    }

    let baud = env::var("RP_SCALE_SCALE_BAUD")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(9600);
    let unit = env::var("RP_SCALE_SCALE_UNIT").unwrap_or_else(|_| "kg".to_string());

    thread::spawn(move || {
        let reader = SerialReader::new(SerialReaderConfig::new(&device, baud, &unit));
        reader.run_forever(|reading| monitor.record_reading(reading));
    });
}
