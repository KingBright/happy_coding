//! Core utilities for Happy Coding

/// Get machine name - prefer macOS ComputerName for user-friendly name
pub fn get_machine_name() -> String {
    #[cfg(target_os = "macos")]
    {
        // On macOS, try to get the user-friendly ComputerName first
        if let Ok(output) = std::process::Command::new("scutil")
            .args(["--get", "ComputerName"])
            .output()
        {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !name.is_empty() {
                    return name;
                }
            }
        }
    }

    // Fallback to hostname using whoami (cross-platform)
    whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string())
}
