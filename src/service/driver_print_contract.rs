use serde::{Deserialize, Serialize};

use crate::core::{PrintSelection, QuantitySource};
use crate::print::{PrintMode, PrinterKind};
use crate::runtime::PrintPipelineResult;
use crate::scale::Reading;
use crate::service::print_activity::PrintActivitySnapshot;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct DriverPrintRequest {
    #[serde(default)]
    pub epc: String,
    #[serde(default)]
    pub item_code: String,
    #[serde(default)]
    pub item_name: String,
    #[serde(default)]
    pub warehouse: String,
    #[serde(default)]
    pub print_mode: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub printer: String,
    #[serde(default)]
    pub gross_qty: Option<f64>,
    #[serde(default)]
    pub qty: Option<f64>,
    #[serde(default)]
    pub manual_qty_kg: Option<f64>,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub tare_enabled: bool,
    #[serde(default)]
    pub tare: bool,
    #[serde(default)]
    pub tare_kg: f64,
}

impl DriverPrintRequest {
    pub fn from_json(body: &str) -> Result<Self, DriverPrintRequestError> {
        serde_json::from_str(body.trim()).map_err(|_| DriverPrintRequestError::InvalidJson)
    }

    pub fn into_job(
        self,
        default_printer: PrinterKind,
    ) -> Result<DriverPrintJob, DriverPrintRequestError> {
        let epc = self.epc.trim().to_ascii_uppercase();
        let item_code = self.item_code.trim().to_string();
        let warehouse = self.warehouse.trim().to_string();
        if epc.is_empty() {
            return Err(DriverPrintRequestError::EpcRequired);
        }
        if item_code.is_empty() || warehouse.is_empty() {
            return Err(DriverPrintRequestError::ItemCodeAndWarehouseRequired);
        }

        let gross_qty = self.gross_qty.or(self.manual_qty_kg).or(self.qty);
        let Some(gross_qty) = gross_qty else {
            return Err(DriverPrintRequestError::GrossQtyRequired);
        };
        if !gross_qty.is_finite() || gross_qty <= 0.0 {
            return Err(DriverPrintRequestError::InvalidGrossQty);
        }

        let printer = PrinterKind::resolve(&self.printer, default_printer.as_str());
        let requested_mode = fallback_print_mode(&self.print_mode, &self.mode);
        let print_mode = if requested_mode.trim().is_empty() && printer == PrinterKind::Godex {
            PrintMode::LabelOnly
        } else {
            PrintMode::normalize(&requested_mode)
        };
        let unit = normalize_unit(&self.unit);
        let tare_enabled = self.tare_enabled || self.tare || self.tare_kg > 0.0;
        let item_name = fallback_item_name(&self.item_name, &item_code);

        let selection = PrintSelection {
            item_code,
            item_name,
            warehouse,
            print_mode,
            printer: printer.as_str().to_string(),
            quantity_source: QuantitySource::Scale,
            manual_qty_kg: 0.0,
            tare_enabled,
            tare_kg: self.tare_kg,
        }
        .normalized();
        let reading = Reading::from_source("driver", "print-request", 0, &unit).with_weight(
            gross_qty,
            Some(true),
            "driver print request",
        );

        Ok(DriverPrintJob {
            epc,
            warehouse: selection.warehouse.clone(),
            reading,
            selection,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DriverPrintJob {
    pub epc: String,
    pub warehouse: String,
    pub reading: Reading,
    pub selection: PrintSelection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverPrintRequestError {
    InvalidJson,
    EpcRequired,
    ItemCodeAndWarehouseRequired,
    GrossQtyRequired,
    InvalidGrossQty,
}

impl DriverPrintRequestError {
    pub fn status(self) -> u16 {
        400
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::EpcRequired => "epc_required",
            Self::ItemCodeAndWarehouseRequired => "item_code_and_warehouse_required",
            Self::GrossQtyRequired => "gross_qty_required",
            Self::InvalidGrossQty => "invalid_gross_qty",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DriverPrintResponse {
    pub ok: bool,
    pub status: String,
    pub epc: String,
    pub item_code: String,
    pub item_name: String,
    pub warehouse: String,
    pub printer: String,
    pub mode: String,
    pub qty: f64,
    pub net_qty: f64,
    pub gross_qty: f64,
    pub unit: String,
    pub tare: bool,
    pub tare_kg: f64,
    pub printer_status: String,
}

impl DriverPrintResponse {
    pub fn done(
        job: &DriverPrintJob,
        prepared: &PrintPipelineResult,
        printer_status: String,
    ) -> Self {
        let plan_job = &prepared.plan.job;
        Self {
            ok: true,
            status: "done".to_string(),
            epc: plan_job.epc.clone(),
            item_code: plan_job.item_code.clone(),
            item_name: plan_job.item_name.clone(),
            warehouse: job.warehouse.clone(),
            printer: prepared.plan.printer.as_str().to_string(),
            mode: plan_job.mode.as_str().to_string(),
            qty: plan_job.net_qty,
            net_qty: plan_job.net_qty,
            gross_qty: plan_job.gross_qty,
            unit: plan_job.unit.clone(),
            tare: plan_job.tare,
            tare_kg: plan_job.tare_kg,
            printer_status,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DriverPrintErrorResponse {
    pub ok: bool,
    pub error: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_activity: Option<PrintActivitySnapshot>,
}

impl DriverPrintErrorResponse {
    pub fn new(error: &'static str, detail: impl Into<String>) -> Self {
        Self {
            ok: false,
            error,
            detail: detail.into(),
            print_activity: None,
        }
    }

    pub fn busy(print_activity: PrintActivitySnapshot) -> Self {
        Self {
            ok: false,
            error: "driver_busy",
            detail: "Printer server band. Boshqa mobile print yakunlagandan keyin qayta urining."
                .to_string(),
            print_activity: Some(print_activity),
        }
    }
}

fn fallback_print_mode(print_mode: &str, mode: &str) -> String {
    let print_mode = print_mode.trim();
    if print_mode.is_empty() {
        mode.trim().to_string()
    } else {
        print_mode.to_string()
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

fn normalize_unit(unit: &str) -> String {
    let unit = unit.trim();
    if unit.is_empty() {
        "kg".to_string()
    } else {
        unit.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_driver_print_job_from_rs_owned_values() {
        let request = DriverPrintRequest::from_json(
            r#"{
                "epc":" 3034257bf7194e406994036b ",
                "item_code":" ITEM-1 ",
                "item_name":" Green Tea ",
                "warehouse":" Stores - A ",
                "printer":"godex",
                "gross_qty":2.5,
                "tare_enabled":true,
                "tare_kg":0.78
            }"#,
        )
        .unwrap();
        let job = request.into_job(PrinterKind::Zebra).unwrap();

        assert_eq!(job.epc, "3034257BF7194E406994036B");
        assert_eq!(job.selection.item_code, "ITEM-1");
        assert_eq!(job.selection.item_name, "Green Tea");
        assert_eq!(job.selection.warehouse, "Stores - A");
        assert_eq!(job.selection.printer, "godex");
        assert_eq!(job.selection.print_mode, PrintMode::LabelOnly);
        assert_eq!(job.reading.weight, Some(2.5));
        assert_eq!(job.reading.unit, "kg");
        assert!(job.selection.tare_enabled);
        assert_eq!(job.selection.tare_kg, 0.78);
    }

    #[test]
    fn keeps_explicit_unsupported_mode_for_core_rejection() {
        let request = DriverPrintRequest::from_json(
            r#"{
                "epc":"EPC-1",
                "item_code":"ITEM-1",
                "warehouse":"Stores - A",
                "printer":"godex",
                "print_mode":"rfid",
                "gross_qty":1.25
            }"#,
        )
        .unwrap();
        let job = request.into_job(PrinterKind::Zebra).unwrap();

        assert_eq!(job.selection.printer, "godex");
        assert_eq!(job.selection.print_mode, PrintMode::Rfid);
    }

    #[test]
    fn rejects_missing_required_fields() {
        let err = DriverPrintRequest::from_json(r#"{"item_code":"ITEM-1"}"#)
            .unwrap()
            .into_job(PrinterKind::Zebra)
            .unwrap_err();

        assert_eq!(err, DriverPrintRequestError::EpcRequired);
        assert_eq!(err.code(), "epc_required");
        assert_eq!(err.status(), 400);
    }

    #[test]
    fn rejects_invalid_gross_qty() {
        let err = DriverPrintRequest::from_json(
            r#"{
                "epc":"EPC-1",
                "item_code":"ITEM-1",
                "warehouse":"Stores - A",
                "gross_qty":0
            }"#,
        )
        .unwrap()
        .into_job(PrinterKind::Zebra)
        .unwrap_err();

        assert_eq!(err, DriverPrintRequestError::InvalidGrossQty);
    }
}
