use std::sync::{Arc, Mutex};

use super::mobile_contract::ServiceIdentity;
use super::monitor_contract::{BatchSnapshot, MonitorResponse};
use crate::print::printer::PrinterKind;
use crate::scale::Reading;

#[derive(Clone, Debug, Default)]
pub struct MonitorRuntimeState {
    last_reading: Arc<Mutex<Option<Reading>>>,
    batch: Arc<Mutex<Option<BatchSnapshot>>>,
}

impl MonitorRuntimeState {
    pub fn record_reading(&self, reading: Reading) {
        if let Ok(mut last_reading) = self.last_reading.lock() {
            *last_reading = Some(reading);
        }
    }

    pub fn start_batch(&self, batch: BatchSnapshot) -> BatchSnapshot {
        if let Ok(mut active_batch) = self.batch.lock() {
            *active_batch = Some(batch.clone());
        }
        batch
    }

    pub fn stop_batch(&self, active_printer: PrinterKind) -> Result<BatchSnapshot, &'static str> {
        let Ok(mut active_batch) = self.batch.lock() else {
            return Err("batch_state_unavailable");
        };
        if active_batch.take().is_none() {
            return Err("batch_not_active");
        }
        Ok(BatchSnapshot::inactive(active_printer))
    }

    pub fn batch_snapshot(&self, active_printer: PrinterKind) -> BatchSnapshot {
        self.batch
            .lock()
            .ok()
            .and_then(|active_batch| active_batch.clone())
            .unwrap_or_else(|| BatchSnapshot::inactive(active_printer))
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
        let batch = self.batch_snapshot(active_printer);

        match reading {
            Some(reading) => MonitorResponse::driver_with_scale_and_batch(
                identity,
                active_printer,
                &reading,
                batch,
            ),
            None => MonitorResponse::driver_idle_with_batch(identity, active_printer, batch),
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

    #[test]
    fn snapshot_exposes_active_local_batch() {
        let runtime = MonitorRuntimeState::default();
        runtime.start_batch(BatchSnapshot {
            active: true,
            chat_id: 0,
            item_code: "ITEM-1".to_string(),
            item_name: "Sugar".to_string(),
            warehouse: "Stores - A".to_string(),
            print_mode: "label".to_string(),
            printer: "godex".to_string(),
            quantity_source: "scale".to_string(),
            manual_qty_kg: 0.0,
            tare: false,
            tare_kg: 0.0,
            total_qty: 0.0,
            updated_at: String::new(),
        });

        let identity = ServiceIdentity::new("rp-scale", "rps_1", "RP Scale", "operator");
        let body = serde_json::to_value(runtime.snapshot(&identity, PrinterKind::Godex)).unwrap();

        assert_eq!(body["state"]["batch"]["active"], true);
        assert_eq!(body["state"]["batch"]["item_code"], "ITEM-1");
        assert_eq!(body["state"]["batch"]["warehouse"], "Stores - A");
    }

    #[test]
    fn stop_batch_clears_active_local_batch() {
        let runtime = MonitorRuntimeState::default();
        runtime.start_batch(BatchSnapshot {
            active: true,
            chat_id: 0,
            item_code: "ITEM-1".to_string(),
            item_name: "Sugar".to_string(),
            warehouse: "Stores - A".to_string(),
            print_mode: "rfid".to_string(),
            printer: "zebra".to_string(),
            quantity_source: "scale".to_string(),
            manual_qty_kg: 0.0,
            tare: false,
            tare_kg: 0.0,
            total_qty: 0.0,
            updated_at: String::new(),
        });

        let stopped = runtime.stop_batch(PrinterKind::Zebra).unwrap();

        assert!(!stopped.active);
        assert!(!runtime.batch_snapshot(PrinterKind::Zebra).active);
        assert_eq!(
            runtime.stop_batch(PrinterKind::Zebra).unwrap_err(),
            "batch_not_active"
        );
    }
}
