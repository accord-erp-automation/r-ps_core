use std::sync::{Arc, Mutex};

use super::mobile_contract::ServiceIdentity;
use super::monitor_contract::MonitorResponse;
use crate::print::printer::PrinterKind;
use crate::scale::Reading;

#[derive(Clone, Debug, Default)]
pub struct MonitorRuntimeState {
    last_reading: Arc<Mutex<Option<Reading>>>,
}

impl MonitorRuntimeState {
    pub fn record_reading(&self, reading: Reading) {
        if let Ok(mut last_reading) = self.last_reading.lock() {
            *last_reading = Some(reading);
        }
    }

    pub fn snapshot(
        &self,
        identity: &ServiceIdentity,
        active_printer: PrinterKind,
    ) -> MonitorResponse {
        let reading = self
            .last_reading
            .lock()
            .ok()
            .and_then(|last_reading| last_reading.clone());

        match reading {
            Some(reading) => MonitorResponse::driver_with_scale(identity, active_printer, &reading),
            None => MonitorResponse::driver_idle(identity, active_printer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::mobile_contract::ServiceIdentity;
    use serde_json::Value;

    #[test]
    fn snapshot_exposes_latest_scale_reading() {
        let runtime = MonitorRuntimeState::default();
        runtime.record_reading(Reading::serial("/dev/tty.sim", 9600, "kg").with_weight(
            1.25,
            Some(true),
            "1.250 kg ST",
        ));

        let identity = ServiceIdentity::new("rp-scale", "rps_1", "RP Scale", "operator");
        let body = serde_json::to_value(runtime.snapshot(&identity, PrinterKind::Zebra)).unwrap();

        assert_eq!(body["state"]["scale"]["source"], "serial");
        assert_eq!(body["state"]["scale"]["port"], "/dev/tty.sim");
        assert_eq!(body["state"]["scale"]["weight"], 1.25);
        assert_eq!(body["state"]["scale"]["stable"], true);
        assert_eq!(body["state"]["printer"]["kind"], "zebra");
    }

    #[test]
    fn empty_snapshot_is_mobile_safe() {
        let runtime = MonitorRuntimeState::default();
        let identity = ServiceIdentity::new("rp-scale", "rps_1", "RP Scale", "operator");
        let body = serde_json::to_value(runtime.snapshot(&identity, PrinterKind::Godex)).unwrap();

        assert_eq!(body["state"]["scale"]["weight"], Value::Null);
        assert_eq!(body["state"]["batch"]["printer"], "godex");
        assert_eq!(body["state"]["batch"]["print_mode"], "label");
    }
}
