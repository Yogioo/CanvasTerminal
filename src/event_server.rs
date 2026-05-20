use crate::event_protocol::{
    now_timestamp_ms, response_error, AppEvent, AutomationCall, AutomationRequest, DoneEvent,
    DEFAULT_CANVAS_BIND_ADDR,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Method, Response, Server, StatusCode};

fn json_header() -> Option<Header> {
    Header::from_bytes(
        b"Content-Type".as_ref(),
        b"application/json; charset=utf-8".as_ref(),
    )
    .ok()
}

fn response_json_or_fallback(response: crate::event_protocol::AutomationResponse) -> String {
    serde_json::to_string(&response).unwrap_or_else(|_| "{\"ok\":false}".to_owned())
}

fn dispatch_automation_request_with_timeout(
    tx: &mpsc::Sender<AppEvent>,
    request_body: AutomationRequest,
    timeout: Duration,
) -> String {
    let (resp_tx, resp_rx) = mpsc::channel();
    let call = AutomationCall {
        request: request_body.clone(),
        received_at_ms: now_timestamp_ms(),
        response_tx: resp_tx,
    };

    if tx.send(AppEvent::Automation(call)).is_err() {
        return response_json_or_fallback(response_error(
            request_body.request_id,
            &request_body.action,
            "INTERNAL_CHANNEL_CLOSED",
            "automation request channel closed",
        ));
    }

    match resp_rx.recv_timeout(timeout) {
        Ok(resp) => response_json_or_fallback(resp),
        Err(_) => response_json_or_fallback(response_error(
            request_body.request_id,
            &request_body.action,
            "TIMEOUT",
            "automation request timed out",
        )),
    }
}

fn dispatch_automation_request(
    tx: &mpsc::Sender<AppEvent>,
    request_body: AutomationRequest,
) -> String {
    dispatch_automation_request_with_timeout(tx, request_body, Duration::from_secs(60))
}

fn respond_automation_json(request: tiny_http::Request, response_json: String) {
    let mut response = Response::from_string(response_json).with_status_code(StatusCode(200));
    if let Some(h) = json_header() {
        response = response.with_header(h);
    }
    let _ = request.respond(response);
}

fn start_event_server_with_addr(bind_addr: &str) -> Result<mpsc::Receiver<AppEvent>, String> {
    let server = Server::http(bind_addr).map_err(|e| format!("事件服务启动失败: {e}"))?;
    let (tx, rx) = mpsc::channel();

    thread::Builder::new()
        .name("canvas_event_server".to_owned())
        .spawn(move || {
            for mut request in server.incoming_requests() {
                if request.method() == &Method::Get && request.url() == "/ping" {
                    let _ = request.respond(Response::from_string("ok"));
                    continue;
                }

                if request.method() == &Method::Post && request.url() == "/done" {
                    let mut body = String::new();
                    let status = match request.as_reader().read_to_string(&mut body) {
                        Ok(_) => match serde_json::from_str::<DoneEvent>(&body) {
                            Ok(event) => match tx.send(AppEvent::Done(event)) {
                                Ok(_) => StatusCode(204),
                                Err(_) => StatusCode(500),
                            },
                            Err(_) => StatusCode(400),
                        },
                        Err(_) => StatusCode(400),
                    };

                    let _ = request.respond(Response::empty(status));
                    continue;
                }

                if request.method() == &Method::Get && request.url() == "/automation/metrics" {
                    let automation_request = AutomationRequest {
                        action: "metrics".to_owned(),
                        payload: serde_json::Value::Object(serde_json::Map::new()),
                        request_id: None,
                        timestamp_ms: Some(now_timestamp_ms()),
                    };

                    let response_json = dispatch_automation_request(&tx, automation_request);
                    respond_automation_json(request, response_json);
                    continue;
                }

                if request.method() == &Method::Post && request.url() == "/automation" {
                    let mut body = String::new();
                    let parse_result = request
                        .as_reader()
                        .read_to_string(&mut body)
                        .ok()
                        .and_then(|_| serde_json::from_str::<AutomationRequest>(&body).ok());

                    let response_json = if let Some(request_body) = parse_result {
                        dispatch_automation_request(&tx, request_body)
                    } else {
                        response_json_or_fallback(response_error(
                            None,
                            "invalid",
                            "BAD_REQUEST",
                            "invalid automation request payload",
                        ))
                    };

                    respond_automation_json(request, response_json);
                    continue;
                }

                let _ = request
                    .respond(Response::from_string("not found").with_status_code(StatusCode(404)));
            }
        })
        .map_err(|e| format!("事件服务线程启动失败: {e}"))?;

    Ok(rx)
}

pub fn start_event_server() -> Result<mpsc::Receiver<AppEvent>, String> {
    start_event_server_with_addr(DEFAULT_CANVAS_BIND_ADDR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_protocol::{empty_diagnostics, AutomationError, AutomationResponse};
    use serde_json::Value;
    use std::net::TcpListener;
    use std::sync::mpsc::{self, Receiver};
    use std::thread;
    use std::time::{Duration, Instant};

    fn wait_for_automation_call(rx: &Receiver<AppEvent>) -> AutomationCall {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| Duration::from_millis(0));
            if remaining.is_zero() {
                panic!("timeout waiting for automation call");
            }

            match rx.recv_timeout(remaining) {
                Ok(AppEvent::Automation(call)) => return call,
                Ok(AppEvent::Done(_)) => continue,
                Err(err) => panic!("failed to receive automation call: {err}"),
            }
        }
    }

    #[test]
    fn event_server_metrics_dispatch_returns_channel_closed_error() {
        let (tx, rx) = mpsc::channel();
        drop(rx);

        let response: Value = serde_json::from_str(&dispatch_automation_request(
            &tx,
            AutomationRequest {
                action: "metrics".to_owned(),
                payload: Value::Object(serde_json::Map::new()),
                request_id: Some("req-closed".to_owned()),
                timestamp_ms: None,
            },
        ))
        .expect("response json");

        assert_eq!(response.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            response
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("INTERNAL_CHANNEL_CLOSED")
        );
    }

    #[test]
    fn event_server_metrics_dispatch_returns_timeout_error() {
        let (tx, _rx) = mpsc::channel();

        let started = Instant::now();
        let response: Value = serde_json::from_str(&dispatch_automation_request_with_timeout(
            &tx,
            AutomationRequest {
                action: "metrics".to_owned(),
                payload: Value::Object(serde_json::Map::new()),
                request_id: Some("req-timeout".to_owned()),
                timestamp_ms: None,
            },
            Duration::from_millis(20),
        ))
        .expect("response json");

        assert!(started.elapsed() >= Duration::from_millis(20));
        assert_eq!(response.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            response
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("TIMEOUT")
        );
    }

    fn allocate_test_bind_addr() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral test port");
        let addr = listener.local_addr().expect("read local addr");
        format!("127.0.0.1:{}", addr.port())
    }

    #[test]
    fn event_server_metrics_get_and_automation_error_paths() {
        let bind_addr = allocate_test_bind_addr();
        let base_url = format!("http://{bind_addr}");
        let rx = start_event_server_with_addr(&bind_addr)
            .unwrap_or_else(|err| panic!("start_event_server failed at {bind_addr}: {err}"));

        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            let get_metrics = wait_for_automation_call(&rx);
            assert_eq!(get_metrics.request.action, "metrics");
            assert_eq!(get_metrics.request.payload, Value::Object(serde_json::Map::new()));
            get_metrics
                .response_tx
                .send(AutomationResponse {
                    request_id: get_metrics.request.request_id.clone(),
                    ok: true,
                    data: serde_json::json!({"fps": 61.0, "cpu_usage": Value::Null}),
                    error: None,
                    diagnostics: empty_diagnostics("metrics"),
                })
                .expect("send metrics response");

            let bad_payload = wait_for_automation_call(&rx);
            assert_eq!(bad_payload.request.action, "node.create");
            bad_payload
                .response_tx
                .send(AutomationResponse {
                    request_id: bad_payload.request.request_id.clone(),
                    ok: false,
                    data: Value::Null,
                    error: Some(AutomationError {
                        code: "BAD_PAYLOAD".to_owned(),
                        message: "invalid payload".to_owned(),
                        details: None,
                    }),
                    diagnostics: empty_diagnostics("node.create"),
                })
                .expect("send bad payload response");

            done_tx.send(()).expect("notify done");
        });

        let metrics_url = format!("{base_url}/automation/metrics");
        let metrics_response: Value = ureq::get(&metrics_url)
            .call()
            .expect("GET /automation/metrics should succeed")
            .into_json()
            .expect("metrics response json");
        assert_eq!(metrics_response.get("ok").and_then(Value::as_bool), Some(true));
        assert_eq!(
            metrics_response
                .get("data")
                .and_then(|data| data.get("fps"))
                .and_then(Value::as_f64),
            Some(61.0)
        );
        assert_eq!(
            metrics_response
                .get("data")
                .and_then(|data| data.get("cpu_usage")),
            Some(&Value::Null)
        );

        let automation_url = format!("{base_url}/automation");
        let bad_payload_response: Value = ureq::post(&automation_url)
            .send_string("{\"action\":\"node.create\",\"payload\":\"oops\"}")
            .expect("POST /automation should return JSON envelope")
            .into_json()
            .expect("bad payload response json");
        assert_eq!(bad_payload_response.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            bad_payload_response
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("BAD_PAYLOAD")
        );

        let malformed_response: Value = ureq::post(&automation_url)
            .send_string("{not-json}")
            .expect("malformed POST /automation should return BAD_REQUEST JSON")
            .into_json()
            .expect("malformed response json");
        assert_eq!(malformed_response.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            malformed_response
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("BAD_REQUEST")
        );

        done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("handler thread should finish");
    }
}
