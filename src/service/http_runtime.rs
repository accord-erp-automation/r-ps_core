use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use super::http::{MobileHttpResponse, MobileHttpState, handle_mobile_http_request_with_body};

const MONITOR_STREAM_PATH: &str = "/v1/mobile/monitor/stream";
const MONITOR_STREAM_TICK: Duration = Duration::from_millis(350);
const MONITOR_STREAM_HEARTBEAT: Duration = Duration::from_secs(15);

pub fn bind_mobile_http_listener(addr: &str) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(false)?;
    Ok(listener)
}

pub fn serve_mobile_http(listener: TcpListener, state: MobileHttpState) -> io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = state.clone();
                thread::spawn(move || {
                    let mut stream = stream;
                    let _ = handle_mobile_http_stream(&mut stream, &state);
                });
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

pub fn handle_mobile_http_stream(
    stream: &mut TcpStream,
    state: &MobileHttpState,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    let request_bytes = read_http_request(stream)?;
    let request = String::from_utf8_lossy(&request_bytes);

    if let Some((method, path)) = parse_request_line(&request)
        && normalize_request_path(path) == MONITOR_STREAM_PATH
    {
        if method.trim().eq_ignore_ascii_case("GET") {
            return write_monitor_stream_response(stream, state);
        }
        return write_http_response(
            stream,
            &MobileHttpResponse::json(
                405,
                &super::http::MobileHttpErrorResponse {
                    error: "method_not_allowed",
                },
            ),
        );
    }

    let response = route_raw_http_request(&request, state);
    write_http_response(stream, &response)
}

pub fn route_raw_http_request(raw: &str, state: &MobileHttpState) -> MobileHttpResponse {
    let Some((method, path)) = parse_request_line(raw) else {
        return MobileHttpResponse::json(
            400,
            &super::http::MobileHttpErrorResponse {
                error: "bad_request",
            },
        );
    };
    handle_mobile_http_request_with_body(state, method, path, parse_body(raw))
}

fn parse_body(raw: &str) -> &str {
    raw.split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .map(|(_, body)| body)
        .unwrap_or("")
}

fn read_http_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut request = Vec::with_capacity(8192);
    let mut buf = [0_u8; 8192];
    let n = stream.read(&mut buf)?;
    request.extend_from_slice(&buf[..n]);

    while header_end_offset(&request).is_none() {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(request);
        }
        request.extend_from_slice(&buf[..n]);
    }

    let header_end = header_end_offset(&request).unwrap_or(request.len());
    let content_length = content_length_from_headers(&request[..header_end]).unwrap_or(0);
    let expected_len = header_end + content_length;
    while request.len() < expected_len {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        request.extend_from_slice(&buf[..n]);
    }

    Ok(request)
}

fn header_end_offset(raw: &[u8]) -> Option<usize> {
    find_bytes(raw, b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| find_bytes(raw, b"\n\n").map(|index| index + 2))
}

fn content_length_from_headers(headers: &[u8]) -> Option<usize> {
    let headers = String::from_utf8_lossy(headers);
    for line in headers.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse::<usize>().ok();
        }
    }
    None
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_request_line(raw: &str) -> Option<(&str, &str)> {
    let line = raw.lines().next()?.trim();
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

fn normalize_request_path(path: &str) -> String {
    let path = path.trim();
    let path = path.split_once('?').map(|(path, _)| path).unwrap_or(path);
    match path {
        "" => "/".to_string(),
        value if value.starts_with('/') => value.to_string(),
        value => format!("/{value}"),
    }
}

fn write_monitor_stream_response(
    stream: &mut TcpStream,
    state: &MobileHttpState,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nX-Accel-Buffering: no\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Accept, Authorization\r\n\r\n"
    )?;
    stream.write_all(b": connected\n\n")?;
    stream.flush()?;

    let mut last_payload = Vec::new();
    let mut last_heartbeat = Instant::now();
    loop {
        let frame = monitor_stream_snapshot_frame(state)?;
        if frame.payload != last_payload {
            last_payload = frame.payload;
            stream.write_all(frame.text.as_bytes())?;
            stream.flush()?;
        }

        if last_heartbeat.elapsed() >= MONITOR_STREAM_HEARTBEAT {
            stream.write_all(b": ping\n\n")?;
            stream.flush()?;
            last_heartbeat = Instant::now();
        }

        thread::sleep(MONITOR_STREAM_TICK);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SseFrame {
    text: String,
    payload: Vec<u8>,
}

fn monitor_stream_snapshot_frame(state: &MobileHttpState) -> io::Result<SseFrame> {
    let payload = serde_json::to_vec(
        &state
            .monitor
            .snapshot(&state.identity, state.active_printer),
    )
    .map_err(io::Error::other)?;
    let text = format!(
        "event: snapshot\ndata: {}\n\n",
        String::from_utf8_lossy(&payload)
    );
    Ok(SseFrame { text, payload })
}

fn write_http_response(stream: &mut TcpStream, response: &MobileHttpResponse) -> io::Result<()> {
    let status_text = match response.status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Accept, Authorization\r\nAccess-Control-Max-Age: 86400\r\n\r\n",
        response.status,
        status_text,
        response.content_type,
        response.body.len()
    )?;
    stream.write_all(&response.body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::printer::PrinterKind;
    use crate::service::mobile_contract::ServiceIdentity;
    use crate::service::monitor_runtime::MonitorRuntimeState;
    use serde_json::Value;

    fn state() -> MobileHttpState {
        MobileHttpState::new(
            ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin"),
            39117,
            vec![39117, 41257],
            PrinterKind::Godex,
            MonitorRuntimeState::default(),
        )
    }

    fn body_json(response: MobileHttpResponse) -> Value {
        serde_json::from_slice(&response.body).unwrap()
    }

    #[test]
    fn routes_raw_handshake_request() {
        let response = route_raw_http_request(
            "GET /v1/mobile/handshake HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(body["service"], "mobileapi");
        assert_eq!(body["server_name"], "rp-scale");
    }

    #[test]
    fn routes_raw_capabilities_request() {
        let response = route_raw_http_request(
            "GET /v1/mobile/printer/capabilities HTTP/1.1\r\n\r\n",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(body["active_printer"]["id"], "godex");
        assert_eq!(
            body["active_printer"]["capabilities"]["rfid_epc_write"],
            false
        );
    }

    #[test]
    fn routes_raw_monitor_state_request() {
        let response = route_raw_http_request(
            "GET /v1/mobile/monitor/state HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(body["ok"], true);
        assert_eq!(body["state"]["batch"]["active"], false);
        assert_eq!(body["state"]["print_request"]["status"], "idle");
    }

    #[test]
    fn routes_raw_setup_status_request() {
        let response = route_raw_http_request(
            "GET /v1/mobile/setup/status HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(body["ok"], true);
        assert_eq!(body["erp_write_configured"], false);
        assert_eq!(body["batch_actions_ready"], false);
    }

    #[test]
    fn routes_raw_batch_state_request() {
        let response = route_raw_http_request(
            "GET /v1/mobile/batch/state HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(body["ok"], true);
        assert_eq!(body["batch"]["active"], false);
        assert_eq!(body["batch"]["printer"], "godex");
    }

    #[test]
    fn routes_raw_items_request() {
        let response = route_raw_http_request(
            "GET /v1/mobile/items?query=a HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(body["ok"], true);
        assert!(body["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn routes_raw_item_warehouses_request() {
        let response = route_raw_http_request(
            "GET /v1/mobile/items/ITEM%201/warehouses HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 200);
        assert_eq!(body["item_code"], "ITEM 1");
        assert!(body["warehouses"].as_array().unwrap().is_empty());
    }

    #[test]
    fn routes_raw_batch_start_with_json_body() {
        let response = route_raw_http_request(
            "POST /v1/mobile/batch/start HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"item_code\":\"ITEM-1\",\"item_name\":\"Sugar\",\"warehouse\":\"Stores - A\",\"printer\":\"godex\",\"print_mode\":\"rfid\"}",
            &state(),
        );
        let body = body_json(response.clone());

        assert_eq!(response.status, 409);
        assert_eq!(body["error"], "driver_batch_not_supported");
    }

    #[test]
    fn parses_empty_raw_http_body() {
        assert_eq!(parse_body("GET /healthz HTTP/1.1\r\n\r\n"), "");
    }

    #[test]
    fn builds_monitor_stream_snapshot_frame_like_gscale_sse() {
        let frame = monitor_stream_snapshot_frame(&state()).unwrap();

        assert!(frame.text.starts_with("event: snapshot\ndata: "));
        assert!(frame.text.ends_with("\n\n"));
        assert!(frame.text.contains(r#""ok":true"#));
        assert!(frame.text.contains(r#""state":"#));
        assert!(!frame.payload.is_empty());
    }

    #[test]
    fn normalizes_monitor_stream_path_with_query() {
        assert_eq!(
            normalize_request_path("/v1/mobile/monitor/stream?x=1"),
            MONITOR_STREAM_PATH
        );
    }

    #[test]
    fn rejects_malformed_http_request() {
        let response = route_raw_http_request("broken", &state());
        let body = body_json(response.clone());

        assert_eq!(response.status, 400);
        assert_eq!(body["error"], "bad_request");
    }

    #[test]
    fn reads_delayed_body_by_content_length_before_routing() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_mobile_http_stream(&mut stream, &state()).unwrap();
        });

        let body =
            r#"{"epc":"EPC-1","item_code":"ITEM-1","warehouse":"Stores - A","gross_qty":1.25}"#;
        let mut client = TcpStream::connect(addr).unwrap();
        write!(
            client,
            "POST /v1/driver/print HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        thread::sleep(Duration::from_millis(50));
        client.write_all(body.as_bytes()).unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        server.join().unwrap();

        assert!(
            response.starts_with("HTTP/1.1 503 Service Unavailable"),
            "{response}"
        );
        assert!(
            response.contains("printer_executor_not_configured"),
            "{response}"
        );
    }
}
