use serde::Serialize;

use super::mobile_contract::ServiceIdentity;
use crate::print::printer::PrinterKind;
use crate::scale::Reading;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonitorResponse {
    pub ok: bool,
    pub profile: MonitorProfile,
    pub state: MonitorState,
    pub printer: MonitorPrinter,
}

impl MonitorResponse {
    pub fn driver_idle(identity: &ServiceIdentity, active_printer: PrinterKind) -> Self {
        let printer = MonitorPrinter::disconnected(active_printer);
        Self {
            ok: true,
            profile: MonitorProfile::from_identity(identity),
            state: MonitorState::driver_idle(printer.clone(), active_printer),
            printer,
        }
    }

    pub fn driver_with_scale(
        identity: &ServiceIdentity,
        active_printer: PrinterKind,
        reading: &Reading,
    ) -> Self {
        let printer = MonitorPrinter::disconnected(active_printer);
        Self {
            ok: true,
            profile: MonitorProfile::from_identity(identity),
            state: MonitorState::driver_with_scale(printer.clone(), active_printer, reading),
            printer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BatchStateResponse {
    pub ok: bool,
    pub batch: BatchSnapshot,
}

impl BatchStateResponse {
    pub fn inactive(active_printer: PrinterKind) -> Self {
        Self {
            ok: true,
            batch: BatchSnapshot::inactive(active_printer),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonitorProfile {
    pub role: String,
    pub display_name: String,
    pub legal_name: String,
    #[serde(rename = "ref")]
    pub profile_ref: String,
    pub phone: String,
    pub avatar_url: String,
}

impl MonitorProfile {
    fn from_identity(identity: &ServiceIdentity) -> Self {
        Self {
            role: identity.role.clone(),
            display_name: identity.display_name.clone(),
            legal_name: identity.display_name.clone(),
            profile_ref: identity.server_ref.clone(),
            phone: String::new(),
            avatar_url: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonitorState {
    pub scale: ScaleSnapshot,
    pub zebra: ZebraSnapshot,
    pub printer: MonitorPrinter,
    pub batch: BatchSnapshot,
    pub print_request: PrintRequestSnapshot,
    pub archive_print: ArchivePrintSnapshot,
    pub updated_at: String,
}

impl MonitorState {
    fn driver_idle(printer: MonitorPrinter, active_printer: PrinterKind) -> Self {
        Self {
            scale: ScaleSnapshot::disconnected(),
            zebra: ZebraSnapshot::disconnected(),
            printer,
            batch: BatchSnapshot::inactive(active_printer),
            print_request: PrintRequestSnapshot::idle(),
            archive_print: ArchivePrintSnapshot::idle(),
            updated_at: String::new(),
        }
    }

    fn driver_with_scale(
        printer: MonitorPrinter,
        active_printer: PrinterKind,
        reading: &Reading,
    ) -> Self {
        Self {
            scale: ScaleSnapshot::from_reading(reading),
            zebra: ZebraSnapshot::disconnected(),
            printer,
            batch: BatchSnapshot::inactive(active_printer),
            print_request: PrintRequestSnapshot::idle(),
            archive_print: ArchivePrintSnapshot::idle(),
            updated_at: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScaleSnapshot {
    pub source: String,
    pub port: String,
    pub weight: Option<f64>,
    pub unit: String,
    pub stable: Option<bool>,
    pub error: String,
    pub updated_at: String,
}

impl ScaleSnapshot {
    fn disconnected() -> Self {
        Self {
            source: String::new(),
            port: String::new(),
            weight: None,
            unit: "kg".to_string(),
            stable: None,
            error: String::new(),
            updated_at: String::new(),
        }
    }

    fn from_reading(reading: &Reading) -> Self {
        Self {
            source: reading.source.clone(),
            port: reading.port.clone(),
            weight: reading.weight,
            unit: normalize_unit(&reading.unit),
            stable: reading.stable,
            error: reading.error.clone(),
            updated_at: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ZebraSnapshot {
    pub connected: bool,
    pub device_path: String,
    pub name: String,
    pub device_state: String,
    pub media_state: String,
    pub read_line1: String,
    pub read_line2: String,
    pub last_epc: String,
    pub verify: String,
    pub action: String,
    pub error: String,
    pub updated_at: String,
}

impl ZebraSnapshot {
    fn disconnected() -> Self {
        Self {
            connected: false,
            device_path: String::new(),
            name: String::new(),
            device_state: String::new(),
            media_state: String::new(),
            read_line1: String::new(),
            read_line2: String::new(),
            last_epc: String::new(),
            verify: "idle".to_string(),
            action: "printer state".to_string(),
            error: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonitorPrinter {
    pub ok: bool,
    pub connected: bool,
    pub kind: String,
    pub label: String,
    pub device_paths: Vec<String>,
    pub error: String,
    pub updated_at: String,
}

impl MonitorPrinter {
    fn disconnected(active_printer: PrinterKind) -> Self {
        Self {
            ok: false,
            connected: false,
            kind: active_printer.as_str().to_string(),
            label: "ulanmagan".to_string(),
            device_paths: Vec::new(),
            error: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BatchSnapshot {
    pub active: bool,
    pub chat_id: i64,
    pub item_code: String,
    pub item_name: String,
    pub warehouse: String,
    pub print_mode: String,
    pub printer: String,
    pub quantity_source: String,
    pub manual_qty_kg: f64,
    pub tare: bool,
    pub tare_kg: f64,
    pub total_qty: f64,
    pub updated_at: String,
}

impl BatchSnapshot {
    fn inactive(active_printer: PrinterKind) -> Self {
        let printer = active_printer.as_str().to_string();
        let print_mode = if active_printer == PrinterKind::Godex {
            "label"
        } else {
            "rfid"
        };

        Self {
            active: false,
            chat_id: 0,
            item_code: String::new(),
            item_name: String::new(),
            warehouse: String::new(),
            print_mode: print_mode.to_string(),
            printer,
            quantity_source: "scale".to_string(),
            manual_qty_kg: 0.0,
            tare: false,
            tare_kg: 0.0,
            total_qty: 0.0,
            updated_at: String::new(),
        }
    }
}

fn normalize_unit(unit: &str) -> String {
    let unit = unit.trim().to_ascii_lowercase();
    if unit.is_empty() {
        "kg".to_string()
    } else {
        unit
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrintRequestSnapshot {
    pub epc: String,
    pub qty: Option<f64>,
    pub gross_qty: Option<f64>,
    pub unit: String,
    pub item_code: String,
    pub item_name: String,
    pub mode: String,
    pub printer: String,
    pub tare: bool,
    pub tare_kg: f64,
    pub status: String,
    pub error: String,
    pub requested_at: String,
    pub updated_at: String,
}

impl PrintRequestSnapshot {
    fn idle() -> Self {
        Self {
            epc: String::new(),
            qty: None,
            gross_qty: None,
            unit: "kg".to_string(),
            item_code: String::new(),
            item_name: String::new(),
            mode: String::new(),
            printer: String::new(),
            tare: false,
            tare_kg: 0.0,
            status: "idle".to_string(),
            error: String::new(),
            requested_at: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArchivePrintSnapshot {
    pub request_id: String,
    pub session_id: String,
    pub item_code: String,
    pub item_name: String,
    pub total_qty: f64,
    pub unit: String,
    pub batch_time: String,
    pub printer: String,
    pub status: String,
    pub error: String,
    pub requested_at: String,
    pub updated_at: String,
}

impl ArchivePrintSnapshot {
    fn idle() -> Self {
        Self {
            request_id: String::new(),
            session_id: String::new(),
            item_code: String::new(),
            item_name: String::new(),
            total_qty: 0.0,
            unit: "kg".to_string(),
            batch_time: String::new(),
            printer: String::new(),
            status: "idle".to_string(),
            error: String::new(),
            requested_at: String::new(),
            updated_at: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::mobile_contract::ServiceIdentity;
    use serde_json::Value;

    fn response() -> Value {
        let identity = ServiceIdentity::new("rp-scale", "rps_1", "RP Scale", "operator");
        serde_json::to_value(MonitorResponse::driver_idle(&identity, PrinterKind::Godex)).unwrap()
    }

    #[test]
    fn builds_mobile_monitor_shape_from_gscale_bridge_contract() {
        let body = response();

        assert_eq!(body["ok"], true);
        assert!(body["state"]["scale"].is_object());
        assert!(body["state"]["zebra"].is_object());
        assert!(body["state"]["printer"].is_object());
        assert!(body["state"]["batch"].is_object());
        assert!(body["state"]["print_request"].is_object());
        assert!(body["state"]["archive_print"].is_object());
    }

    #[test]
    fn defaults_to_safe_disconnected_driver_state() {
        let body = response();

        assert_eq!(body["state"]["scale"]["weight"], Value::Null);
        assert_eq!(body["state"]["scale"]["stable"], Value::Null);
        assert_eq!(body["state"]["scale"]["unit"], "kg");
        assert_eq!(body["state"]["printer"]["connected"], false);
        assert_eq!(body["state"]["printer"]["kind"], "godex");
        assert_eq!(body["state"]["printer"]["label"], "ulanmagan");
        assert_eq!(body["state"]["batch"]["active"], false);
        assert_eq!(body["state"]["batch"]["printer"], "godex");
        assert_eq!(body["state"]["batch"]["print_mode"], "label");
        assert_eq!(body["state"]["print_request"]["status"], "idle");
    }

    #[test]
    fn exposes_realtime_scale_reading() {
        let identity = ServiceIdentity::new("rp-scale", "rps_1", "RP Scale", "operator");
        let reading = Reading::serial("/dev/ttys001", 9600, "KG").with_weight(
            2.75,
            Some(false),
            "2.750 KG US",
        );
        let body = serde_json::to_value(MonitorResponse::driver_with_scale(
            &identity,
            PrinterKind::Zebra,
            &reading,
        ))
        .unwrap();

        assert_eq!(body["state"]["scale"]["source"], "serial");
        assert_eq!(body["state"]["scale"]["port"], "/dev/ttys001");
        assert_eq!(body["state"]["scale"]["weight"], 2.75);
        assert_eq!(body["state"]["scale"]["unit"], "kg");
        assert_eq!(body["state"]["scale"]["stable"], false);
    }

    #[test]
    fn builds_batch_state_response_like_gscale_mobileapi() {
        let body = serde_json::to_value(BatchStateResponse::inactive(PrinterKind::Godex)).unwrap();

        assert_eq!(body["ok"], true);
        assert_eq!(body["batch"]["active"], false);
        assert_eq!(body["batch"]["printer"], "godex");
        assert_eq!(body["batch"]["print_mode"], "label");
        assert_eq!(body["batch"]["quantity_source"], "scale");
    }
}
