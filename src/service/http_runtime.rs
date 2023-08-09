use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use super::http::{MobileHttpResponse, MobileHttpState, handle_mobile_http_request};

pub fn bind_mobile_http_listener(addr: &str) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(false)?;
    Ok(listener)
}

pub fn serve_mobile_http(listener: TcpListener, state: MobileHttpState) -> io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let _ = handle_mobile_http_stream(&mut stream, &state);
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

    let mut buf = [0_u8; 8192];
    let n = stream.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..n]);
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
    handle_mobile_http_request(state, method, path)
}

fn parse_request_line(raw: &str) -> Option<(&str, &str)> {
    let line = raw.lines().next()?.trim();
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

fn write_http_response(stream: &mut TcpStream, response: &MobileHttpResponse) -> io::Result<()> {
    let status_text = match response.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    fn rejects_malformed_http_request() {
        let response = route_raw_http_request("broken", &state());
        let body = body_json(response.clone());

        assert_eq!(response.status, 400);
        assert_eq!(body["error"], "bad_request");
    }
}
