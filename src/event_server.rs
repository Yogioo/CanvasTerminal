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

pub fn start_event_server() -> Result<mpsc::Receiver<AppEvent>, String> {
    let server =
        Server::http(DEFAULT_CANVAS_BIND_ADDR).map_err(|e| format!("事件服务启动失败: {e}"))?;
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

                if request.method() == &Method::Post && request.url() == "/automation" {
                    let mut body = String::new();
                    let parse_result = request
                        .as_reader()
                        .read_to_string(&mut body)
                        .ok()
                        .and_then(|_| serde_json::from_str::<AutomationRequest>(&body).ok());

                    let response_json = if let Some(request_body) = parse_result {
                        let (resp_tx, resp_rx) = mpsc::channel();
                        let call = AutomationCall {
                            request: request_body.clone(),
                            received_at_ms: now_timestamp_ms(),
                            response_tx: resp_tx,
                        };

                        if tx.send(AppEvent::Automation(call)).is_err() {
                            serde_json::to_string(&response_error(
                                request_body.request_id,
                                &request_body.action,
                                "INTERNAL_CHANNEL_CLOSED",
                                "automation request channel closed",
                            ))
                            .unwrap_or_else(|_| "{\"ok\":false}".to_owned())
                        } else {
                            match resp_rx.recv_timeout(Duration::from_secs(60)) {
                                Ok(resp) => serde_json::to_string(&resp)
                                    .unwrap_or_else(|_| "{\"ok\":false}".to_owned()),
                                Err(_) => serde_json::to_string(&response_error(
                                    request_body.request_id,
                                    &request_body.action,
                                    "TIMEOUT",
                                    "automation request timed out",
                                ))
                                .unwrap_or_else(|_| "{\"ok\":false}".to_owned()),
                            }
                        }
                    } else {
                        serde_json::to_string(&response_error(
                            None,
                            "invalid",
                            "BAD_REQUEST",
                            "invalid automation request payload",
                        ))
                        .unwrap_or_else(|_| "{\"ok\":false}".to_owned())
                    };

                    let mut response =
                        Response::from_string(response_json).with_status_code(StatusCode(200));
                    if let Some(h) = json_header() {
                        response = response.with_header(h);
                    }
                    let _ = request.respond(response);
                    continue;
                }

                let _ = request
                    .respond(Response::from_string("not found").with_status_code(StatusCode(404)));
            }
        })
        .map_err(|e| format!("事件服务线程启动失败: {e}"))?;

    Ok(rx)
}
