use egui_node_graph_mvp::event_protocol::{DoneEvent, DEFAULT_CANVAS_API};
use std::env;

fn print_help() {
    println!(
        "canvas - agent event CLI\n\nUSAGE:\n  canvas <COMMAND> [ARGS]\n\nCOMMANDS:\n  help                 Show this help message\n  ping                 Check whether the Canvas app event server is reachable\n  done <summary>       Emit a done event from the current terminal node\n\nENVIRONMENT:\n  CANVAS_NODE_ID       Current terminal node id\n  CANVAS_IDENTITY      Current terminal identity\n  CANVAS_API           Canvas app API base URL (default: http://127.0.0.1:4545)\n\nEXAMPLES:\n  canvas done \"已完成测试\"\n  canvas ping"
    );
}

fn command_done(args: Vec<String>) {
    let summary = args.join(" ").trim().to_owned();
    if summary.is_empty() {
        eprintln!("usage: canvas done \"summary\"");
        std::process::exit(1);
    }

    let node_id = env::var("CANVAS_NODE_ID")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| {
            eprintln!("error: CANVAS_NODE_ID is missing");
            std::process::exit(1);
        });

    let identity = env::var("CANVAS_IDENTITY").unwrap_or_else(|_| "agent".to_owned());
    let api = env::var("CANVAS_API").unwrap_or_else(|_| DEFAULT_CANVAS_API.to_owned());
    let url = format!("{}/done", api.trim_end_matches('/'));

    let response = ureq::post(&url).send_json(serde_json::json!(DoneEvent {
        node_id,
        identity,
        summary,
    }));

    match response {
        Ok(_) => println!("ok"),
        Err(err) => {
            eprintln!("error: failed to emit done event: {err}");
            std::process::exit(1);
        }
    }
}

fn command_ping() {
    let api = env::var("CANVAS_API").unwrap_or_else(|_| DEFAULT_CANVAS_API.to_owned());
    let url = format!("{}/ping", api.trim_end_matches('/'));

    match ureq::get(&url).call() {
        Ok(response) => {
            if response.status() == 200 {
                println!("ok");
            } else {
                eprintln!("error: unexpected status {}", response.status());
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("error: failed to reach canvas app: {err}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return;
    };

    match command.as_str() {
        "-h" | "--help" | "help" => print_help(),
        "ping" => command_ping(),
        "done" => command_done(args.collect()),
        other => {
            eprintln!("error: unknown command '{other}'\n");
            print_help();
            std::process::exit(1);
        }
    }
}
