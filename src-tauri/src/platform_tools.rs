use std::{env, path::PathBuf};

use serde::Serialize;

use crate::process::{run, AndroidTool};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    name: String,
    path: String,
    available: bool,
    version: Option<String>,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformToolsStatus {
    source: String,
    adb: ToolStatus,
    fastboot: ToolStatus,
    ready: bool,
    diagnostic: String,
}

fn executable_name(tool: AndroidTool) -> &'static str {
    match (tool, cfg!(windows)) {
        (AndroidTool::Adb, true) => "adb.exe",
        (AndroidTool::Fastboot, true) => "fastboot.exe",
        (AndroidTool::Adb, false) => "adb",
        (AndroidTool::Fastboot, false) => "fastboot",
    }
}

fn resolve(tool: AndroidTool) -> (String, String) {
    let executable = executable_name(tool);
    if let Ok(directory) = env::var("FLASHROM_PLATFORM_TOOLS") {
        let path = PathBuf::from(&directory).join(executable);
        return ("FLASHROM_PLATFORM_TOOLS".into(), path.to_string_lossy().to_string());
    }
    let local = PathBuf::from("tools").join("platform-tools").join(executable);
    if local.is_file() {
        return ("bundled/local tools/platform-tools".into(), local.to_string_lossy().to_string());
    }
    ("system PATH".into(), executable.into())
}

fn first_version_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn inspect(tool: AndroidTool) -> ToolStatus {
    let (_, path) = resolve(tool);
    let args: &[&str] = match tool {
        AndroidTool::Adb => &["version"],
        AndroidTool::Fastboot => &["--version"],
    };
    let name = match tool {
        AndroidTool::Adb => "adb",
        AndroidTool::Fastboot => "fastboot",
    };

    match run(tool, args) {
        Ok(output) => {
            let available = output.success();
            let combined = output.combined_output();
            ToolStatus {
                name: name.into(),
                path,
                available,
                version: first_version_line(&combined),
                diagnostic: if available {
                    format!("{name} executed successfully.")
                } else {
                    format!("{name} returned exit code {}: {combined}", output.status)
                },
            }
        }
        Err(error) => ToolStatus {
            name: name.into(),
            path,
            available: false,
            version: None,
            diagnostic: error,
        },
    }
}

#[tauri::command]
pub fn inspect_platform_tools() -> Result<PlatformToolsStatus, String> {
    let (source, _) = resolve(AndroidTool::Adb);
    let adb = inspect(AndroidTool::Adb);
    let fastboot = inspect(AndroidTool::Fastboot);
    let ready = adb.available && fastboot.available;
    Ok(PlatformToolsStatus {
        source,
        adb,
        fastboot,
        ready,
        diagnostic: if ready {
            "ADB and Fastboot are both available for FlashROM workflows.".into()
        } else {
            "Android Platform Tools are incomplete. Configure FLASHROM_PLATFORM_TOOLS, tools/platform-tools, or system PATH."
                .into()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::first_version_line;

    #[test]
    fn extracts_version_line() {
        assert_eq!(first_version_line("fastboot version 37.0.0\nInstalled as x"), Some("fastboot version 37.0.0".into()));
    }
}
