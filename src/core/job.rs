use crate::print::mode::PrintMode;
use crate::print::printer::PrinterKind;
use crate::print::request::PrintRequest;

use super::selection::PrintSelection;

#[derive(Clone, Debug, PartialEq)]
pub struct CorePrintJob {
    pub epc: String,
    pub net_qty: f64,
    pub gross_qty: f64,
    pub unit: String,
    pub item_code: String,
    pub item_name: String,
    pub label_kind: String,
    pub executor_name: String,
    pub mode: PrintMode,
    pub printer: Option<PrinterKind>,
    pub tare: bool,
    pub tare_kg: f64,
}

impl CorePrintJob {
    pub fn from_selection(
        epc: &str,
        net_qty: f64,
        gross_qty: f64,
        unit: &str,
        selection: PrintSelection,
    ) -> Self {
        let selection = selection.normalized();
        let unit = match unit.trim() {
            "" => "kg".to_string(),
            value => value.to_string(),
        };

        Self {
            epc: epc.trim().to_ascii_uppercase(),
            net_qty,
            gross_qty,
            unit,
            item_code: selection.item_code,
            item_name: selection.item_name,
            label_kind: String::new(),
            executor_name: String::new(),
            mode: selection.print_mode,
            printer: PrinterKind::normalize_request(&selection.printer),
            tare: selection.tare_enabled,
            tare_kg: selection.tare_kg,
        }
    }

    pub fn into_print_request(self) -> PrintRequest {
        PrintRequest {
            epc: self.epc,
            qty: Some(self.net_qty),
            gross_qty: Some(self.gross_qty),
            unit: self.unit,
            item_code: self.item_code,
            item_name: self.item_name,
            mode: self.mode,
            printer: self.printer,
            tare: self.tare,
            tare_kg: self.tare_kg,
        }
        .normalized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::selection::{PrintSelection, QuantitySource};

    fn selection() -> PrintSelection {
        PrintSelection {
            item_code: " ITEM-1 ".to_string(),
            item_name: " Green Tea ".to_string(),
            warehouse: " Stores - A ".to_string(),
            print_mode: PrintMode::LabelOnly,
            printer: "g500".to_string(),
            quantity_source: QuantitySource::Scale,
            manual_qty_kg: 0.0,
            tare_enabled: true,
            tare_kg: 0.78,
        }
    }

    #[test]
    fn builds_core_print_job_like_gscale_set_print_request() {
        let job = CorePrintJob::from_selection(" abc123 ", 1.72, 2.5, "", selection());

        assert_eq!(job.epc, "ABC123");
        assert_eq!(job.net_qty, 1.72);
        assert_eq!(job.gross_qty, 2.5);
        assert_eq!(job.unit, "kg");
        assert_eq!(job.item_code, "ITEM-1");
        assert_eq!(job.item_name, "Green Tea");
        assert_eq!(job.mode, PrintMode::LabelOnly);
        assert_eq!(job.printer, Some(PrinterKind::Godex));
        assert!(job.tare);
        assert_eq!(job.tare_kg, 0.78);
    }

    #[test]
    fn converts_core_job_to_print_request() {
        let request = CorePrintJob::from_selection(" abc123 ", 1.72, 2.5, "", selection())
            .into_print_request();

        assert_eq!(request.epc, "ABC123");
        assert_eq!(request.qty, Some(1.72));
        assert_eq!(request.gross_qty, Some(2.5));
        assert_eq!(request.unit, "kg");
        assert_eq!(request.item_name, "Green Tea");
        assert_eq!(request.printer, Some(PrinterKind::Godex));
        assert!(request.tare);
    }
}
