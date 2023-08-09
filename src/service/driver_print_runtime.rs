use std::fmt;

use crate::print::PrintExecutionResult;
use crate::runtime::PrintPipelineResult;

pub trait DriverPrintExecutor: fmt::Debug + Send + Sync {
    fn execute(
        &self,
        prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError>;
}

#[derive(Clone, Debug, Default)]
pub struct UnconfiguredDriverPrintExecutor;

impl DriverPrintExecutor for UnconfiguredDriverPrintExecutor {
    fn execute(
        &self,
        _prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError> {
        Err(DriverPrintExecutionError::Unavailable(
            "printer_executor_not_configured".to_string(),
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct SimulatedDriverPrintExecutor;

impl DriverPrintExecutor for SimulatedDriverPrintExecutor {
    fn execute(
        &self,
        prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError> {
        Ok(PrintExecutionResult {
            printer: prepared.plan.printer,
            status: "simulated".to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DriverPrintExecutionError {
    Unavailable(String),
    Failed(String),
}

impl DriverPrintExecutionError {
    pub fn status(&self) -> u16 {
        match self {
            Self::Unavailable(_) => 503,
            Self::Failed(_) => 500,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable(_) => "printer_executor_not_configured",
            Self::Failed(_) => "print_execution_failed",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::Unavailable(detail) | Self::Failed(detail) => detail,
        }
    }
}

impl fmt::Display for DriverPrintExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.detail())
    }
}

impl std::error::Error for DriverPrintExecutionError {}

pub fn simulated_executor_from_env(value: &str) -> Option<SimulatedDriverPrintExecutor> {
    match value.trim().to_ascii_lowercase().as_str() {
        "simulated" | "simulate" | "dry-run" | "dry_run" => Some(SimulatedDriverPrintExecutor),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{PrintSelection, QuantitySource};
    use crate::print::{PrintMode, PrinterKind};
    use crate::runtime::prepare_print_command;
    use crate::scale::Reading;

    fn prepared() -> PrintPipelineResult {
        let reading = Reading::serial("/dev/tty.sim", 9600, "kg").with_weight(
            1.25,
            Some(true),
            "1.250 kg ST",
        );
        let selection = PrintSelection {
            item_code: "ITEM-1".to_string(),
            item_name: "Green Tea".to_string(),
            warehouse: "Stores - A".to_string(),
            print_mode: PrintMode::Rfid,
            printer: "zebra".to_string(),
            quantity_source: QuantitySource::Scale,
            manual_qty_kg: 0.0,
            tare_enabled: false,
            tare_kg: 0.0,
        };
        prepare_print_command(&reading, selection, "3034257BF7194E406994036B").unwrap()
    }

    #[test]
    fn unconfigured_executor_fails_closed() {
        let err = UnconfiguredDriverPrintExecutor
            .execute(&prepared())
            .unwrap_err();

        assert_eq!(err.status(), 503);
        assert_eq!(err.code(), "printer_executor_not_configured");
    }

    #[test]
    fn simulated_executor_returns_done_status_without_hardware() {
        let result = SimulatedDriverPrintExecutor.execute(&prepared()).unwrap();

        assert_eq!(result.printer, PrinterKind::Zebra);
        assert_eq!(result.status, "simulated");
    }

    #[test]
    fn parses_simulated_executor_env_aliases() {
        assert!(simulated_executor_from_env("simulate").is_some());
        assert!(simulated_executor_from_env("dry-run").is_some());
        assert!(simulated_executor_from_env("real").is_none());
    }
}
