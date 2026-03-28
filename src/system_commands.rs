use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCommand {
    pub name: String,
    pub description: String,
    pub command: String,
}

pub fn search_commands(query: &str) -> Vec<SystemCommand> {
    let commands = get_all_commands();
    let query_lower = query.to_lowercase();

    commands
        .into_iter()
        .filter(|cmd| {
            cmd.name.to_lowercase().contains(&query_lower)
                || cmd.description.to_lowercase().contains(&query_lower)
        })
        .collect()
}

pub fn get_all_commands() -> Vec<SystemCommand> {
    #[cfg(target_os = "macos")]
    {
        return vec![
            command("Lock Screen", "Lock the screen immediately", "lock"),
            command("Sleep", "Put the computer to sleep", "sleep"),
            command("Restart", "Restart the computer", "restart"),
            command("Shutdown", "Shut down the computer", "shutdown"),
            command("Volume Up", "Increase system volume", "volume_up"),
            command("Volume Down", "Decrease system volume", "volume_down"),
            command("Mute", "Toggle mute", "mute"),
            command("Empty Trash", "Empty the trash", "empty_trash"),
            command(
                "Show Hidden Files",
                "Toggle hidden files visibility in Finder",
                "toggle_hidden_files",
            ),
            command("Screenshot", "Take a screenshot", "screenshot"),
            command("Color Picker", "Open color picker", "color_picker"),
            command("System Settings", "Open System Settings", "system_settings"),
        ];
    }

    #[cfg(target_os = "windows")]
    {
        return vec![
            command("Lock Screen", "Lock the workstation", "lock"),
            command("Sleep", "Put the computer to sleep", "sleep"),
            command("Restart", "Restart Windows", "restart"),
            command("Shutdown", "Shut down Windows", "shutdown"),
            command("Screenshot", "Open Snipping Tool", "screenshot"),
            command("Settings", "Open Windows Settings", "system_settings"),
            command("Recycle Bin", "Open the Recycle Bin", "recycle_bin"),
        ];
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        vec![
            command("Lock Screen", "Lock the current session", "lock"),
            command("Sleep", "Suspend the computer", "sleep"),
            command("Restart", "Restart the computer", "restart"),
            command("Shutdown", "Shut down the computer", "shutdown"),
            command(
                "Settings",
                "Open the default system settings application",
                "system_settings",
            ),
        ]
    }
}

pub async fn execute(command: &str) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        return execute_macos(command);
    }

    #[cfg(target_os = "windows")]
    {
        return execute_windows(command);
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        execute_linux(command)
    }
}

fn command(name: &str, description: &str, command: &str) -> SystemCommand {
    SystemCommand {
        name: name.to_string(),
        description: description.to_string(),
        command: command.to_string(),
    }
}

#[cfg(target_os = "macos")]
fn execute_macos(command: &str) -> Result<String> {
    match command {
        "lock" => {
            Command::new("pmset").arg("displaysleepnow").spawn()?;
            Ok("Screen locked".to_string())
        }
        "sleep" => {
            Command::new("pmset").arg("sleepnow").spawn()?;
            Ok("Going to sleep".to_string())
        }
        "restart" => {
            Command::new("osascript")
                .arg("-e")
                .arg("tell app \"System Events\" to restart")
                .spawn()?;
            Ok("Restarting".to_string())
        }
        "shutdown" => {
            Command::new("osascript")
                .arg("-e")
                .arg("tell app \"System Events\" to shut down")
                .spawn()?;
            Ok("Shutting down".to_string())
        }
        "volume_up" => {
            Command::new("osascript")
                .arg("-e")
                .arg("set volume output volume (output volume of (get volume settings) + 10)")
                .spawn()?;
            Ok("Volume increased".to_string())
        }
        "volume_down" => {
            Command::new("osascript")
                .arg("-e")
                .arg("set volume output volume (output volume of (get volume settings) - 10)")
                .spawn()?;
            Ok("Volume decreased".to_string())
        }
        "mute" => {
            Command::new("osascript")
                .arg("-e")
                .arg("set volume output muted not (output muted of (get volume settings))")
                .spawn()?;
            Ok("Mute toggled".to_string())
        }
        "empty_trash" => {
            Command::new("osascript")
                .arg("-e")
                .arg("tell app \"Finder\" to empty trash")
                .spawn()?;
            Ok("Trash emptied".to_string())
        }
        "toggle_hidden_files" => {
            Command::new("defaults")
                .args([
                    "write",
                    "com.apple.finder",
                    "AppleShowAllFiles",
                    "-bool",
                    "YES",
                ])
                .spawn()?;
            Command::new("killall").arg("Finder").spawn()?;
            Ok("Hidden files toggled".to_string())
        }
        "screenshot" => {
            Command::new("screencapture").arg("-i").spawn()?;
            Ok("Screenshot tool opened".to_string())
        }
        "color_picker" => {
            Command::new("osascript")
                .arg("-e")
                .arg("choose color")
                .spawn()?;
            Ok("Color picker opened".to_string())
        }
        "system_settings" => {
            Command::new("open")
                .arg("-a")
                .arg("System Settings")
                .spawn()?;
            Ok("System Settings opened".to_string())
        }
        _ => Ok("Unknown command".to_string()),
    }
}

#[cfg(target_os = "windows")]
fn execute_windows(command: &str) -> Result<String> {
    match command {
        "lock" => {
            Command::new("rundll32.exe")
                .args(["user32.dll,LockWorkStation"])
                .spawn()?;
            Ok("Workstation locked".to_string())
        }
        "sleep" => {
            Command::new("rundll32.exe")
                .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                .spawn()?;
            Ok("Sleep requested".to_string())
        }
        "restart" => {
            Command::new("shutdown").args(["/r", "/t", "0"]).spawn()?;
            Ok("Restarting".to_string())
        }
        "shutdown" => {
            Command::new("shutdown").args(["/s", "/t", "0"]).spawn()?;
            Ok("Shutting down".to_string())
        }
        "screenshot" => {
            Command::new("cmd")
                .args(["/C", "start", "", "ms-screenclip:"])
                .spawn()?;
            Ok("Snipping Tool opened".to_string())
        }
        "system_settings" => {
            Command::new("cmd")
                .args(["/C", "start", "", "ms-settings:"])
                .spawn()?;
            Ok("Settings opened".to_string())
        }
        "recycle_bin" => {
            Command::new("cmd")
                .args(["/C", "start", "", "shell:RecycleBinFolder"])
                .spawn()?;
            Ok("Recycle Bin opened".to_string())
        }
        _ => Ok("Unknown command".to_string()),
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn execute_linux(command: &str) -> Result<String> {
    match command {
        "lock" => {
            Command::new("xdg-screensaver").arg("lock").spawn()?;
            Ok("Screen locked".to_string())
        }
        "sleep" => {
            Command::new("systemctl").arg("suspend").spawn()?;
            Ok("Going to sleep".to_string())
        }
        "restart" => {
            Command::new("systemctl").arg("reboot").spawn()?;
            Ok("Restarting".to_string())
        }
        "shutdown" => {
            Command::new("systemctl").arg("poweroff").spawn()?;
            Ok("Shutting down".to_string())
        }
        "system_settings" => {
            Command::new("xdg-open").arg("settings://").spawn()?;
            Ok("System settings opened".to_string())
        }
        _ => Ok("Unknown command".to_string()),
    }
}
