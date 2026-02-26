use std::process::Command;

use anyhow::Result;

pub fn send_desktop_notification(title: &str, body: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escape_applescript_string(body),
            escape_applescript_string(title)
        );

        let _ = Command::new("osascript").arg("-e").arg(script).status();
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn escape_applescript_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}
