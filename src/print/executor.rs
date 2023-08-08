use std::fmt;

use super::adapter::PrintCommand;
use super::godex::{GodexTransport, execute_pack_render};
use super::printer::PrinterKind;

#[derive(Clone, Debug, PartialEq)]
pub struct PrintExecutionResult {
    pub printer: PrinterKind,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrintExecutionError {
    UnsupportedCommand {
        executor: PrinterKind,
        command: &'static str,
    },
    Transport(String),
}

impl fmt::Display for PrintExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCommand { executor, command } => {
                write!(f, "{} executor cannot run {command}", executor.as_str())
            }
            Self::Transport(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PrintExecutionError {}

pub trait PrinterExecutor {
    fn printer(&self) -> PrinterKind;

    fn execute(
        &mut self,
        command: &PrintCommand,
    ) -> Result<PrintExecutionResult, PrintExecutionError>;
}

pub trait ZebraTransport {
    fn send_zpl(&mut self, zpl: &str) -> Result<String, PrintExecutionError>;
}

pub struct ZebraExecutor<T> {
    transport: T,
}

impl<T> ZebraExecutor<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

impl<T: ZebraTransport> PrinterExecutor for ZebraExecutor<T> {
    fn printer(&self) -> PrinterKind {
        PrinterKind::Zebra
    }

    fn execute(
        &mut self,
        command: &PrintCommand,
    ) -> Result<PrintExecutionResult, PrintExecutionError> {
        let PrintCommand::ZebraZpl(zpl) = command else {
            return Err(PrintExecutionError::UnsupportedCommand {
                executor: self.printer(),
                command: command_name(command),
            });
        };

        let status = self.transport.send_zpl(zpl)?;
        Ok(PrintExecutionResult {
            printer: self.printer(),
            status,
        })
    }
}

pub struct GodexExecutor<T> {
    transport: T,
}

impl<T> GodexExecutor<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

impl<T: GodexTransport> PrinterExecutor for GodexExecutor<T> {
    fn printer(&self) -> PrinterKind {
        PrinterKind::Godex
    }

    fn execute(
        &mut self,
        command: &PrintCommand,
    ) -> Result<PrintExecutionResult, PrintExecutionError> {
        let PrintCommand::GodexPack(render) = command else {
            return Err(PrintExecutionError::UnsupportedCommand {
                executor: self.printer(),
                command: command_name(command),
            });
        };

        let status = execute_pack_render(&mut self.transport, render)
            .map_err(|err| PrintExecutionError::Transport(err.to_string()))?;
        Ok(PrintExecutionResult {
            printer: self.printer(),
            status,
        })
    }
}

fn command_name(command: &PrintCommand) -> &'static str {
    match command {
        PrintCommand::ZebraZpl(_) => "zebra_zpl",
        PrintCommand::GodexPack(_) => "godex_pack",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::core::{CorePrintJob, PrintSelection, QuantitySource, validate_core_print_job};
    use crate::print::adapter::build_print_command;
    use crate::print::godex::GodexExecutionError;
    use crate::print::mode::PrintMode;
    use crate::print::printer::PrinterKind;

    #[derive(Default)]
    struct MockZebraTransport {
        sent: Vec<String>,
        fail: bool,
    }

    impl ZebraTransport for MockZebraTransport {
        fn send_zpl(&mut self, zpl: &str) -> Result<String, PrintExecutionError> {
            self.sent.push(zpl.to_string());
            if self.fail {
                return Err(PrintExecutionError::Transport("zebra offline".to_string()));
            }
            Ok("OK".to_string())
        }
    }

    #[derive(Default)]
    struct MockGodexTransport {
        calls: Vec<String>,
        fail_status: bool,
    }

    impl GodexTransport for MockGodexTransport {
        fn send(
            &mut self,
            command: &str,
            read: bool,
            _pause: Duration,
        ) -> Result<String, GodexExecutionError> {
            self.calls.push(format!("send:{command}:read={read}"));
            if self.fail_status && command == "~S,STATUS" {
                return Err(GodexExecutionError::new("godex timeout"));
            }
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

    fn command(printer: PrinterKind, mode: PrintMode) -> PrintCommand {
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
                printer: printer.as_str().to_string(),
                quantity_source: QuantitySource::Scale,
                manual_qty_kg: 0.0,
                tare_enabled: true,
                tare_kg: 0.78,
            },
        );
        build_print_command(validate_core_print_job(job).unwrap()).unwrap()
    }

    #[test]
    fn zebra_executor_sends_zpl_command() {
        let mut executor = ZebraExecutor::new(MockZebraTransport::default());
        let result = executor
            .execute(&command(PrinterKind::Zebra, PrintMode::Rfid))
            .unwrap();

        assert_eq!(result.printer, PrinterKind::Zebra);
        assert_eq!(result.status, "OK");
        assert!(executor.transport_mut().sent[0].contains("^RFW,H,,,A^FD"));
    }

    #[test]
    fn godex_executor_runs_pack_render_sequence() {
        let mut executor = GodexExecutor::new(MockGodexTransport::default());
        let result = executor
            .execute(&command(PrinterKind::Godex, PrintMode::LabelOnly))
            .unwrap();

        assert_eq!(result.printer, PrinterKind::Godex);
        assert_eq!(result.status, "00,OK");
        assert_eq!(
            executor.transport_mut().calls[0],
            "send:^XSET,BUZZER,0:read=false"
        );
        assert!(
            executor
                .transport_mut()
                .calls
                .iter()
                .any(|call| call == "send:~S,STATUS:read=true")
        );
    }

    #[test]
    fn rejects_wrong_command_for_executor() {
        let mut executor = ZebraExecutor::new(MockZebraTransport::default());
        let err = executor
            .execute(&command(PrinterKind::Godex, PrintMode::LabelOnly))
            .unwrap_err();

        assert_eq!(
            err,
            PrintExecutionError::UnsupportedCommand {
                executor: PrinterKind::Zebra,
                command: "godex_pack",
            }
        );
    }

    #[test]
    fn maps_transport_errors() {
        let mut executor = GodexExecutor::new(MockGodexTransport {
            fail_status: true,
            ..Default::default()
        });
        let err = executor
            .execute(&command(PrinterKind::Godex, PrintMode::LabelOnly))
            .unwrap_err();

        assert_eq!(err.to_string(), "final status: godex timeout");
    }
}
