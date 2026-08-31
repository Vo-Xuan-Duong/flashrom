use std::{
    env,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

use serde::Serialize;
use tauri::Emitter;

#[derive(Clone, Copy, Debug)]
pub enum AndroidTool {
    Adb,
    Fastboot,
}

impl AndroidTool {
    fn executable_name(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Self::Adb, true) => "adb.exe",
            (Self::Fastboot, true) => "fastboot.exe",
            (Self::Adb, false) => "adb",
            (Self::Fastboot, false) => "fastboot",
        }
    }
}

#[derive(Debug)]
pub struct CommandOutput {
    pub command: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status == 0
    }

    pub fn combined_output(&self) -> String {
        match (self.stdout.trim(), self.stderr.trim()) {
            ("", "") => String::new(),
            (stdout, "") => stdout.to_string(),
            ("", stderr) => stderr.to_string(),
            (stdout, stderr) => format!("{stdout}\n{stderr}"),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessOutputEvent {
    operation_id: String,
    stream: String,
    data: String,
}

fn tool_path(tool: AndroidTool) -> PathBuf {
    let executable = tool.executable_name();

    if let Ok(directory) = env::var("FLASHROM_PLATFORM_TOOLS") {
        let candidate = PathBuf::from(directory).join(executable);
        if candidate.is_file() {
            return candidate;
        }
    }

    let local = PathBuf::from("tools")
        .join("platform-tools")
        .join(executable);
    if local.is_file() {
        return local;
    }

    PathBuf::from(executable)
}

fn quote_argument(value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn command_text(executable: &Path, args: &[&str]) -> String {
    std::iter::once(executable.to_string_lossy().to_string())
        .chain(args.iter().map(|arg| quote_argument(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_stream<R: Read>(
    mut reader: R,
    app: tauri::AppHandle,
    operation_id: String,
    stream: &'static str,
) -> String {
    let mut collected = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                collected.extend_from_slice(&buffer[..read]);
                let data = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let _ = app.emit(
                    "flashrom-process-output",
                    ProcessOutputEvent {
                        operation_id: operation_id.clone(),
                        stream: stream.into(),
                        data,
                    },
                );
            }
            Err(_) => break,
        }
    }

    String::from_utf8_lossy(&collected).into_owned()
}

pub fn run_executable(executable: &Path, args: &[&str]) -> Result<CommandOutput, String> {
    let command_text = command_text(executable, args);
    let output = Command::new(executable)
        .args(args)
        .output()
        .map_err(|error| format!("Unable to start {}: {error}", executable.display()))?;

    Ok(CommandOutput {
        command: command_text,
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub fn run_executable_streaming(
    app: &tauri::AppHandle,
    operation_id: &str,
    executable: &Path,
    args: &[&str],
) -> Result<CommandOutput, String> {
    let command_text = command_text(executable, args);
    let mut child = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Unable to start {}: {error}", executable.display()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Unable to capture process stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Unable to capture process stderr.".to_string())?;

    let stdout_handle = {
        let app = app.clone();
        let operation_id = operation_id.to_string();
        thread::spawn(move || read_stream(stdout, app, operation_id, "stdout"))
    };
    let stderr_handle = {
        let app = app.clone();
        let operation_id = operation_id.to_string();
        thread::spawn(move || read_stream(stderr, app, operation_id, "stderr"))
    };

    let status = child
        .wait()
        .map_err(|error| format!("Unable to wait for process completion: {error}"))?;
    let stdout = stdout_handle
        .join()
        .map_err(|_| "stdout reader thread failed.".to_string())?;
    let stderr = stderr_handle
        .join()
        .map_err(|_| "stderr reader thread failed.".to_string())?;

    Ok(CommandOutput {
        command: command_text,
        status: status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

pub fn run(tool: AndroidTool, args: &[&str]) -> Result<CommandOutput, String> {
    run_executable(&tool_path(tool), args)
}

pub fn run_streaming(
    app: &tauri::AppHandle,
    operation_id: &str,
    tool: AndroidTool,
    args: &[&str],
) -> Result<CommandOutput, String> {
    run_executable_streaming(app, operation_id, &tool_path(tool), args)
}
