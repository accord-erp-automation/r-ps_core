use std::fmt;

use crate::core::{CorePrintJob, build_pack_label_content};

use super::capabilities::capabilities_for;
use super::godex::{GodexPackRender, LabelOptions, build_pack_render};
use super::mode::PrintMode;
use super::printer::PrinterKind;
use super::weight::format_print_weight_labels;
use super::zebra::{
    build_label_only_print_command_with_weights, build_rfid_encode_command_with_weights,
};

#[derive(Clone, Debug, PartialEq)]
pub enum PrintCommand {
    ZebraZpl(String),
    GodexPack(GodexPackRender),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrintAdapterError {
    BuildCommand(String),
    UnsupportedMode {
        printer: PrinterKind,
        mode: PrintMode,
    },
}

impl fmt::Display for PrintAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildCommand(error) => write!(f, "{error}"),
            Self::UnsupportedMode { printer, mode } => {
                write!(f, "{} does not support {}", printer.as_str(), mode.as_str())
            }
        }
    }
}

impl std::error::Error for PrintAdapterError {}

pub fn build_print_command(job: CorePrintJob) -> Result<PrintCommand, PrintAdapterError> {
    let printer = job.printer.unwrap_or(PrinterKind::Zebra);
    let mode = job.mode;
    if !capabilities_for(printer).supports_mode(mode) {
        return Err(PrintAdapterError::UnsupportedMode { printer, mode });
    }

    match printer {
        PrinterKind::Zebra => build_zebra_command(job),
        PrinterKind::Godex => build_godex_command(job),
    }
}

fn build_godex_command(job: CorePrintJob) -> Result<PrintCommand, PrintAdapterError> {
    let content =
        build_pack_label_content(&job, "Accord", "5kg").map_err(PrintAdapterError::BuildCommand)?;
    let render = build_pack_render(&content, LabelOptions::default_pack())
        .map_err(PrintAdapterError::BuildCommand)?;
    Ok(PrintCommand::GodexPack(render))
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

    fn unwrap_zebra(command: PrintCommand) -> String {
        match command {
            PrintCommand::ZebraZpl(command) => command,
            PrintCommand::GodexPack(_) => panic!("expected zebra zpl command"),
        }
    }

    #[test]
    fn builds_zebra_rfid_command_from_core_job() {
        let command = unwrap_zebra(
            build_print_command(job(PrintMode::Rfid, Some(PrinterKind::Zebra))).unwrap(),
        );

        assert!(command.contains("^RS8,,,1,N"));
        assert!(command.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
        assert!(command.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(command.contains("^FDBRUTTO: 2.5 kg^FS"));
        assert!(command.contains("^FDMAHSULOT: Green Tea^FS"));
    }

    #[test]
    fn builds_zebra_label_only_command_from_core_job() {
        let command = unwrap_zebra(
            build_print_command(job(PrintMode::LabelOnly, Some(PrinterKind::Zebra))).unwrap(),
        );

        assert!(command.contains("^MMT"));
        assert!(!command.contains("^RFW"));
        assert!(!command.contains("^RS8"));
        assert!(command.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(command.contains("^FDBRUTTO: 2.5 kg^FS"));
    }

    #[test]
    fn defaults_missing_printer_to_zebra_like_gscale_backend() {
        let command = unwrap_zebra(build_print_command(job(PrintMode::Rfid, None)).unwrap());

        assert!(command.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
    }

    #[test]
    fn builds_godex_pack_render_from_core_job() {
        let PrintCommand::GodexPack(render) =
            build_print_command(job(PrintMode::LabelOnly, Some(PrinterKind::Godex))).unwrap()
        else {
            panic!("expected godex pack render");
        };

        assert_eq!(render.commands[11], "Y0,0,TEXTLBL");
        assert_eq!(
            render.commands[12],
            "BA,0,24,1,2,42,0,0,3034257BF7194E406994036B"
        );
        assert_eq!(render.commands[13], "Y224,224,QRLBL");
        assert_eq!(
            render.qr_payload,
            "https://scan.wspace.sbs/L/ACCORD/GREEN+TEA/1.7/2.5/3034257BF7194E406994036B"
        );
        assert_eq!(render.qr_box_dots, 144);
    }

    #[test]
    fn rejects_godex_rfid_mode_by_capability() {
        let err = build_print_command(job(PrintMode::Rfid, Some(PrinterKind::Godex))).unwrap_err();

        assert_eq!(
            err,
            PrintAdapterError::UnsupportedMode {
                printer: PrinterKind::Godex,
                mode: PrintMode::Rfid,
            }
        );
        assert_eq!(err.to_string(), "godex does not support rfid");
    }
}
