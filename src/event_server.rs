use crate::event_protocol::{DoneEvent, DEFAULT_CANVAS_BIND_ADDR};
use std::sync::mpsc;
use std::thread;
use tiny_http::{Method, Response, Server, StatusCode};

pub fn start_done_event_server() -> Result<mpsc::Receiver<DoneEvent>, String> {
    let server = Server::http(DEFAULT_CANVAS_BIND_ADDR)
        .map_err(|e| format!("事件服务启动失败: {e}"))?;
    let (tx, rx) = mpsc::channel();

    thread::Builder::new()
        .name("canvas_done_event_server".to_owned())
        .spawn(move || {
            for mut request in server.incoming_requests() {
                if request.method() == &Method::Get && request.url() == "/ping" {
                    let _ = request.respond(Response::from_string("ok"));
                    continue;
                }

                if request.method() != &Method::Post || request.url() != "/done" {
                    let _ = request.respond(
                        Response::from_string("not found").with_status_code(StatusCode(404)),
                    );
                    continue;
                }

                let mut body = String::new();
                let status = match request.as_reader().read_to_string(&mut body) {
                    Ok(_) => match serde_json::from_str::<DoneEvent>(&body) {
                        Ok(event) => match tx.send(event) {
                            Ok(_) => StatusCode(204),
                            Err(_) => StatusCode(500),
                        },
                        Err(_) => StatusCode(400),
                    },
                    Err(_) => StatusCode(400),
                };

                let _ = request.respond(Response::empty(status));
            }
        })
        .map_err(|e| format!("事件服务线程启动失败: {e}"))?;

    Ok(rx)
}
