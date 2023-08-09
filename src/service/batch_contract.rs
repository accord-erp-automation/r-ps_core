use serde::Deserialize;

use super::monitor_contract::BatchSnapshot;
use crate::core::selection::QuantitySource;
use crate::print::{PrintMode, PrinterKind};

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BatchStartRequest {
    #[serde(default)]
    pub item_code: String,
    #[serde(default)]
    pub item_name: String,
    #[serde(default)]
    pub warehouse: String,
    #[serde(default)]
    pub print_mode: String,
    #[serde(default)]
    pub printer: String,
    #[serde(default)]
    pub quantity_source: String,
    #[serde(default)]
    pub manual_qty_kg: f64,
    #[serde(default)]
    pub tare_enabled: bool,
    #[serde(default)]
    pub tare_kg: f64,
}

impl BatchStartRequest {
    pub fn from_json(body: &str) -> Result<Self, BatchStartError> {
        serde_json::from_str(body.trim()).map_err(|_| BatchStartError::InvalidJson)
    }

    pub fn into_snapshot(
        self,
        default_printer: PrinterKind,
    ) -> Result<BatchSnapshot, BatchStartError> {
        let item_code = self.item_code.trim().to_string();
        let warehouse = self.warehouse.trim().to_string();
        if item_code.is_empty() || warehouse.is_empty() {
            return Err(BatchStartError::ItemCodeAndWarehouseRequired);
        }

        let printer = PrinterKind::resolve(&self.printer, default_printer.as_str());
        let print_mode = print_mode_for_printer(printer, &self.print_mode);
        let quantity_source = QuantitySource::normalize(&self.quantity_source);
        let manual_qty_kg = if quantity_source == QuantitySource::Manual && self.manual_qty_kg > 0.0
        {
            self.manual_qty_kg
        } else {
            0.0
        };
        let tare_kg = if self.tare_enabled && self.tare_kg > 0.0 {
            self.tare_kg
        } else {
            0.0
        };

        Ok(BatchSnapshot {
            active: true,
            chat_id: 0,
            item_code: item_code.clone(),
            item_name: fallback_item_name(&self.item_name, &item_code),
            warehouse,
            print_mode: print_mode.as_str().to_string(),
            printer: printer.as_str().to_string(),
            quantity_source: quantity_source.as_str().to_string(),
            manual_qty_kg,
            tare: tare_kg > 0.0,
            tare_kg,
            total_qty: 0.0,
            updated_at: String::new(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchStartError {
    InvalidJson,
    ItemCodeAndWarehouseRequired,
}

impl BatchStartError {
    pub fn status(self) -> u16 {
        match self {
            Self::InvalidJson => 400,
            Self::ItemCodeAndWarehouseRequired => 400,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::ItemCodeAndWarehouseRequired => "item_code_and_warehouse_required",
        }
    }
}

fn print_mode_for_printer(printer: PrinterKind, requested: &str) -> PrintMode {
    if printer == PrinterKind::Godex {
        PrintMode::LabelOnly
    } else {
        PrintMode::normalize(requested)
    }
}

fn fallback_item_name(item_name: &str, item_code: &str) -> String {
    let item_name = item_name.trim();
    if item_name.is_empty() {
        item_code.to_string()
    } else {
        item_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_active_snapshot_from_mobile_batch_start_json() {
        let request = BatchStartRequest::from_json(
            r#"{
                "item_code":" ITEM-1 ",
                "item_name":" Sugar ",
                "warehouse":" Stores - A ",
                "print_mode":"rfid",
                "printer":"godex",
                "quantity_source":"scale",
                "manual_qty_kg":7,
                "tare_enabled":true,
                "tare_kg":0.25
            }"#,
        )
        .unwrap();
        let batch = request.into_snapshot(PrinterKind::Zebra).unwrap();

        assert!(batch.active);
        assert_eq!(batch.item_code, "ITEM-1");
        assert_eq!(batch.item_name, "Sugar");
        assert_eq!(batch.warehouse, "Stores - A");
        assert_eq!(batch.printer, "godex");
        assert_eq!(batch.print_mode, "label");
        assert_eq!(batch.quantity_source, "scale");
        assert_eq!(batch.manual_qty_kg, 0.0);
        assert!(batch.tare);
        assert_eq!(batch.tare_kg, 0.25);
    }

    #[test]
    fn rejects_missing_item_or_warehouse_like_mobileapi() {
        let request = BatchStartRequest::from_json(r#"{"item_code":"ITEM-1"}"#).unwrap();
        let err = request.into_snapshot(PrinterKind::Zebra).unwrap_err();

        assert_eq!(err.code(), "item_code_and_warehouse_required");
        assert_eq!(err.status(), 400);
    }

    #[test]
    fn keeps_manual_quantity_only_for_manual_source() {
        let request = BatchStartRequest::from_json(
            r#"{
                "item_code":"ITEM-1",
                "warehouse":"Stores - A",
                "printer":"zebra",
                "quantity_source":"manual",
                "manual_qty_kg":2.5
            }"#,
        )
        .unwrap();
        let batch = request.into_snapshot(PrinterKind::Zebra).unwrap();

        assert_eq!(batch.quantity_source, "manual");
        assert_eq!(batch.manual_qty_kg, 2.5);
        assert_eq!(batch.print_mode, "rfid");
    }
}
