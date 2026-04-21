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
