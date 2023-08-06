use std::fmt;

use crate::core::CorePrintJob;

use super::mode::PrintMode;
use super::printer::PrinterKind;
use super::weight::format_print_weight_labels;
use super::zebra::{
    build_label_only_print_command_with_weights, build_rfid_encode_command_with_weights,
};

#[derive(Clone, Debug, PartialEq)]
pub enum PrintCommand {
    ZebraZpl(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrintAdapterError {
    UnsupportedPrinter(String),
    BuildCommand(String),
}

impl fmt::Display for PrintAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPrinter(printer) => write!(f, "{printer} print adapter unsupported"),
            Self::BuildCommand(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PrintAdapterError {}

pub fn build_print_command(job: CorePrintJob) -> Result<PrintCommand, PrintAdapterError> {
    match job.printer.unwrap_or(PrinterKind::Zebra) {
        PrinterKind::Zebra => build_zebra_command(job),
        PrinterKind::Godex => Err(PrintAdapterError::UnsupportedPrinter(
            "godex pack label".to_string(),
        )),
    }
}

fn build_zebra_command(job: CorePrintJob) -> Result<PrintCommand, PrintAdapterError> {
    let request = job.into_print_request();
    let weights = format_print_weight_labels(&request);
    let item_name = request.item_label();
    let command = match request.mode {
        PrintMode::Rfid => build_rfid_encode_command_with_weights(
            &request.epc,
            &weights.netto,
            if weights.has_tare {
                &weights.brutto
            } else {
                ""
            },
            item_name,
        ),
        PrintMode::LabelOnly => build_label_only_print_command_with_weights(
            &request.epc,
            &weights.netto,
            if weights.has_tare {
                &weights.brutto
            } else {
                ""
            },
            item_name,
        ),
    }
    .map_err(PrintAdapterError::BuildCommand)?;

    Ok(PrintCommand::ZebraZpl(command))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{PrintSelection, QuantitySource};

    fn job(mode: PrintMode, printer: Option<PrinterKind>) -> CorePrintJob {
        CorePrintJob::from_selection(
            "3034257BF7194E406994036B",
            1.72,
            2.5,
            "kg",
            PrintSelection {
                item_code: "ITEM-1".to_string(),
                item_name: "Green Tea".to_string(),
                warehouse: "Stores - A".to_string(),
                print_mode: mode,
                printer: printer.map(|kind| kind.as_str()).unwrap_or("").to_string(),
                quantity_source: QuantitySource::Scale,
                manual_qty_kg: 0.0,
                tare_enabled: true,
                tare_kg: 0.78,
            },
        )
    }

    #[test]
    fn builds_zebra_rfid_command_from_core_job() {
        let PrintCommand::ZebraZpl(command) =
            build_print_command(job(PrintMode::Rfid, Some(PrinterKind::Zebra))).unwrap();

        assert!(command.contains("^RS8,,,1,N"));
        assert!(command.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
        assert!(command.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(command.contains("^FDBRUTTO: 2.5 kg^FS"));
        assert!(command.contains("^FDMAHSULOT: Green Tea^FS"));
    }

    #[test]
    fn builds_zebra_label_only_command_from_core_job() {
        let PrintCommand::ZebraZpl(command) =
            build_print_command(job(PrintMode::LabelOnly, Some(PrinterKind::Zebra))).unwrap();

        assert!(command.contains("^MMT"));
        assert!(!command.contains("^RFW"));
        assert!(!command.contains("^RS8"));
        assert!(command.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(command.contains("^FDBRUTTO: 2.5 kg^FS"));
    }

    #[test]
    fn defaults_missing_printer_to_zebra_like_gscale_backend() {
        let PrintCommand::ZebraZpl(command) =
            build_print_command(job(PrintMode::Rfid, None)).unwrap();

        assert!(command.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
    }

    #[test]
    fn rejects_godex_until_pack_label_adapter_is_ported() {
        let err =
            build_print_command(job(PrintMode::LabelOnly, Some(PrinterKind::Godex))).unwrap_err();

        assert_eq!(
            err,
            PrintAdapterError::UnsupportedPrinter("godex pack label".to_string())
        );
    }
}
