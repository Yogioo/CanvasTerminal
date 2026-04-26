use egui_node_graph_mvp::event_protocol::{
    AutomationRequest, AutomationResponse, DoneEvent, DEFAULT_CANVAS_API,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::env;

const HELP_TEXT: &str = "canvas - agent/debug CLI

USAGE:
  canvas <COMMAND> [ARGS]

COMMANDS:
  help                                  Show this help
  ping                                  Check whether Canvas app event server is reachable
  done [--route <route_key>] <summary>  Emit a done event from the current terminal node
  debug metrics [--pretty] [--jsonpath p]
  debug graph get [--pretty] [--jsonpath p]
  debug node create|update|move|delete ...
  debug edge create|reconnect|delete ...
  debug inject text|terminal ...
  debug terminal restart --node-id <id>

ENVIRONMENT:
  CANVAS_NODE_UID      Current terminal node uid
  CANVAS_API           Canvas app API base URL (default: http://127.0.0.1:4545)

EXAMPLES:
  canvas done --route fix \"build failed, please fix\"
  canvas debug metrics --pretty
  canvas debug graph get --pretty
  canvas debug node create --kind text --x 200 --y 120 --text \"hello\"
  canvas debug node update --id 2 --working-directory \"../wt-feature-abc\"
  canvas debug inject terminal --node-id 2 --command \"echo ok\" --wait";

fn print_help() {
    println!("{HELP_TEXT}");
}

fn api_base() -> String {
    env::var("CANVAS_API").unwrap_or_else(|_| DEFAULT_CANVAS_API.to_owned())
}

fn command_done(args: Vec<String>) {
    let mut args = VecDeque::from(args);
    let route_key = pop_flag_value(&mut args, "--route")
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty());

    let summary = args
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned();
    if summary.is_empty() {
        eprintln!("usage: canvas done [--route <route_key>] \"summary\"");
        std::process::exit(1);
    }

    let node_uid = env::var("CANVAS_NODE_UID").unwrap_or_else(|_| {
        eprintln!("error: CANVAS_NODE_UID is missing");
        std::process::exit(1);
    });

    let url = format!("{}/done", api_base().trim_end_matches('/'));
    let response = ureq::post(&url).send_json(serde_json::json!(DoneEvent {
        node_uid,
        summary,
        route_key
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
    let url = format!("{}/ping", api_base().trim_end_matches('/'));
    match ureq::get(&url).call() {
        Ok(response) if response.status() == 200 => println!("ok"),
        Ok(response) => {
            eprintln!("error: unexpected status {}", response.status());
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("error: failed to reach canvas app: {err}");
            std::process::exit(1);
        }
    }
}

fn pop_flag_value(args: &mut VecDeque<String>, flag: &str) -> Option<String> {
    let mut idx = 0usize;
    while idx < args.len() {
        if args[idx] == flag {
            args.remove(idx);
            return args.remove(idx);
        }
        idx += 1;
    }
    None
}

fn pop_flag(args: &mut VecDeque<String>, flag: &str) -> bool {
    let mut idx = 0usize;
    while idx < args.len() {
        if args[idx] == flag {
            args.remove(idx);
            return true;
        }
        idx += 1;
    }
    false
}

fn parse_usize(value: Option<String>, name: &str) -> usize {
    value
        .unwrap_or_else(|| {
            eprintln!("error: missing {name}");
            std::process::exit(1);
        })
        .parse::<usize>()
        .unwrap_or_else(|_| {
            eprintln!("error: invalid {name}");
            std::process::exit(1);
        })
}

fn parse_f32(value: Option<String>, name: &str) -> f32 {
    value
        .unwrap_or_else(|| {
            eprintln!("error: missing {name}");
            std::process::exit(1);
        })
        .parse::<f32>()
        .unwrap_or_else(|_| {
            eprintln!("error: invalid {name}");
            std::process::exit(1);
        })
}

fn try_build_debug_action(
    args: Vec<String>,
) -> Result<(AutomationRequest, bool, Option<String>, Vec<String>), String> {
    let mut args = VecDeque::from(args);
    let Some(group) = args.pop_front() else {
        return Err("usage: canvas debug <metrics|graph|node|edge|inject|terminal> ...".to_owned());
    };

    let op = if group == "metrics" {
        if args.front().is_some_and(|value| value == "get") {
            let _ = args.pop_front();
        }
        None
    } else {
        Some(
            args.pop_front()
                .ok_or_else(|| format!("usage: canvas debug {group} <operation> ..."))?,
        )
    };

    let pretty = pop_flag(&mut args, "--pretty");
    let jsonpath = pop_flag_value(&mut args, "--jsonpath");
    let request_id = pop_flag_value(&mut args, "--request-id");

    let (action, payload) = match (group.as_str(), op.as_deref()) {
        ("metrics", None) => ("metrics", json!({})),
        ("graph", Some("get")) => {
            let since_version =
                pop_flag_value(&mut args, "--since-version").and_then(|v| v.parse::<u64>().ok());
            ("graph.get", json!({ "since_version": since_version }))
        }
        ("node", Some("create")) => {
            let kind = pop_flag_value(&mut args, "--kind").unwrap_or_else(|| "text".to_owned());
            let x = parse_f32(pop_flag_value(&mut args, "--x"), "--x");
            let y = parse_f32(pop_flag_value(&mut args, "--y"), "--y");
            let text = pop_flag_value(&mut args, "--text");
            let title = pop_flag_value(&mut args, "--title");
            let startup_script = pop_flag_value(&mut args, "--startup-script");
            let working_directory = pop_flag_value(&mut args, "--working-directory");
            let image_path = pop_flag_value(&mut args, "--image-path");
            (
                "node.create",
                json!({
                    "kind": kind,
                    "x": x,
                    "y": y,
                    "text_body": text,
                    "title": title,
                    "startup_script": startup_script,
                    "working_directory": working_directory,
                    "image_path": image_path,
                }),
            )
        }
        ("node", Some("update")) => {
            let id = parse_usize(pop_flag_value(&mut args, "--id"), "--id");
            let text = pop_flag_value(&mut args, "--text");
            let auto_size = pop_flag_value(&mut args, "--auto-size").map(|v| v == "true");
            let title = pop_flag_value(&mut args, "--title");
            let startup_script = pop_flag_value(&mut args, "--startup-script");
            let working_directory = pop_flag_value(&mut args, "--working-directory");
            (
                "node.update",
                json!({
                    "id": id,
                    "text_body": text,
                    "auto_size": auto_size,
                    "title": title,
                    "startup_script": startup_script,
                    "working_directory": working_directory,
                }),
            )
        }
        ("node", Some("move")) => {
            let id = parse_usize(pop_flag_value(&mut args, "--id"), "--id");
            let x = parse_f32(pop_flag_value(&mut args, "--x"), "--x");
            let y = parse_f32(pop_flag_value(&mut args, "--y"), "--y");
            ("node.move", json!({"id": id, "x": x, "y": y}))
        }
        ("node", Some("delete")) => {
            let id = parse_usize(pop_flag_value(&mut args, "--id"), "--id");
            ("node.delete", json!({"id": id}))
        }
        ("edge", Some("create")) => {
            let from = parse_usize(pop_flag_value(&mut args, "--from"), "--from");
            let to = parse_usize(pop_flag_value(&mut args, "--to"), "--to");
            let route_key = pop_flag_value(&mut args, "--route");
            (
                "edge.create",
                json!({"from": from, "to": to, "route_key": route_key}),
            )
        }
        ("edge", Some("reconnect")) => {
            let from = parse_usize(pop_flag_value(&mut args, "--from"), "--from");
            let to = parse_usize(pop_flag_value(&mut args, "--to"), "--to");
            let new_from = parse_usize(pop_flag_value(&mut args, "--new-from"), "--new-from");
            let new_to = parse_usize(pop_flag_value(&mut args, "--new-to"), "--new-to");
            let new_route_key = pop_flag_value(&mut args, "--new-route");
            (
                "edge.reconnect",
                json!({
                    "from": from,
                    "to": to,
                    "new_from": new_from,
                    "new_to": new_to,
                    "new_route_key": new_route_key,
                }),
            )
        }
        ("edge", Some("delete")) => {
            let from = parse_usize(pop_flag_value(&mut args, "--from"), "--from");
            let to = parse_usize(pop_flag_value(&mut args, "--to"), "--to");
            ("edge.delete", json!({"from": from, "to": to}))
        }
        ("inject", Some("text")) => {
            let node_id = parse_usize(pop_flag_value(&mut args, "--node-id"), "--node-id");
            let mode = pop_flag_value(&mut args, "--mode").unwrap_or_else(|| "replace".to_owned());
            let text = pop_flag_value(&mut args, "--text").unwrap_or_default();
            (
                "inject.text",
                json!({"node_id": node_id, "mode": mode, "text": text}),
            )
        }
        ("inject", Some("terminal")) => {
            let node_id = parse_usize(pop_flag_value(&mut args, "--node-id"), "--node-id");
            let command = pop_flag_value(&mut args, "--command").unwrap_or_default();
            let wait = pop_flag(&mut args, "--wait");
            let timeout =
                pop_flag_value(&mut args, "--timeout").and_then(|v| v.parse::<u64>().ok());
            (
                "inject.terminal",
                json!({
                    "node_id": node_id,
                    "command": command,
                    "wait": wait,
                    "timeout_ms": timeout,
                }),
            )
        }
        ("terminal", Some("restart")) => {
            let node_id = parse_usize(pop_flag_value(&mut args, "--node-id"), "--node-id");
            ("terminal.restart", json!({"node_id": node_id}))
        }
        _ => {
            let op_label = op.unwrap_or_else(|| "<none>".to_owned());
            return Err(format!("error: unknown debug command '{} {}'", group, op_label));
        }
    };

    let warnings = args.into_iter().collect::<Vec<_>>();

    Ok((
        AutomationRequest {
            action: action.to_owned(),
            payload,
            request_id,
            timestamp_ms: None,
        },
        pretty,
        jsonpath,
        warnings,
    ))
}

fn build_debug_action(args: Vec<String>) -> (AutomationRequest, bool, Option<String>) {
    match try_build_debug_action(args) {
        Ok((request, pretty, jsonpath, warnings)) => {
            if !warnings.is_empty() {
                eprintln!("warning: ignored args: {:?}", warnings);
            }
            (request, pretty, jsonpath)
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

fn value_at_jsonpath<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut cursor = value;
    for raw in path.split('.') {
        if raw.is_empty() {
            continue;
        }

        if let Ok(index) = raw.parse::<usize>() {
            cursor = cursor.get(index)?;
        } else {
            cursor = cursor.get(raw)?;
        }
    }

    Some(cursor)
}

fn print_json(value: &Value, pretty: bool) {
    let text = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .unwrap_or_else(|_| "{}".to_owned());
    println!("{text}");
}

fn decode_automation_response(raw: &str) -> Result<AutomationResponse, serde_json::Error> {
    serde_json::from_str(raw)
}

fn command_debug(args: Vec<String>) {
    let (request, pretty, jsonpath) = build_debug_action(args);
    let url = format!("{}/automation", api_base().trim_end_matches('/'));

    let response =
        match ureq::post(&url).send_json(serde_json::to_value(request).unwrap_or(Value::Null)) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("error: failed to call automation api: {err}");
                std::process::exit(1);
            }
        };

    let parsed: AutomationResponse = match response.into_string() {
        Ok(raw) => match decode_automation_response(&raw) {
            Ok(parsed) => parsed,
            Err(err) => {
                eprintln!("error: invalid automation response: {err}");
                std::process::exit(1);
            }
        },
        Err(err) => {
            eprintln!("error: invalid automation response: {err}");
            std::process::exit(1);
        }
    };

    let mut output = serde_json::to_value(&parsed).unwrap_or(Value::Null);
    if let Some(path) = jsonpath {
        output = value_at_jsonpath(&output, &path)
            .cloned()
            .unwrap_or(Value::Null);
    }

    print_json(&output, pretty);

    if !parsed.ok {
        std::process::exit(2);
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
        "debug" => command_debug(args.collect()),
        other => {
            eprintln!("error: unknown command '{other}'\n");
            print_help();
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_metrics_supports_direct_invocation() {
        let (request, pretty, jsonpath, warnings) =
            try_build_debug_action(vec!["metrics".to_owned()]).unwrap();

        assert_eq!(request.action, "metrics");
        assert_eq!(request.payload, json!({}));
        assert!(!pretty);
        assert_eq!(jsonpath, None);
        assert!(warnings.is_empty());
    }

    #[test]
    fn debug_metrics_supports_pretty_and_jsonpath_together() {
        let (request, pretty, jsonpath, warnings) = try_build_debug_action(vec![
            "metrics".to_owned(),
            "--pretty".to_owned(),
            "--jsonpath".to_owned(),
            "data.fps".to_owned(),
        ])
        .unwrap();

        assert_eq!(request.action, "metrics");
        assert!(pretty);
        assert_eq!(jsonpath.as_deref(), Some("data.fps"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn debug_metrics_ignores_extra_positional_args_with_warning() {
        let (request, _pretty, _jsonpath, warnings) = try_build_debug_action(vec![
            "metrics".to_owned(),
            "extra".to_owned(),
            "more".to_owned(),
        ])
        .unwrap();

        assert_eq!(request.action, "metrics");
        assert_eq!(warnings, vec!["extra".to_owned(), "more".to_owned()]);
    }

    #[test]
    fn debug_metrics_decode_automation_response_rejects_non_json() {
        let err = decode_automation_response("not-json").unwrap_err();
        assert!(err.to_string().contains("line 1"));
    }

    #[test]
    fn debug_metrics_requires_subcommand_group() {
        let err = try_build_debug_action(Vec::new()).unwrap_err();
        assert!(err.contains("usage: canvas debug"));
    }

    #[test]
    fn help_text_mentions_debug_metrics() {
        assert!(HELP_TEXT.contains("debug metrics [--pretty] [--jsonpath p]"));
    }
}
