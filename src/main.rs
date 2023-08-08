use std::env;
use std::net::Ipv4Addr;
use std::process;
use std::thread;

use rp_scale::print::PrinterKind;
use rp_scale::service::{
    DiscoveryRuntimeState, DiscoverySocketConfig, MobileHttpState, MobileServiceConfig,
    ServiceIdentity, bind_mobile_http_listener, bonjour_config,
    collect_discovery_broadcast_targets, register_bonjour_service, serve_discovery,
    serve_mobile_http,
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
    let config = MobileServiceConfig::new(
        "0.0.0.0",
        &env::var("RP_SCALE_MOBILE_API_ADDR").unwrap_or_default(),
        vec![],
        &env::var("RP_SCALE_SERVER_NAME").unwrap_or_else(|_| "rp-scale".to_string()),
    );
    let identity = ServiceIdentity::new(&config.server_name, "rp-scale", "RP Scale", "operator");
    let http_state = MobileHttpState::from_config(&config, identity.clone(), active_printer);
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
