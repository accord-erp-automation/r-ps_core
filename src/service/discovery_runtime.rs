use std::io;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use super::config::MobileServiceConfig;
use super::discovery::{
    DISCOVERY_ANNOUNCE_INTERVAL_MS, DiscoverySocketConfig, bind_announcement_socket,
    bind_discovery_socket, discovery_response_for_packet, send_discovery_announcement,
};
use super::mobile_contract::ServiceIdentity;

#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryRuntimeState {
    pub identity: ServiceIdentity,
    pub http_port: u16,
    pub candidate_ports: Vec<u16>,
}

impl DiscoveryRuntimeState {
    pub fn from_config(config: &MobileServiceConfig, identity: ServiceIdentity) -> Self {
        Self {
            identity,
            http_port: config.http_port(),
            candidate_ports: config.candidate_ports.clone(),
        }
    }
}

pub fn serve_discovery(
    config: DiscoverySocketConfig,
    state: DiscoveryRuntimeState,
) -> io::Result<()> {
    let socket = bind_discovery_socket(&config)?;
    let announce_socket = bind_announcement_socket()?;
    serve_discovery_socket(&socket, &announce_socket, &config, &state)
}

pub fn serve_discovery_socket(
    socket: &UdpSocket,
    announce_socket: &UdpSocket,
    config: &DiscoverySocketConfig,
    state: &DiscoveryRuntimeState,
) -> io::Result<()> {
    let mut last_announce = Instant::now()
        .checked_sub(Duration::from_millis(DISCOVERY_ANNOUNCE_INTERVAL_MS))
        .unwrap_or_else(Instant::now);
    let mut buf = [0_u8; 2048];

    loop {
        if last_announce.elapsed() >= Duration::from_millis(DISCOVERY_ANNOUNCE_INTERVAL_MS) {
            let _ = send_discovery_announcement(
                announce_socket,
                config,
                &state.identity,
                state.http_port,
                state.candidate_ports.clone(),
            );
            last_announce = Instant::now();
        }

        match socket.recv_from(&mut buf) {
            Ok((n, remote)) => {
                if let Some(response) = discovery_response_for_packet(
                    &buf[..n],
                    &state.identity,
                    state.http_port,
                    state.candidate_ports.clone(),
                ) {
                    let _ = socket.send_to(&response, remote);
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::discovery::DISCOVERY_PROBE_V1;
    use serde_json::Value;

    fn state() -> DiscoveryRuntimeState {
        DiscoveryRuntimeState {
            identity: ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin"),
            http_port: 39117,
            candidate_ports: vec![39117, 41257],
        }
    }

    #[test]
    fn runtime_state_uses_config_http_port_and_candidates() {
        let config =
            MobileServiceConfig::new("127.0.0.1", "127.0.0.1:41257", vec![41257], "rp-scale");
        let state = DiscoveryRuntimeState::from_config(
            &config,
            ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin"),
        );

        assert_eq!(state.http_port, 41257);
        assert_eq!(state.candidate_ports, vec![41257]);
    }

    #[test]
    fn probe_response_matches_runtime_state() {
        let response = discovery_response_for_packet(
            DISCOVERY_PROBE_V1.as_bytes(),
            &state().identity,
            state().http_port,
            state().candidate_ports,
        )
        .unwrap();
        let body: Value = serde_json::from_slice(&response).unwrap();

        assert_eq!(body["type"], "gscale_announce_v1");
        assert_eq!(body["service"], "mobileapi");
        assert_eq!(body["http_port"], 39117);
        assert_eq!(body["candidate_ports"][1], 41257);
    }
}
