use serde::Serialize;

use super::config::{DEFAULT_DISCOVERY_PORT, DEFAULT_MOBILE_API_PORTS, default_mobile_api_port};
use super::print_activity::PrintActivitySnapshot;
use crate::print::capabilities::ActivePrinterManifest;

pub const APP_ID: &str = "gscale-zebra";
pub const SERVICE_ID: &str = "mobileapi";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceIdentity {
    pub server_name: String,
    pub server_ref: String,
    pub display_name: String,
    pub role: String,
}

impl ServiceIdentity {
    pub fn new(server_name: &str, server_ref: &str, display_name: &str, role: &str) -> Self {
        Self {
            server_name: normalize(server_name, APP_ID),
            server_ref: normalize(server_ref, "unknown"),
            display_name: normalize(display_name, "Operator"),
            role: normalize(role, "operator"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: &'static str,
    pub busy: bool,
    pub print_activity: PrintActivitySnapshot,
}

impl HealthResponse {
    pub fn ok(print_activity: PrintActivitySnapshot) -> Self {
        Self {
            ok: true,
            service: SERVICE_ID,
            busy: print_activity.busy,
            print_activity,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HandshakeResponse {
    pub ok: bool,
    pub service: &'static str,
    pub app: &'static str,
    pub server_name: String,
    pub server_ref: String,
    pub display_name: String,
    pub role: String,
    pub phone: String,
    pub http_port: u16,
    pub discovery_port: u16,
    pub candidate_ports: Vec<u16>,
    pub monitor_path: &'static str,
    pub profile_path: &'static str,
    pub items_path: &'static str,
    pub batch_state_path: &'static str,
    pub requires_auth: bool,
    pub busy: bool,
    pub print_activity: PrintActivitySnapshot,
}

impl HandshakeResponse {
    pub fn new(
        identity: &ServiceIdentity,
        http_port: u16,
        candidate_ports: Vec<u16>,
        print_activity: PrintActivitySnapshot,
    ) -> Self {
        Self {
            ok: true,
            service: SERVICE_ID,
            app: APP_ID,
            server_name: identity.server_name.clone(),
            server_ref: identity.server_ref.clone(),
            display_name: identity.display_name.clone(),
            role: identity.role.clone(),
            phone: String::new(),
            http_port: normalize_port(http_port),
            discovery_port: DEFAULT_DISCOVERY_PORT,
            candidate_ports: normalize_candidate_ports(candidate_ports),
            monitor_path: "/v1/mobile/monitor/state",
            profile_path: "/v1/mobile/profile",
            items_path: "/v1/mobile/items",
            batch_state_path: "/v1/mobile/batch/state",
            requires_auth: false,
            busy: print_activity.busy,
            print_activity,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DiscoveryAnnouncement {
    #[serde(rename = "type")]
    pub announcement_type: &'static str,
    pub app: &'static str,
    pub service: &'static str,
    pub server_name: String,
    pub server_ref: String,
    pub display_name: String,
    pub role: String,
    pub http_port: u16,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidate_ports: Vec<u16>,
}

impl DiscoveryAnnouncement {
    pub fn new(identity: &ServiceIdentity, http_port: u16, candidate_ports: Vec<u16>) -> Self {
        Self {
            announcement_type: "gscale_announce_v1",
            app: APP_ID,
            service: SERVICE_ID,
            server_name: identity.server_name.clone(),
            server_ref: identity.server_ref.clone(),
            display_name: identity.display_name.clone(),
            role: identity.role.clone(),
            http_port: normalize_port(http_port),
            candidate_ports: normalize_candidate_ports(candidate_ports),
        }
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrinterCapabilityFlagsResponse {
    pub thermal_label: bool,
    pub rfid_epc_write: bool,
    pub barcode: bool,
    pub qr: bool,
    pub verify_after_print: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActivePrinterResponse {
    pub id: &'static str,
    pub name: &'static str,
    pub capabilities: PrinterCapabilityFlagsResponse,
    pub required_fields: Vec<&'static str>,
    pub unsupported_modes: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrinterCapabilitiesResponse {
    pub ok: bool,
    pub active_printer: ActivePrinterResponse,
}

impl PrinterCapabilitiesResponse {
    pub fn from_manifest(manifest: ActivePrinterManifest) -> Self {
        Self {
            ok: true,
            active_printer: ActivePrinterResponse {
                id: manifest.id,
                name: manifest.name,
                capabilities: PrinterCapabilityFlagsResponse {
                    thermal_label: manifest.capabilities.thermal_label,
                    rfid_epc_write: manifest.capabilities.rfid_epc_write,
                    barcode: manifest.capabilities.barcode,
                    qr: manifest.capabilities.qr,
                    verify_after_print: manifest.capabilities.verify_after_print,
                },
                required_fields: manifest.required_fields.to_vec(),
                unsupported_modes: manifest.unsupported_modes.to_vec(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SetupStatusResponse {
    pub ok: bool,
    pub erp_write_configured: bool,
    pub erp_write_simulated: bool,
    pub erp_read_configured: bool,
    pub batch_actions_ready: bool,
    pub erp_url: String,
    pub erp_read_url: String,
    pub warehouse_mode: &'static str,
    pub default_warehouse: String,
    pub warehouse_default_configured: bool,
    pub warehouse_default_active: bool,
}

impl SetupStatusResponse {
    pub fn driver_scope() -> Self {
        Self {
            ok: true,
            erp_write_configured: false,
            erp_write_simulated: false,
            erp_read_configured: false,
            batch_actions_ready: false,
            erp_url: String::new(),
            erp_read_url: String::new(),
            warehouse_mode: "manual",
            default_warehouse: String::new(),
            warehouse_default_configured: false,
            warehouse_default_active: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EmptyItemsResponse {
    pub ok: bool,
    pub items: Vec<serde_json::Value>,
}

impl EmptyItemsResponse {
    pub fn driver_scope() -> Self {
        Self {
            ok: true,
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EmptyWarehousesResponse {
    pub ok: bool,
    pub warehouses: Vec<serde_json::Value>,
}

impl EmptyWarehousesResponse {
    pub fn driver_scope() -> Self {
        Self {
            ok: true,
            warehouses: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ItemWarehousesResponse {
    pub ok: bool,
    pub item_code: String,
    pub warehouses: Vec<serde_json::Value>,
}

impl ItemWarehousesResponse {
    pub fn driver_scope(item_code: &str) -> Self {
        Self {
            ok: true,
            item_code: item_code.to_string(),
            warehouses: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EmptyArchiveResponse {
    pub ok: bool,
    pub archive: Vec<serde_json::Value>,
}

impl EmptyArchiveResponse {
    pub fn driver_scope() -> Self {
        Self {
            ok: true,
            archive: Vec::new(),
        }
    }
}

fn normalize(value: &str, fallback: &str) -> String {
    match value.trim() {
        "" => fallback.to_string(),
        value => value.replace(['\n', '\r'], " "),
    }
}

fn normalize_port(port: u16) -> u16 {
    if port == 0 {
        default_mobile_api_port()
    } else {
        port
    }
}

fn normalize_candidate_ports(candidate_ports: Vec<u16>) -> Vec<u16> {
    if candidate_ports.is_empty() {
        DEFAULT_MOBILE_API_PORTS.to_vec()
    } else {
        candidate_ports
            .into_iter()
            .filter(|port| *port > 0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ServiceIdentity {
        ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin")
    }

    #[test]
    fn builds_gscale_compatible_handshake_shape() {
        let handshake = HandshakeResponse::new(
            &identity(),
            39117,
            vec![39117, 41257],
            PrintActivitySnapshot::idle(),
        );

        assert!(handshake.ok);
        assert_eq!(handshake.service, "mobileapi");
        assert_eq!(handshake.app, "gscale-zebra");
        assert_eq!(handshake.server_name, "rp-scale");
        assert_eq!(handshake.discovery_port, 18081);
        assert_eq!(handshake.monitor_path, "/v1/mobile/monitor/state");
        assert!(!handshake.requires_auth);
        assert!(!handshake.busy);
    }

    #[test]
    fn builds_gscale_compatible_discovery_announcement_json() {
        let payload = DiscoveryAnnouncement::new(&identity(), 39117, vec![39117, 41257]);
        let json = String::from_utf8(payload.to_json_bytes().unwrap()).unwrap();

        assert!(json.contains(r#""type":"gscale_announce_v1""#));
        assert!(json.contains(r#""service":"mobileapi""#));
        assert!(json.contains(r#""app":"gscale-zebra""#));
        assert!(json.contains(r#""http_port":39117"#));
        assert!(json.contains(r#""candidate_ports":[39117,41257]"#));
    }

    #[test]
    fn health_response_matches_mobile_fallback_probe() {
        let health = HealthResponse::ok(PrintActivitySnapshot::idle());

        assert_eq!(health.service, "mobileapi");
        assert!(!health.busy);
    }

    #[test]
    fn printer_capability_response_keeps_godex_rfid_disabled() {
        use crate::print::capabilities::manifest_for;
        use crate::print::printer::PrinterKind;

        let response = PrinterCapabilitiesResponse::from_manifest(manifest_for(PrinterKind::Godex));
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""id":"godex""#));
        assert!(json.contains(r#""rfid_epc_write":false"#));
        assert!(json.contains(r#""qr":true"#));
        assert!(json.contains(r#""unsupported_modes":["rfid_epc_write"]"#));
    }

    #[test]
    fn setup_status_is_driver_scope_and_does_not_claim_erp_readiness() {
        let status = SetupStatusResponse::driver_scope();

        assert!(status.ok);
        assert!(!status.erp_write_configured);
        assert!(!status.erp_read_configured);
        assert!(!status.batch_actions_ready);
        assert_eq!(status.warehouse_mode, "manual");
    }

    #[test]
    fn catalog_and_archive_stubs_are_empty_driver_scope_lists() {
        assert!(EmptyItemsResponse::driver_scope().items.is_empty());
        assert!(
            EmptyWarehousesResponse::driver_scope()
                .warehouses
                .is_empty()
        );
        assert!(
            ItemWarehousesResponse::driver_scope("ITEM-1")
                .warehouses
                .is_empty()
        );
        assert!(EmptyArchiveResponse::driver_scope().archive.is_empty());
    }
}
