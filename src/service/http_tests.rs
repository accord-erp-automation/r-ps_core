use super::*;
use serde_json::Value;

fn state(printer: PrinterKind) -> MobileHttpState {
    MobileHttpState::new(
        ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin"),
        39117,
        vec![39117, 41257],
        printer,
        MonitorRuntimeState::default(),
    )
}

fn json(response: MobileHttpResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap()
}

#[test]
fn healthz_matches_gscale_mobile_fallback_contract() {
    let response = handle_mobile_http_request(&state(PrinterKind::Zebra), "GET", "/healthz");
    let body = json(response.clone());

    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "application/json");
    assert_eq!(body["ok"], true);
    assert_eq!(body["service"], "mobileapi");
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
fn batch_start_stores_mobile_provided_values_in_local_runtime() {
    let state = state(PrinterKind::Godex);
    let started = json(handle_mobile_http_request_with_body(
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
    ));
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

    assert_eq!(started["ok"], true);
    assert_eq!(started["batch"]["active"], true);
    assert_eq!(started["batch"]["item_code"], "ITEM-1");
    assert_eq!(started["batch"]["item_name"], "Sugar");
    assert_eq!(started["batch"]["warehouse"], "Stores - A");
    assert_eq!(started["batch"]["printer"], "godex");
    assert_eq!(started["batch"]["print_mode"], "label");
    assert_eq!(started["batch"]["tare"], true);
    assert_eq!(current["batch"]["active"], true);
    assert_eq!(monitor["state"]["batch"]["item_code"], "ITEM-1");
}

#[test]
fn batch_start_rejects_missing_item_or_warehouse() {
    let response = handle_mobile_http_request_with_body(
        &state(PrinterKind::Zebra),
        "POST",
        "/v1/mobile/batch/start",
        r#"{"item_code":"ITEM-1"}"#,
    );
    let body = json(response.clone());

    assert_eq!(response.status, 400);
    assert_eq!(body["error"], "item_code_and_warehouse_required");
}

#[test]
fn batch_stop_clears_local_runtime_batch() {
    let state = state(PrinterKind::Zebra);
    let _ = handle_mobile_http_request_with_body(
        &state,
        "POST",
        "/v1/mobile/batch/start",
        r#"{"item_code":"ITEM-1","warehouse":"Stores - A","printer":"zebra"}"#,
    );
    let stopped = json(handle_mobile_http_request(
        &state,
        "POST",
        "/v1/mobile/batch/stop",
    ));
    let current = json(handle_mobile_http_request(
        &state,
        "GET",
        "/v1/mobile/batch/state",
    ));

    assert_eq!(stopped["ok"], true);
    assert_eq!(stopped["batch"]["active"], false);
    assert_eq!(current["batch"]["active"], false);
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
