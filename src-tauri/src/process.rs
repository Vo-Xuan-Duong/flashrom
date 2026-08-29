use std::{env, path::PathBuf, process::Command};

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

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Adb => "adb",
            Self::Fastboot => "fastboot",
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
    if value.contains(char::is_whitespace) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

pub fn run(tool: AndroidTool, args: &[&str]) -> Result<CommandOutput, String> {
    let executable = tool_path(tool);
    let command_text = std::iter::once(executable.to_string_lossy().to_string())
        .chain(args.iter().map(|arg| quote_argument(arg)))
        .collect::<Vec<_>>()
        .join(" ");

    let output = Command::new(&executable)
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
