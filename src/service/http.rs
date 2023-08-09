use serde::Serialize;

use super::config::MobileServiceConfig;
use super::mobile_contract::{
    HandshakeResponse, HealthResponse, PrinterCapabilitiesResponse, ServiceIdentity,
};
use super::monitor_contract::MonitorResponse;
use crate::print::capabilities::manifest_for;
use crate::print::printer::PrinterKind;

#[derive(Clone, Debug, PartialEq)]
pub struct MobileHttpState {
    pub identity: ServiceIdentity,
    pub http_port: u16,
    pub candidate_ports: Vec<u16>,
    pub active_printer: PrinterKind,
}

impl MobileHttpState {
    pub fn new(
        identity: ServiceIdentity,
        http_port: u16,
        candidate_ports: Vec<u16>,
        active_printer: PrinterKind,
    ) -> Self {
        Self {
            identity,
            http_port,
            candidate_ports,
            active_printer,
        }
    }

    pub fn from_config(
        config: &MobileServiceConfig,
        identity: ServiceIdentity,
        active_printer: PrinterKind,
    ) -> Self {
        Self::new(
            identity,
            config.http_port(),
            config.candidate_ports.clone(),
            active_printer,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileHttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl MobileHttpResponse {
    pub fn json<T: Serialize>(status: u16, value: &T) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: serde_json::to_vec(value)
                .unwrap_or_else(|_| b"{\"error\":\"json_encode\"}".to_vec()),
        }
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileHttpErrorResponse {
    pub error: &'static str,
}

impl Serialize for MobileHttpErrorResponse {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("MobileHttpErrorResponse", 1)?;
        state.serialize_field("error", self.error)?;
        state.end()
    }
}

pub fn handle_mobile_http_request(
    state: &MobileHttpState,
    method: &str,
    path: &str,
) -> MobileHttpResponse {
    let method = method.trim().to_ascii_uppercase();
    let path = normalize_path(path);

    match (method.as_str(), path.as_str()) {
        ("GET", "/healthz") => MobileHttpResponse::json(200, &HealthResponse::ok()),
        ("GET", "/v1/mobile/handshake") => {
            let handshake = HandshakeResponse::new(
                &state.identity,
                state.http_port,
                state.candidate_ports.clone(),
            );
            MobileHttpResponse::json(200, &handshake)
        }
        ("GET", "/v1/mobile/printer/capabilities") => {
            let response =
                PrinterCapabilitiesResponse::from_manifest(manifest_for(state.active_printer));
            MobileHttpResponse::json(200, &response)
        }
        ("GET", "/v1/mobile/monitor/state") => {
            let response = MonitorResponse::driver_idle(&state.identity, state.active_printer);
            MobileHttpResponse::json(200, &response)
        }
        (_, "/healthz")
        | (_, "/v1/mobile/handshake")
        | (_, "/v1/mobile/printer/capabilities")
        | (_, "/v1/mobile/monitor/state") => MobileHttpResponse::json(
            405,
            &MobileHttpErrorResponse {
                error: "method_not_allowed",
            },
        ),
        _ => MobileHttpResponse::json(404, &MobileHttpErrorResponse { error: "not_found" }),
    }
}

fn normalize_path(path: &str) -> String {
    let path = path.trim();
    let path = path.split_once('?').map(|(path, _)| path).unwrap_or(path);
    match path {
        "" => "/".to_string(),
        value if value.starts_with('/') => value.to_string(),
        value => format!("/{value}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn state(printer: PrinterKind) -> MobileHttpState {
        MobileHttpState::new(
            ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin"),
            39117,
            vec![39117, 41257],
            printer,
        )
    }

    fn json(response: MobileHttpResponse) -> Value {
        serde_json::from_slice(&response.body).unwrap()
    }

    #[test]
    fn healthz_matches_gscale_mobile_fallback_contract() {
        let response = handle_mobile_http_request(&state(PrinterKind::Zebra), "GET", "/healthz");
        let body = json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "application/json");
        assert_eq!(body["ok"], true);
        assert_eq!(body["service"], "mobileapi");
    }

    #[test]
    fn handshake_matches_mobile_discovery_contract() {
        let body = json(handle_mobile_http_request(
            &state(PrinterKind::Zebra),
            "GET",
            "/v1/mobile/handshake",
        ));

        assert_eq!(body["service"], "mobileapi");
        assert_eq!(body["app"], "gscale-zebra");
        assert_eq!(body["server_name"], "rp-scale");
        assert_eq!(body["http_port"], 39117);
        assert_eq!(body["discovery_port"], 18081);
        assert_eq!(body["candidate_ports"][1], 41257);
        assert_eq!(body["requires_auth"], false);
    }

    #[test]
    fn printer_capabilities_expose_active_printer_limits() {
        let body = json(handle_mobile_http_request(
            &state(PrinterKind::Godex),
            "GET",
            "/v1/mobile/printer/capabilities",
        ));

        assert_eq!(body["active_printer"]["id"], "godex");
        assert_eq!(
            body["active_printer"]["capabilities"]["rfid_epc_write"],
            false
        );
        assert_eq!(body["active_printer"]["capabilities"]["qr"], true);
        assert_eq!(
            body["active_printer"]["unsupported_modes"][0],
            "rfid_epc_write"
        );
    }

    #[test]
    fn monitor_state_matches_mobile_snapshot_shape() {
        let body = json(handle_mobile_http_request(
            &state(PrinterKind::Godex),
            "GET",
            "/v1/mobile/monitor/state",
        ));

        assert_eq!(body["ok"], true);
        assert_eq!(body["profile"]["ref"], "dev-operator");
        assert_eq!(body["state"]["scale"]["weight"], Value::Null);
        assert_eq!(body["state"]["scale"]["unit"], "kg");
        assert_eq!(body["state"]["zebra"]["connected"], false);
        assert_eq!(body["state"]["printer"]["connected"], false);
        assert_eq!(body["state"]["printer"]["kind"], "godex");
        assert_eq!(body["state"]["batch"]["active"], false);
        assert_eq!(body["state"]["print_request"]["status"], "idle");
        assert_eq!(body["printer"]["label"], "ulanmagan");
    }

    #[test]
    fn rejects_wrong_methods_like_gscale_mobileapi() {
        let response = handle_mobile_http_request(&state(PrinterKind::Zebra), "POST", "/healthz");
        let body = json(response.clone());

        assert_eq!(response.status, 405);
        assert_eq!(body["error"], "method_not_allowed");
    }

    #[test]
    fn normalizes_query_string_before_routing() {
        let response = handle_mobile_http_request(
            &state(PrinterKind::Zebra),
            "GET",
            "/v1/mobile/handshake?x=1",
        );

        assert_eq!(response.status, 200);
    }
}
