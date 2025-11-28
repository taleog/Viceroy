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
    vec![
        SystemCommand {
            name: "Lock Screen".to_string(),
            description: "Lock the screen immediately".to_string(),
            command: "lock".to_string(),
        },
        SystemCommand {
            name: "Sleep".to_string(),
            description: "Put the computer to sleep".to_string(),
            command: "sleep".to_string(),
        },
        SystemCommand {
            name: "Restart".to_string(),
            description: "Restart the computer".to_string(),
            command: "restart".to_string(),
        },
        SystemCommand {
            name: "Shutdown".to_string(),
            description: "Shut down the computer".to_string(),
            command: "shutdown".to_string(),
        },
        SystemCommand {
            name: "Volume Up".to_string(),
            description: "Increase system volume".to_string(),
            command: "volume_up".to_string(),
        },
        SystemCommand {
            name: "Volume Down".to_string(),
            description: "Decrease system volume".to_string(),
            command: "volume_down".to_string(),
        },
        SystemCommand {
            name: "Mute".to_string(),
            description: "Toggle mute".to_string(),
            command: "mute".to_string(),
        },
        SystemCommand {
            name: "Empty Trash".to_string(),
            description: "Empty the trash".to_string(),
            command: "empty_trash".to_string(),
        },
        SystemCommand {
            name: "Show Hidden Files".to_string(),
            description: "Toggle hidden files visibility in Finder".to_string(),
            command: "toggle_hidden_files".to_string(),
        },
        SystemCommand {
            name: "Screenshot".to_string(),
            description: "Take a screenshot".to_string(),
            command: "screenshot".to_string(),
        },
        SystemCommand {
            name: "Color Picker".to_string(),
            description: "Open color picker".to_string(),
            command: "color_picker".to_string(),
        },
        SystemCommand {
            name: "System Settings".to_string(),
            description: "Open System Settings".to_string(),
            command: "system_settings".to_string(),
        },
    ]
}

pub async fn execute(command: &str) -> Result<String> {
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
