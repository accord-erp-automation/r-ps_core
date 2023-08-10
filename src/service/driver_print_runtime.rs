use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crate::print::godex::{GodexExecutionError, GodexTransport};
use crate::print::{PrintExecutionError, PrintExecutionResult, PrinterKind, ZebraTransport};
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

#[derive(Debug)]
pub struct DeviceDriverPrintExecutor {
    zebra_device: Option<PathBuf>,
    godex_device: Option<PathBuf>,
    lock: Mutex<()>,
}

impl DeviceDriverPrintExecutor {
    pub fn new(zebra_device: Option<PathBuf>, godex_device: Option<PathBuf>) -> Self {
        Self {
            zebra_device,
            godex_device,
            lock: Mutex::new(()),
        }
    }
}

impl DriverPrintExecutor for DeviceDriverPrintExecutor {
    fn execute(
        &self,
        prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError> {
        let _guard = self.lock.lock().map_err(|_| {
            DriverPrintExecutionError::Failed("printer executor lock poisoned".to_string())
        })?;

        match prepared.plan.printer {
            PrinterKind::Zebra => {
                let device = self.zebra_device.as_ref().ok_or_else(|| {
                    DriverPrintExecutionError::Unavailable(
                        "zebra_device_not_configured".to_string(),
                    )
                })?;
                let mut executor = crate::print::ZebraExecutor::new(FileZebraTransport {
                    path: device.clone(),
                });
                crate::runtime::execute_prepared_print(&mut executor, prepared)
                    .map_err(|err| DriverPrintExecutionError::Failed(err.to_string()))
            }
            PrinterKind::Godex => {
                let device = self.godex_device.as_ref().ok_or_else(|| {
                    DriverPrintExecutionError::Unavailable(
                        "godex_device_not_configured".to_string(),
                    )
                })?;
                let mut executor = crate::print::GodexExecutor::new(FileGodexTransport {
                    path: device.clone(),
                });
                crate::runtime::execute_prepared_print(&mut executor, prepared)
                    .map_err(|err| DriverPrintExecutionError::Failed(err.to_string()))
            }
        }
    }
}

#[derive(Debug)]
struct FileZebraTransport {
    path: PathBuf,
}

impl ZebraTransport for FileZebraTransport {
    fn send_zpl(&mut self, zpl: &str) -> Result<String, PrintExecutionError> {
        write_device_payload(&self.path, zpl.as_bytes())
            .map_err(|err| PrintExecutionError::Transport(err.to_string()))?;
        Ok("sent".to_string())
    }
}

#[derive(Debug)]
struct FileGodexTransport {
    path: PathBuf,
}

impl GodexTransport for FileGodexTransport {
    fn send(
        &mut self,
        command: &str,
        read: bool,
        pause: Duration,
    ) -> Result<String, GodexExecutionError> {
        write_device_payload(&self.path, &encode_godex_command(command))
            .map_err(|err| GodexExecutionError::new(err.to_string()))?;
        if !pause.is_zero() {
            std::thread::sleep(pause);
        }
        if read {
            Ok("sent".to_string())
        } else {
            Ok(String::new())
        }
    }

    fn write_raw(&mut self, payload: &[u8]) -> Result<(), GodexExecutionError> {
        write_device_payload(&self.path, payload)
            .map_err(|err| GodexExecutionError::new(err.to_string()))
    }
}

fn write_device_payload(path: &PathBuf, payload: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().append(true).open(path)?;
    file.write_all(payload)?;
    file.flush()
}

fn encode_godex_command(command: &str) -> Vec<u8> {
    let mut out = command.trim_end_matches(['\r', '\n']).as_bytes().to_vec();
    out.extend_from_slice(b"\r\n");
    out
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
    match print_executor_mode_from_env(value).ok().flatten() {
        Some(PrintExecutorMode::Simulated) => Some(SimulatedDriverPrintExecutor),
        _ => None,
    }
}

pub fn device_executor_from_env(
    value: &str,
    zebra_device: Option<&str>,
    godex_device: Option<&str>,
) -> Option<DeviceDriverPrintExecutor> {
    match print_executor_mode_from_env(value).ok().flatten() {
        Some(PrintExecutorMode::Device) => Some(DeviceDriverPrintExecutor::new(
            normalize_device_path(zebra_device),
            normalize_device_path(godex_device),
        )),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintExecutorMode {
    Simulated,
    Device,
}

pub fn print_executor_mode_from_env(value: &str) -> Result<Option<PrintExecutorMode>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    match value.to_ascii_lowercase().as_str() {
        "simulated" | "simulate" | "dry-run" | "dry_run" => Ok(Some(PrintExecutorMode::Simulated)),
        "real" | "device" | "hardware" | "godex-device" | "godex_device" | "zebra-device"
        | "zebra_device" => Ok(Some(PrintExecutorMode::Device)),
        _ => Err(format!(
            "unsupported RP_SCALE_PRINT_EXECUTOR={value}; expected simulated, dry-run, device, godex-device, or zebra-device"
        )),
    }
}

fn normalize_device_path(value: Option<&str>) -> Option<PathBuf> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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

    #[test]
    fn device_executor_writes_prepared_zebra_command_to_device_path() {
        let path = std::env::temp_dir().join(format!("rp-scale-zebra-test-{}", std::process::id()));
        std::fs::File::create(&path).unwrap();
        let executor = DeviceDriverPrintExecutor::new(Some(path.clone()), None);

        let result = executor.execute(&prepared()).unwrap();
        let payload = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(result.printer, PrinterKind::Zebra);
        assert_eq!(result.status, "sent");
        assert!(payload.contains("^RFW,H,,,A^FD3034257BF7194E406994036B^FS"));
    }

    #[test]
    fn real_executor_env_requires_matching_device_path() {
        assert!(device_executor_from_env("real", Some("/tmp/zebra"), None).is_some());
        assert!(device_executor_from_env("device", None, Some("/tmp/godex")).is_some());
        assert!(device_executor_from_env("godex-device", None, Some("/tmp/godex")).is_some());
        assert!(device_executor_from_env("zebra-device", Some("/tmp/zebra"), None).is_some());
        assert!(device_executor_from_env("simulated", Some("/tmp/zebra"), None).is_none());
    }

    #[test]
    fn executor_env_mode_rejects_unknown_values() {
        let err = print_executor_mode_from_env("godex-printer").unwrap_err();

        assert!(err.contains("unsupported RP_SCALE_PRINT_EXECUTOR"));
        assert!(err.contains("godex-printer"));
    }
}
