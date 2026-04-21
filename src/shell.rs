use crate::event_protocol::DEFAULT_CANVAS_API;

pub fn system_shell() -> String {
    #[cfg(windows)]
    {
        "cmd.exe".to_owned()
    }

    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_owned())
    }
}

pub fn terminal_shell_args(node_id: usize, identity: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let identity = identity.replace('"', "");
        vec![
            "/D".to_owned(),
            "/K".to_owned(),
            format!(
                "set \"CANVAS_NODE_ID={node_id}\" && set \"CANVAS_IDENTITY={identity}\" && set \"CANVAS_API={DEFAULT_CANVAS_API}\""
            ),
        ]
    }

    #[cfg(not(windows))]
    {
        let escaped_identity = identity.replace('\\', "\\\\").replace('"', "\\\"");
        vec![
            "-lc".to_owned(),
            format!(
                "export CANVAS_NODE_ID={node_id} CANVAS_IDENTITY=\"{escaped_identity}\" CANVAS_API=\"{DEFAULT_CANVAS_API}\"; exec {}",
                system_shell()
            ),
        ]
    }
}
