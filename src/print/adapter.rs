use std::fmt;

use crate::core::{
    CorePrintJob, CorePrintPlan, build_pack_label_content, build_progress_label_content,
};

use super::godex::{
    GodexPackRender, LabelOptions, build_pack_render, build_progress_pack_render,
    build_qolip_cell_render, build_qolip_code_render,
};
use super::mode::PrintMode;
use super::printer::PrinterKind;
use super::weight::format_print_weight_labels;
use super::zebra::{
    build_label_only_print_command_with_weights, build_qolip_cell_qr_command,
    build_qolip_code_qr_command,
    build_rfid_encode_command_with_weights,
};

#[derive(Clone, Debug, PartialEq)]
pub enum PrintCommand {
    ZebraZpl(String),
    GodexPack(GodexPackRender),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrintAdapterError {
    BuildCommand(String),
}

impl fmt::Display for PrintAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildCommand(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PrintAdapterError {}

pub fn build_print_command(plan: CorePrintPlan) -> Result<PrintCommand, PrintAdapterError> {
    match plan.printer {
        PrinterKind::Zebra => build_zebra_command(plan.job),
        PrinterKind::Godex => build_godex_command(plan.job),
    }
}

fn build_godex_command(job: CorePrintJob) -> Result<PrintCommand, PrintAdapterError> {
    if is_qolip_cell_label(&job.label_kind) {
        let content = build_pack_label_content(&job, "Accord", "5kg")
            .map_err(PrintAdapterError::BuildCommand)?;
        let render = build_qolip_cell_render(&content, LabelOptions::default_pack())
            .map_err(PrintAdapterError::BuildCommand)?;
        return Ok(PrintCommand::GodexPack(render));
    }
    if is_qolip_code_label(&job.label_kind) {
        let code = job.item_code.clone();
        let content = build_pack_label_content(&job, "Accord", "5kg")
            .map_err(PrintAdapterError::BuildCommand)?;
        let render = build_qolip_code_render(&content, &code, LabelOptions::default_pack())
            .map_err(PrintAdapterError::BuildCommand)?;
        return Ok(PrintCommand::GodexPack(render));
    }
    if job.label_kind.trim().eq_ignore_ascii_case("progress") {
        return build_godex_progress_command(job);
    }

    let content =
        build_pack_label_content(&job, "Accord", "5kg").map_err(PrintAdapterError::BuildCommand)?;
    let render = build_pack_render(&content, LabelOptions::default_pack())
        .map_err(PrintAdapterError::BuildCommand)?;
    Ok(PrintCommand::GodexPack(render))
}

fn build_godex_progress_command(job: CorePrintJob) -> Result<PrintCommand, PrintAdapterError> {
    let content =
        build_progress_label_content(&job, "Accord").map_err(PrintAdapterError::BuildCommand)?;
    let render = build_progress_pack_render(&content, LabelOptions::default_pack())
        .map_err(PrintAdapterError::BuildCommand)?;
    Ok(PrintCommand::GodexPack(render))
}

fn build_zebra_command(job: CorePrintJob) -> Result<PrintCommand, PrintAdapterError> {
    if is_qolip_cell_label(&job.label_kind) {
        let command = build_qolip_cell_qr_command(&job.epc, &job.item_name)
            .map_err(PrintAdapterError::BuildCommand)?;
        return Ok(PrintCommand::ZebraZpl(command));
    }
    if is_qolip_code_label(&job.label_kind) {
        let command = build_qolip_code_qr_command(&job.epc, &job.item_name, &job.item_code)
            .map_err(PrintAdapterError::BuildCommand)?;
        return Ok(PrintCommand::ZebraZpl(command));
    }
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

fn is_qolip_cell_label(label_kind: &str) -> bool {
    matches!(
        label_kind.trim().to_ascii_lowercase().as_str(),
        "qolip_cell" | "qr_center"
    )
}

fn is_qolip_code_label(label_kind: &str) -> bool {
    label_kind.trim().eq_ignore_ascii_case("qolip_code")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{PrintSelection, QuantitySource, validate_core_print_job};

    fn plan(mode: PrintMode, printer: Option<PrinterKind>) -> CorePrintPlan {
        let job = CorePrintJob::from_selection(
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
        );
        validate_core_print_job(job).unwrap()
    }

    fn unwrap_zebra(command: PrintCommand) -> String {
        match command {
            PrintCommand::ZebraZpl(command) => command,
            PrintCommand::GodexPack(_) => panic!("expected zebra zpl command"),
        }
    }

    #[test]
    fn builds_zebra_rfid_command_from_core_plan() {
        let command = unwrap_zebra(
            build_print_command(plan(PrintMode::Rfid, Some(PrinterKind::Zebra))).unwrap(),
        );

        assert!(command.contains("^RS8,,,1,N"));
        assert!(command.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
        assert!(command.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(command.contains("^FDBRUTTO: 2.5 kg^FS"));
        assert!(command.contains("^FDMAHSULOT: Green Tea^FS"));
    }

    #[test]
    fn builds_zebra_label_only_command_from_core_plan() {
        let command = unwrap_zebra(
            build_print_command(plan(PrintMode::LabelOnly, Some(PrinterKind::Zebra))).unwrap(),
        );

        assert!(command.contains("^MMT"));
        assert!(!command.contains("^RFW"));
        assert!(!command.contains("^RS8"));
        assert!(command.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(command.contains("^FDBRUTTO: 2.5 kg^FS"));
    }

    #[test]
    fn defaults_missing_printer_to_zebra_like_gscale_backend() {
        let command = unwrap_zebra(build_print_command(plan(PrintMode::Rfid, None)).unwrap());

        assert!(command.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
    }

    #[test]
    fn builds_godex_pack_render_from_core_plan() {
        let PrintCommand::GodexPack(render) =
            build_print_command(plan(PrintMode::LabelOnly, Some(PrinterKind::Godex))).unwrap()
        else {
            panic!("expected godex pack render");
        };

        assert_eq!(render.commands[11], "Y0,0,TEXTLBL");
        assert!(
            !render
                .commands
                .iter()
                .any(|command| command.starts_with("AB,") && command.contains("EPC:"))
        );
        assert_eq!(
            render
                .commands
                .iter()
                .find(|command| command.starts_with("BA,"))
                .unwrap(),
            "BA,0,24,1,2,42,0,0,3034257BF7194E406994036B"
        );
        assert!(
            render
                .commands
                .iter()
                .any(|command| command == "Y224,224,QRLBL")
        );
        assert_eq!(render.qr_payload, "3034257BF7194E406994036B");
        assert_eq!(render.qr_box_dots, 144);
    }

    #[test]
    fn builds_godex_progress_render_without_pack_weight_labels() {
        let mut plan = plan(PrintMode::LabelOnly, Some(PrinterKind::Godex));
        plan.job.label_kind = "progress".to_string();
        plan.job.executor_name = "Ali".to_string();
        plan.job.item_name = "Vesta yarim tayyor, 7 ta rangli pechat holatda, pauza".to_string();
        plan.job.net_qty = 120.0;
        plan.job.gross_qty = 17.0;
        plan.job.unit = "kg".to_string();
        plan.job.progress_qty = Some(120.0);
        plan.job.progress_unit = "m".to_string();
        let PrintCommand::GodexPack(render) = build_print_command(plan).unwrap() else {
            panic!("expected godex pack render");
        };

        assert!(
            render
                .commands
                .iter()
                .any(|command| command == "Y0,0,TEXTLBL")
        );
        assert!(
            !render
                .commands
                .iter()
                .any(|command| command.starts_with("AB,") && command.contains("EPC:"))
        );
        assert!(
            render
                .commands
                .iter()
                .any(|command| command.contains("METRAJ: 120 M"))
        );
        assert!(
            render
                .commands
                .iter()
                .any(|command| command.contains("KG: 17"))
        );
        assert!(
            !render
                .commands
                .iter()
                .any(|command| command.contains("NETTO:") || command.contains("BRUTTO:"))
        );
    }

    #[test]
    fn builds_qolip_cell_render_for_godex_and_zebra() {
        let mut godex_plan = plan(PrintMode::LabelOnly, Some(PrinterKind::Godex));
        godex_plan.job.label_kind = "qolip_cell".to_string();
        godex_plan.job.item_name = "A1".to_string();
        let PrintCommand::GodexPack(render) = build_print_command(godex_plan).unwrap() else {
            panic!("expected godex cell render");
        };
        assert!(render.commands.iter().any(|command| command == "Y56,96,QRLBL"));
        assert!(render.commands.iter().any(|command| command == "Y0,0,TEXTLBL"));

        let mut zebra_plan = plan(PrintMode::Rfid, Some(PrinterKind::Zebra));
        zebra_plan.job.label_kind = "qolip_cell".to_string();
        zebra_plan.job.item_name = "A1".to_string();
        let PrintCommand::ZebraZpl(zpl) = build_print_command(zebra_plan).unwrap() else {
            panic!("expected zebra cell render");
        };
        assert!(zpl.contains("^BQN,2,9"));
        assert!(!zpl.contains("^RFW"));
    }
}
