use super::*;
use std::sync::{Arc, Mutex};

use crate::print::PrintExecutionResult;
use crate::runtime::PrintPipelineResult;
use crate::service::driver_print_runtime::{DriverPrintExecutionError, DriverPrintExecutor};
use serde_json::Value;

const EPC: &str = "3034257BF7194E406994036B";

fn state(printer: PrinterKind) -> MobileHttpState {
    MobileHttpState::new(
        ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin"),
        39117,
        vec![39117, 41257],
        printer,
        MonitorRuntimeState::default(),
    )
}

fn state_with_executor<E>(printer: PrinterKind, executor: E) -> MobileHttpState
where
    E: DriverPrintExecutor + 'static,
{
    state(printer).with_print_executor(Arc::new(executor))
}

fn json(response: MobileHttpResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap()
}

#[derive(Debug)]
struct AcceptingDriverPrintExecutor;

impl DriverPrintExecutor for AcceptingDriverPrintExecutor {
    fn execute(
        &self,
        prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError> {
        Ok(PrintExecutionResult {
            printer: prepared.plan.printer,
            status: "OK".to_string(),
        })
    }
}

#[derive(Debug)]
struct CountingDriverPrintExecutor {
    calls: Arc<Mutex<Vec<String>>>,
}

impl DriverPrintExecutor for CountingDriverPrintExecutor {
    fn execute(
        &self,
        prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError> {
        self.calls
            .lock()
            .unwrap()
            .push(prepared.plan.job.epc.clone());
        Ok(PrintExecutionResult {
            printer: prepared.plan.printer,
            status: "OK".to_string(),
        })
    }
}

#[derive(Debug)]
struct FailingDriverPrintExecutor;

impl DriverPrintExecutor for FailingDriverPrintExecutor {
    fn execute(
        &self,
        _prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError> {
        Err(DriverPrintExecutionError::Failed(
            "printer offline".to_string(),
        ))
    }
}

#[derive(Debug)]
struct HoldingDriverPrintExecutor;

impl DriverPrintExecutor for HoldingDriverPrintExecutor {
    fn execute(
        &self,
        _prepared: &PrintPipelineResult,
    ) -> Result<PrintExecutionResult, DriverPrintExecutionError> {
        std::thread::sleep(std::time::Duration::from_millis(120));
        Ok(PrintExecutionResult {
            printer: PrinterKind::Godex,
            status: "OK".to_string(),
        })
    }
}

#[test]
fn healthz_matches_gscale_mobile_fallback_contract() {
    let response = handle_mobile_http_request(&state(PrinterKind::Zebra), "GET", "/healthz");
    let body = json(response.clone());

    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "application/json");
    assert_eq!(body["ok"], true);
    assert_eq!(body["service"], "mobileapi");
    assert_eq!(body["busy"], false);
    assert_eq!(body["print_activity"]["busy"], false);
}

#[test]
fn handshake_matches_mobile_discovery_contract() {
    let body = json(handle_mobile_http_request(
        &state(PrinterKind::Zebra),
        "GET",
        "/v1/mobile/handshake",
    ));

    assert_eq!(body["service"], "mobileapi");
    assert_eq!(body["app"], "gscale-zebra");
    assert_eq!(body["server_name"], "rp-scale");
    assert_eq!(body["http_port"], 39117);
    assert_eq!(body["discovery_port"], 18081);
    assert_eq!(body["candidate_ports"][1], 41257);
    assert_eq!(body["requires_auth"], false);
    assert_eq!(body["busy"], false);
    assert_eq!(body["print_activity"]["busy"], false);
}

#[test]
fn printer_capabilities_expose_active_printer_limits() {
    let body = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/printer/capabilities",
    ));

    assert_eq!(body["active_printer"]["id"], "godex");
    assert_eq!(
        body["active_printer"]["capabilities"]["rfid_epc_write"],
        false
    );
    assert_eq!(body["active_printer"]["capabilities"]["qr"], true);
    assert_eq!(
        body["active_printer"]["unsupported_modes"][0],
        "rfid_epc_write"
    );
}

#[test]
fn monitor_state_matches_mobile_snapshot_shape() {
    let body = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/monitor/state",
    ));

    assert_eq!(body["ok"], true);
    assert_eq!(body["profile"]["ref"], "dev-operator");
    assert_eq!(body["state"]["scale"]["weight"], Value::Null);
    assert_eq!(body["state"]["scale"]["unit"], "kg");
    assert_eq!(body["state"]["zebra"]["connected"], false);
    assert_eq!(body["state"]["printer"]["connected"], false);
    assert_eq!(body["state"]["printer"]["kind"], "godex");
    assert_eq!(body["state"]["batch"]["active"], false);
    assert_eq!(body["state"]["print_request"]["status"], "idle");
    assert_eq!(body["printer"]["label"], "ulanmagan");
}

#[test]
fn setup_status_matches_gscale_fields_without_owning_erp() {
    let body = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/setup/status",
    ));

    assert_eq!(body["ok"], true);
    assert_eq!(body["erp_write_configured"], false);
    assert_eq!(body["erp_write_simulated"], false);
    assert_eq!(body["erp_read_configured"], false);
    assert_eq!(body["batch_actions_ready"], false);
    assert_eq!(body["erp_url"], "");
    assert_eq!(body["erp_read_url"], "");
    assert_eq!(body["warehouse_mode"], "manual");
    assert_eq!(body["default_warehouse"], "");
    assert_eq!(body["warehouse_default_configured"], false);
    assert_eq!(body["warehouse_default_active"], false);
}

#[test]
fn batch_state_matches_gscale_shape_with_driver_inactive_batch() {
    let body = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/batch/state",
    ));

    assert_eq!(body["ok"], true);
    assert_eq!(body["batch"]["active"], false);
    assert_eq!(body["batch"]["printer"], "godex");
    assert_eq!(body["batch"]["print_mode"], "label");
    assert_eq!(body["batch"]["quantity_source"], "scale");
    assert_eq!(body["batch"]["total_qty"], 0.0);
}

#[test]
fn batch_start_is_rejected_until_split_contract_exists() {
    let state = state(PrinterKind::Godex);
    let started = handle_mobile_http_request_with_body(
        &state,
        "POST",
        "/v1/mobile/batch/start",
        r#"{
            "item_code":"ITEM-1",
            "item_name":"Sugar",
            "warehouse":"Stores - A",
            "print_mode":"rfid",
            "printer":"godex",
            "quantity_source":"scale",
            "tare_enabled":true,
            "tare_kg":0.25
        }"#,
    );
    let started_body = json(started.clone());
    let current = json(handle_mobile_http_request(
        &state,
        "GET",
        "/v1/mobile/batch/state",
    ));
    let monitor = json(handle_mobile_http_request(
        &state,
        "GET",
        "/v1/mobile/monitor/state",
    ));

    assert_eq!(started.status, 409);
    assert_eq!(started_body["error"], "driver_batch_not_supported");
    assert_eq!(current["batch"]["active"], false);
    assert_eq!(current["batch"]["item_code"], "");
    assert_eq!(monitor["state"]["batch"]["active"], false);
    assert_eq!(monitor["state"]["batch"]["item_code"], "");
}

#[test]
fn driver_print_executes_rs_owned_request_and_returns_done_response() {
    let state = state_with_executor(PrinterKind::Godex, AcceptingDriverPrintExecutor);
    let response = handle_mobile_http_request_with_body(
        &state,
        "POST",
        "/v1/driver/print",
        &format!(
            r#"{{
                "epc":"{EPC}",
                "item_code":"ITEM-1",
                "item_name":"Green Tea",
                "warehouse":"Stores - A",
                "printer":"godex",
                "gross_qty":2.5,
                "tare_enabled":true,
                "tare_kg":0.78
            }}"#
        ),
    );
    let body = json(response.clone());

    assert_eq!(response.status, 200);
    assert_eq!(body["ok"], true);
    assert_eq!(body["status"], "done");
    assert_eq!(body["epc"], EPC);
    assert_eq!(body["printer"], "godex");
    assert_eq!(body["mode"], "label");
    assert_eq!(body["qty"], 1.72);
    assert_eq!(body["gross_qty"], 2.5);
    assert_eq!(body["printer_status"], "OK");
}

#[test]
fn driver_print_executes_duplicate_count_with_same_prepared_label() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let state = state_with_executor(
        PrinterKind::Godex,
        CountingDriverPrintExecutor {
            calls: calls.clone(),
        },
    );
    let response = handle_mobile_http_request_with_body(
        &state,
        "POST",
        "/v1/driver/print",
        &format!(
            r#"{{
                "epc":"{EPC}",
                "item_code":"ITEM-1",
                "item_name":"Green Tea",
                "warehouse":"Stores - A",
                "printer":"godex",
                "gross_qty":2.5,
                "print_count":5
            }}"#
        ),
    );
    let body = json(response.clone());

    assert_eq!(response.status, 200);
    assert_eq!(body["ok"], true);
    assert_eq!(body["print_count"], 5);
    assert_eq!(calls.lock().unwrap().len(), 5);
    assert!(calls.lock().unwrap().iter().all(|epc| epc == EPC));
}

#[test]
fn driver_print_reports_busy_state_and_rejects_parallel_mobile_print() {
    let state = state_with_executor(PrinterKind::Godex, HoldingDriverPrintExecutor);
    let busy_state = state.clone();
    let first = std::thread::spawn(move || {
        handle_mobile_http_request_with_body(
            &busy_state,
            "POST",
            "/v1/driver/print",
            &format!(
                r#"{{
                    "epc":"{EPC}",
                    "item_code":"ITEM-1",
                    "item_name":"Green Tea",
                    "warehouse":"Stores - A",
                    "printer":"godex",
                    "gross_qty":2.5
                }}"#
            ),
        )
    });
    std::thread::sleep(std::time::Duration::from_millis(25));

    let health = json(handle_mobile_http_request(&state, "GET", "/healthz"));
    let handshake = json(handle_mobile_http_request(
        &state,
        "GET",
        "/v1/mobile/handshake",
    ));
    let second = handle_mobile_http_request_with_body(
        &state,
        "POST",
        "/v1/mobile/driver/print",
        &format!(
            r#"{{
                "epc":"{EPC}",
                "item_code":"ITEM-2",
                "item_name":"Black Tea",
                "warehouse":"Stores - A",
                "printer":"godex",
                "gross_qty":1.0
            }}"#
        ),
    );
    let second_body = json(second.clone());
    let first_response = first.join().unwrap();
    let idle_health = json(handle_mobile_http_request(&state, "GET", "/healthz"));

    assert_eq!(health["busy"], true);
    assert_eq!(health["print_activity"]["status"], "printing");
    assert_eq!(health["print_activity"]["item_code"], "ITEM-1");
    assert_eq!(handshake["busy"], true);
    assert_eq!(second.status, 409);
    assert_eq!(second_body["error"], "driver_busy");
    assert_eq!(second_body["print_activity"]["busy"], true);
    assert_eq!(first_response.status, 200);
    assert_eq!(idle_health["busy"], false);
}

#[test]
fn driver_print_fails_closed_when_printer_executor_is_missing() {
    let response = handle_mobile_http_request_with_body(
        &state(PrinterKind::Zebra),
        "POST",
        "/v1/driver/print",
        &format!(
            r#"{{
                "epc":"{EPC}",
                "item_code":"ITEM-1",
                "warehouse":"Stores - A",
                "printer":"zebra",
                "print_mode":"rfid",
                "gross_qty":1.25
            }}"#
        ),
    );
    let body = json(response.clone());

    assert_eq!(response.status, 503);
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "printer_executor_not_configured");
}

#[test]
fn driver_print_rejects_unsupported_printer_mode_before_execution() {
    let state = state_with_executor(PrinterKind::Godex, AcceptingDriverPrintExecutor);
    let response = handle_mobile_http_request_with_body(
        &state,
        "POST",
        "/v1/driver/print",
        &format!(
            r#"{{
                "epc":"{EPC}",
                "item_code":"ITEM-1",
                "warehouse":"Stores - A",
                "printer":"godex",
                "print_mode":"rfid",
                "gross_qty":1.25
            }}"#
        ),
    );
    let body = json(response.clone());

    assert_eq!(response.status, 422);
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "print_prepare_failed");
    assert_eq!(body["detail"], "godex does not support rfid");
}

#[test]
fn driver_print_maps_executor_failure_to_error_response() {
    let state = state_with_executor(PrinterKind::Zebra, FailingDriverPrintExecutor);
    let response = handle_mobile_http_request_with_body(
        &state,
        "POST",
        "/v1/mobile/driver/print",
        &format!(
            r#"{{
                "epc":"{EPC}",
                "item_code":"ITEM-1",
                "warehouse":"Stores - A",
                "printer":"zebra",
                "print_mode":"label",
                "gross_qty":1.25
            }}"#
        ),
    );
    let body = json(response.clone());

    assert_eq!(response.status, 500);
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "print_execution_failed");
    assert_eq!(body["detail"], "printer offline");
}

#[test]
fn batch_mutation_endpoints_are_rejected_in_driver_scope() {
    for path in [
        "/v1/mobile/batch/start",
        "/v1/mobile/batch/stop",
        "/v1/mobile/batch/manual-print",
    ] {
        let response = handle_mobile_http_request_with_body(
            &state(PrinterKind::Zebra),
            "POST",
            path,
            r#"{"item_code":"ITEM-1","warehouse":"Stores - A"}"#,
        );
        let body = json(response.clone());

        assert_eq!(response.status, 409, "{path}");
        assert_eq!(body["error"], "driver_batch_not_supported", "{path}");
    }
}

#[test]
fn catalog_endpoints_return_empty_driver_scope_lists() {
    let items = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/items?query=a",
    ));
    let warehouses = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/warehouses?query=w",
    ));
    let archive = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/archive?limit=50",
    ));

    assert_eq!(items["ok"], true);
    assert_eq!(items["items"].as_array().unwrap().len(), 0);
    assert_eq!(warehouses["ok"], true);
    assert_eq!(warehouses["warehouses"].as_array().unwrap().len(), 0);
    assert_eq!(archive["ok"], true);
    assert_eq!(archive["archive"].as_array().unwrap().len(), 0);
}

#[test]
fn item_warehouses_endpoint_returns_empty_driver_scope_list() {
    let body = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "GET",
        "/v1/mobile/items/ITEM%201/warehouses?limit=12",
    ));

    assert_eq!(body["ok"], true);
    assert_eq!(body["item_code"], "ITEM 1");
    assert_eq!(body["warehouses"].as_array().unwrap().len(), 0);
}

#[test]
fn warehouse_setup_post_returns_driver_scope_setup_status() {
    let body = json(handle_mobile_http_request(
        &state(PrinterKind::Godex),
        "POST",
        "/v1/mobile/setup/warehouse",
    ));

    assert_eq!(body["ok"], true);
    assert_eq!(body["warehouse_mode"], "manual");
    assert_eq!(body["batch_actions_ready"], false);
}

#[test]
fn rejects_wrong_methods_like_gscale_mobileapi() {
    let response = handle_mobile_http_request(&state(PrinterKind::Zebra), "POST", "/healthz");
    let body = json(response.clone());

    assert_eq!(response.status, 405);
    assert_eq!(body["error"], "method_not_allowed");
}

#[test]
fn normalizes_query_string_before_routing() {
    let response = handle_mobile_http_request(
        &state(PrinterKind::Zebra),
        "GET",
        "/v1/mobile/handshake?x=1",
    );

    assert_eq!(response.status, 200);
}
