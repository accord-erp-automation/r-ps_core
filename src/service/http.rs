use std::sync::Arc;

use serde::Serialize;

use super::config::MobileServiceConfig;
use super::driver_print_contract::{
    DriverPrintErrorResponse, DriverPrintRequest, DriverPrintResponse,
};
use super::driver_print_runtime::{DriverPrintExecutor, UnconfiguredDriverPrintExecutor};
use super::mobile_contract::{
    EmptyArchiveResponse, EmptyItemsResponse, EmptyWarehousesResponse, HandshakeResponse,
    HealthResponse, ItemWarehousesResponse, PrinterCapabilitiesResponse, ServiceIdentity,
    SetupStatusResponse,
};
use super::monitor_contract::BatchStateResponse;
use super::monitor_runtime::MonitorRuntimeState;
use super::print_activity::PrintActivityState;
use crate::print::capabilities::manifest_for;
use crate::print::printer::PrinterKind;
use crate::runtime::prepare_print_command;

#[derive(Clone, Debug)]
pub struct MobileHttpState {
    pub identity: ServiceIdentity,
    pub http_port: u16,
    pub candidate_ports: Vec<u16>,
    pub active_printer: PrinterKind,
    pub monitor: MonitorRuntimeState,
    pub print_executor: Arc<dyn DriverPrintExecutor>,
    pub print_activity: PrintActivityState,
}

impl MobileHttpState {
    pub fn new(
        identity: ServiceIdentity,
        http_port: u16,
        candidate_ports: Vec<u16>,
        active_printer: PrinterKind,
        monitor: MonitorRuntimeState,
    ) -> Self {
        Self {
            identity,
            http_port,
            candidate_ports,
            active_printer,
            monitor,
            print_executor: Arc::new(UnconfiguredDriverPrintExecutor),
            print_activity: PrintActivityState::default(),
        }
    }

    pub fn from_config(
        config: &MobileServiceConfig,
        identity: ServiceIdentity,
        active_printer: PrinterKind,
        monitor: MonitorRuntimeState,
    ) -> Self {
        Self::new(
            identity,
            config.http_port(),
            config.candidate_ports.clone(),
            active_printer,
            monitor,
        )
    }

    pub fn with_print_executor(mut self, executor: Arc<dyn DriverPrintExecutor>) -> Self {
        self.print_executor = executor;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileHttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl MobileHttpResponse {
    pub fn json<T: Serialize>(status: u16, value: &T) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: serde_json::to_vec(value)
                .unwrap_or_else(|_| b"{\"error\":\"json_encode\"}".to_vec()),
        }
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobileHttpErrorResponse {
    pub error: &'static str,
}

impl Serialize for MobileHttpErrorResponse {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("MobileHttpErrorResponse", 1)?;
        state.serialize_field("error", self.error)?;
        state.end()
    }
}

pub fn handle_mobile_http_request(
    state: &MobileHttpState,
    method: &str,
    path: &str,
) -> MobileHttpResponse {
    handle_mobile_http_request_with_body(state, method, path, "")
}

pub fn handle_mobile_http_request_with_body(
    state: &MobileHttpState,
    method: &str,
    path: &str,
    body: &str,
) -> MobileHttpResponse {
    let method = method.trim().to_ascii_uppercase();
    let path = normalize_path(path);

    match (method.as_str(), path.as_str()) {
        ("GET", "/healthz") => {
            MobileHttpResponse::json(200, &HealthResponse::ok(state.print_activity.snapshot()))
        }
        ("GET", "/v1/mobile/handshake") => {
            let handshake = HandshakeResponse::new(
                &state.identity,
                state.http_port,
                state.candidate_ports.clone(),
                state.print_activity.snapshot(),
            );
            MobileHttpResponse::json(200, &handshake)
        }
        ("GET", "/v1/mobile/printer/capabilities") => {
            let response =
                PrinterCapabilitiesResponse::from_manifest(manifest_for(state.active_printer));
            MobileHttpResponse::json(200, &response)
        }
        ("GET", "/v1/mobile/monitor/state") => {
            let response = state
                .monitor
                .snapshot(&state.identity, state.active_printer);
            MobileHttpResponse::json(200, &response)
        }
        ("GET", "/v1/mobile/setup/status") => {
            MobileHttpResponse::json(200, &SetupStatusResponse::driver_scope())
        }
        ("POST", "/v1/mobile/setup/warehouse") => {
            MobileHttpResponse::json(200, &SetupStatusResponse::driver_scope())
        }
        ("GET", "/v1/mobile/items") => {
            MobileHttpResponse::json(200, &EmptyItemsResponse::driver_scope())
        }
        ("GET", "/v1/mobile/warehouses") => {
            MobileHttpResponse::json(200, &EmptyWarehousesResponse::driver_scope())
        }
        ("GET", "/v1/mobile/archive") => {
            MobileHttpResponse::json(200, &EmptyArchiveResponse::driver_scope())
        }
        ("GET", path) if is_item_warehouses_path(path) => {
            let item_code = extract_item_code_from_warehouses_path(path).unwrap_or_default();
            MobileHttpResponse::json(200, &ItemWarehousesResponse::driver_scope(&item_code))
        }
        ("GET", "/v1/mobile/batch/state") => {
            MobileHttpResponse::json(200, &BatchStateResponse::inactive(state.active_printer))
        }
        ("POST", "/v1/driver/print") | ("POST", "/v1/mobile/driver/print") => {
            driver_print_response(state, body)
        }
        ("POST", "/v1/mobile/batch/start")
        | ("POST", "/v1/mobile/batch/stop")
        | ("POST", "/v1/mobile/batch/manual-print") => driver_batch_not_supported(),
        ("GET", "/v1/mobile/batch/manual-print")
        | ("GET", "/v1/driver/print")
        | ("GET", "/v1/mobile/driver/print")
        | ("GET", "/v1/mobile/batch/start")
        | ("GET", "/v1/mobile/batch/stop") => MobileHttpResponse::json(
            405,
            &MobileHttpErrorResponse {
                error: "method_not_allowed",
            },
        ),
        (_, "/v1/mobile/batch/start")
        | (_, "/v1/mobile/batch/stop")
        | (_, "/v1/driver/print")
        | (_, "/v1/mobile/driver/print")
        | (_, "/v1/mobile/batch/manual-print") => MobileHttpResponse::json(
            405,
            &MobileHttpErrorResponse {
                error: "method_not_allowed",
            },
        ),
        (_, path) if is_item_warehouses_path(path) => MobileHttpResponse::json(
            405,
            &MobileHttpErrorResponse {
                error: "method_not_allowed",
            },
        ),
        (_, "/healthz")
        | (_, "/v1/mobile/handshake")
        | (_, "/v1/mobile/printer/capabilities")
        | (_, "/v1/mobile/monitor/state")
        | (_, "/v1/mobile/setup/status")
        | (_, "/v1/mobile/setup/warehouse")
        | (_, "/v1/mobile/items")
        | (_, "/v1/mobile/warehouses")
        | (_, "/v1/mobile/archive")
        | (_, "/v1/mobile/batch/state") => MobileHttpResponse::json(
            405,
            &MobileHttpErrorResponse {
                error: "method_not_allowed",
            },
        ),
        _ => MobileHttpResponse::json(404, &MobileHttpErrorResponse { error: "not_found" }),
    }
}

fn driver_print_response(state: &MobileHttpState, body: &str) -> MobileHttpResponse {
    let request = match DriverPrintRequest::from_json(body) {
        Ok(request) => request,
        Err(err) => {
            eprintln!("rp-scale driver print rejected: error={}", err.code());
            return MobileHttpResponse::json(
                err.status(),
                &DriverPrintErrorResponse::new(err.code(), err.code()),
            );
        }
    };
    println!(
        "rp-scale driver print request: epc={:?} item_code={:?} item_name={:?} warehouse={:?} printer={:?} print_mode={:?} mode={:?} gross_qty={:?} qty={:?} manual_qty_kg={:?} unit={:?} tare_enabled={} tare={} tare_kg={:.3}",
        request.epc,
        request.item_code,
        request.item_name,
        request.warehouse,
        request.printer,
        request.print_mode,
        request.mode,
        request.gross_qty,
        request.qty,
        request.manual_qty_kg,
        request.unit,
        request.tare_enabled,
        request.tare,
        request.tare_kg
    );

    let job = match request.into_job(state.active_printer) {
        Ok(job) => job,
        Err(err) => {
            eprintln!(
                "rp-scale driver print rejected: error={} active_printer={}",
                err.code(),
                state.active_printer.as_str()
            );
            return MobileHttpResponse::json(
                err.status(),
                &DriverPrintErrorResponse::new(err.code(), err.code()),
            );
        }
    };
    println!(
        "rp-scale driver print accepted: epc={} item_code={:?} item_name={:?} warehouse={:?} printer={} mode={} gross_qty={:.3} net_qty={:.3} unit={}",
        job.epc,
        job.selection.item_code,
        job.selection.item_name,
        job.warehouse,
        job.selection.printer,
        job.selection.print_mode.as_str(),
        job.reading.weight.unwrap_or(0.0),
        job.reading.weight.unwrap_or(0.0),
        job.reading.unit
    );

    let prepared = match prepare_print_command(&job.reading, job.selection.clone(), &job.epc) {
        Ok(prepared) => prepared,
        Err(err) => {
            eprintln!(
                "rp-scale driver print prepare failed: epc={} item_code={:?} qty={:.3} error={}",
                job.epc,
                job.selection.item_code,
                job.reading.weight.unwrap_or(0.0),
                err
            );
            return MobileHttpResponse::json(
                422,
                &DriverPrintErrorResponse::new("print_prepare_failed", err.to_string()),
            );
        }
    };

    let _activity_guard = match state.print_activity.try_start(
        &job.epc,
        &job.selection.item_code,
        &job.selection.item_name,
        job.selection.printer.as_str(),
    ) {
        Ok(guard) => guard,
        Err(snapshot) => {
            eprintln!(
                "rp-scale driver print busy: epc={} item_code={:?} qty={:.3}",
                job.epc,
                job.selection.item_code,
                job.reading.weight.unwrap_or(0.0)
            );
            return MobileHttpResponse::json(409, &DriverPrintErrorResponse::busy(snapshot));
        }
    };

    let mut last_status = String::new();
    for _ in 0..job.print_count {
        match state.print_executor.execute(&prepared) {
            Ok(result) => {
                last_status = result.status;
            }
            Err(err) => {
                eprintln!(
                    "rp-scale driver print execution failed: epc={} item_code={:?} qty={:.3} error={} detail={}",
                    job.epc,
                    job.selection.item_code,
                    job.reading.weight.unwrap_or(0.0),
                    err.code(),
                    err.detail()
                );
                return MobileHttpResponse::json(
                    err.status(),
                    &DriverPrintErrorResponse::new(err.code(), err.detail()),
                );
            }
        }
    }
    println!(
        "rp-scale driver print done: epc={} item_code={:?} item_name={:?} warehouse={:?} printer={} mode={} gross_qty={:.3} net_qty={:.3} status={}",
        job.epc,
        job.selection.item_code,
        job.selection.item_name,
        job.warehouse,
        prepared.plan.printer.as_str(),
        prepared.plan.job.mode.as_str(),
        prepared.plan.job.gross_qty,
        prepared.plan.job.net_qty,
        last_status
    );
    MobileHttpResponse::json(
        200,
        &DriverPrintResponse::done(&job, &prepared, last_status),
    )
}

fn driver_batch_not_supported() -> MobileHttpResponse {
    MobileHttpResponse::json(
        409,
        &MobileHttpErrorResponse {
            error: "driver_batch_not_supported",
        },
    )
}

fn is_item_warehouses_path(path: &str) -> bool {
    extract_item_code_from_warehouses_path(path).is_some()
}

fn extract_item_code_from_warehouses_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/v1/mobile/items/")?;
    let (item_code, suffix) = rest.rsplit_once('/')?;
    if suffix != "warehouses" || item_code.trim().is_empty() {
        return None;
    }
    Some(percent_decode(item_code))
}

fn percent_decode(value: &str) -> String {
    let mut out = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
            && let Ok(decoded) = u8::from_str_radix(hex, 16)
        {
            out.push(decoded);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalize_path(path: &str) -> String {
    let path = path.trim();
    let path = path.split_once('?').map(|(path, _)| path).unwrap_or(path);
    match path {
        "" => "/".to_string(),
        value if value.starts_with('/') => value.to_string(),
        value => format!("/{value}"),
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
