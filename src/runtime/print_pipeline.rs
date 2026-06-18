use std::fmt;

use crate::core::{CorePrintPlan, CorePrintPlanError, PrintSelection, plan_core_print};
use crate::print::adapter::{PrintAdapterError, PrintCommand, build_print_command};
use crate::print::executor::{PrintExecutionError, PrintExecutionResult, PrinterExecutor};
use crate::scale::Reading;

#[derive(Clone, Debug, PartialEq)]
pub struct PrintPipelineResult {
    pub plan: CorePrintPlan,
    pub command: PrintCommand,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrintPipelineError {
    Core(CorePrintPlanError),
    Adapter(PrintAdapterError),
    Execution(PrintExecutionError),
}

impl fmt::Display for PrintPipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => write!(f, "{error}"),
            Self::Adapter(error) => write!(f, "{error}"),
            Self::Execution(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PrintPipelineError {}

pub fn prepare_print_command(
    reading: &Reading,
    selection: PrintSelection,
    epc: &str,
) -> Result<PrintPipelineResult, PrintPipelineError> {
    prepare_print_command_with_label_meta(reading, selection, epc, "", "")
}

pub fn prepare_print_command_with_label_meta(
    reading: &Reading,
    selection: PrintSelection,
    epc: &str,
    label_kind: &str,
    executor_name: &str,
) -> Result<PrintPipelineResult, PrintPipelineError> {
    prepare_print_command_with_progress_label_meta(
        reading,
        selection,
        epc,
        label_kind,
        executor_name,
        None,
        "",
    )
}

pub fn prepare_print_command_with_progress_label_meta(
    reading: &Reading,
    selection: PrintSelection,
    epc: &str,
    label_kind: &str,
    executor_name: &str,
    progress_qty: Option<f64>,
    progress_unit: &str,
) -> Result<PrintPipelineResult, PrintPipelineError> {
    let mut plan = plan_core_print(reading, selection, epc).map_err(PrintPipelineError::Core)?;
    plan.job.label_kind = label_kind.trim().to_ascii_lowercase();
    plan.job.executor_name = executor_name.trim().to_string();
    plan.job.progress_qty = progress_qty;
    plan.job.progress_unit = progress_unit.trim().to_string();
    let command = build_print_command(plan.clone()).map_err(PrintPipelineError::Adapter)?;

    Ok(PrintPipelineResult { plan, command })
}

pub fn execute_prepared_print<E: PrinterExecutor>(
    executor: &mut E,
    prepared: &PrintPipelineResult,
) -> Result<PrintExecutionResult, PrintPipelineError> {
    executor
        .execute(&prepared.command)
        .map_err(PrintPipelineError::Execution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::QuantitySource;
    use crate::print::adapter::PrintCommand;
    use crate::print::executor::{
        GodexExecutor, PrintExecutionError, ZebraExecutor, ZebraTransport,
    };
    use crate::print::godex::{GodexExecutionError, GodexTransport};
    use crate::print::mode::PrintMode;
    use crate::print::printer::PrinterKind;
    use crate::scale::Reading;

    fn selection(printer: &str, mode: PrintMode) -> PrintSelection {
        PrintSelection {
            item_code: "ITEM-1".to_string(),
            item_name: "Green Tea".to_string(),
            warehouse: "Stores - A".to_string(),
            print_mode: mode,
            printer: printer.to_string(),
            quantity_source: QuantitySource::Scale,
            manual_qty_kg: 0.0,
            tare_enabled: true,
            tare_kg: 0.78,
        }
    }

    fn reading(weight: f64) -> Reading {
        Reading::serial("/dev/ttyUSB0", 9600, "kg").with_weight(weight, Some(true), "raw")
    }

    #[test]
    fn prepares_zebra_rfid_command_from_scale_reading() {
        let result = prepare_print_command(
            &reading(2.5),
            selection("zebra", PrintMode::Rfid),
            "3034257BF7194E406994036B",
        )
        .unwrap();

        assert_eq!(result.plan.printer, PrinterKind::Zebra);
        let PrintCommand::ZebraZpl(command) = result.command else {
            panic!("expected zebra zpl command");
        };
        assert!(command.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
        assert!(command.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(command.contains("^FDBRUTTO: 2.5 kg^FS"));
    }

    #[test]
    fn prepares_godex_pack_render_from_scale_reading() {
        let result = prepare_print_command(
            &reading(2.5),
            selection("godex", PrintMode::LabelOnly),
            "3034257BF7194E406994036B",
        )
        .unwrap();

        assert_eq!(result.plan.printer, PrinterKind::Godex);
        let PrintCommand::GodexPack(render) = result.command else {
            panic!("expected godex pack render");
        };
        assert_eq!(render.qr_payload, "3034257BF7194E406994036B");
        assert_eq!(render.commands[11], "Y0,0,TEXTLBL");
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
                .any(|command| command == "Y224,224,QRLBL")
        );
    }

    #[test]
    fn rejects_godex_rfid_before_adapter_runs() {
        let err = prepare_print_command(
            &reading(2.5),
            selection("godex", PrintMode::Rfid),
            "3034257BF7194E406994036B",
        )
        .unwrap_err();

        assert!(matches!(err, PrintPipelineError::Core(_)));
        assert_eq!(err.to_string(), "godex does not support rfid");
    }

    #[test]
    fn rejects_missing_required_fields_from_core_plan() {
        let err = prepare_print_command(&reading(2.5), selection("zebra", PrintMode::Rfid), " ")
            .unwrap_err();

        assert!(matches!(err, PrintPipelineError::Core(_)));
        assert_eq!(
            err.to_string(),
            "zebra print job missing required fields: epc"
        );
    }

    #[derive(Default)]
    struct MockZebraTransport {
        sent: Vec<String>,
    }

    impl ZebraTransport for MockZebraTransport {
        fn send_zpl(&mut self, zpl: &str) -> Result<String, PrintExecutionError> {
            self.sent.push(zpl.to_string());
            Ok("OK".to_string())
        }
    }

    #[derive(Default)]
    struct MockGodexTransport {
        calls: Vec<String>,
    }

    impl GodexTransport for MockGodexTransport {
        fn send(
            &mut self,
            command: &str,
            read: bool,
            _pause: std::time::Duration,
        ) -> Result<String, GodexExecutionError> {
            self.calls.push(format!("send:{command}:read={read}"));
            if command == "~S,STATUS" {
                return Ok("00,OK".to_string());
            }
            Ok(String::new())
        }

        fn write_raw(&mut self, payload: &[u8]) -> Result<(), GodexExecutionError> {
            self.calls.push(format!("raw:{}", payload.len()));
            Ok(())
        }
    }

    #[test]
    fn executes_prepared_zebra_print_without_replanning() {
        let prepared = prepare_print_command(
            &reading(2.5),
            selection("zebra", PrintMode::Rfid),
            "3034257BF7194E406994036B",
        )
        .unwrap();
        let mut executor = ZebraExecutor::new(MockZebraTransport::default());

        let result = execute_prepared_print(&mut executor, &prepared).unwrap();

        assert_eq!(result.printer, PrinterKind::Zebra);
        assert_eq!(result.status, "OK");
        assert!(executor.transport_mut().sent[0].contains("^RFW,H,,,A^FD"));
    }

    #[test]
    fn executes_prepared_godex_print_without_replanning() {
        let prepared = prepare_print_command(
            &reading(2.5),
            selection("godex", PrintMode::LabelOnly),
            "3034257BF7194E406994036B",
        )
        .unwrap();
        let mut executor = GodexExecutor::new(MockGodexTransport::default());

        let result = execute_prepared_print(&mut executor, &prepared).unwrap();

        assert_eq!(result.printer, PrinterKind::Godex);
        assert_eq!(result.status, "00,OK");
        assert!(
            executor
                .transport_mut()
                .calls
                .iter()
                .any(|call| call == "send:~S,STATUS:read=true")
        );
    }
}
