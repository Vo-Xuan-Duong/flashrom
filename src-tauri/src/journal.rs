use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalStep {
    index: usize,
    image: String,
    partition: String,
    required_mode: String,
    status: String,
    command: Option<String>,
    exit_code: Option<i32>,
    diagnostic: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionJournalRecord {
    version: u8,
    operation_id: String,
    serial: String,
    product: Option<String>,
    rom_path: String,
    slot_strategy: String,
    status: String,
    started_unix_ms: u64,
    updated_unix_ms: u64,
    clean_data_requested: bool,
    reboot_requested: bool,
    steps: Vec<JournalStep>,
    diagnostic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(default)]
    recoverable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalSummary {
    operation_id: String,
    serial: String,
    product: Option<String>,
    rom_path: String,
    status: String,
    started_unix_ms: u64,
    updated_unix_ms: u64,
    completed_steps: usize,
    failed_steps: usize,
    total_steps: usize,
    recoverable: bool,
    path: String,
    diagnostic: String,
}

fn journal_directory() -> PathBuf {
    let root = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    root.join("FlashROM").join("journals")
}

fn canonical_or_original(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn validate_journal_path(path: &Path) -> Result<(), String> {
    let base = canonical_or_original(&journal_directory());
    let candidate = canonical_or_original(path);
    if !candidate.starts_with(&base) {
        return Err("Journal path must remain inside the FlashROM journal directory.".into());
    }
    if candidate.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err("Execution journal must be a .json file.".into());
    }
    Ok(())
}

fn is_recoverable(record: &ExecutionJournalRecord) -> bool {
    matches!(record.status.as_str(), "running" | "failed") && Path::new(&record.rom_path).exists()
}

fn read_journal(path: &Path) -> Result<ExecutionJournalRecord, String> {
    validate_journal_path(path)?;
    let bytes = fs::read(path)
        .map_err(|error| format!("Unable to read operation journal {}: {error}", path.display()))?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Operation journal is unexpectedly large and was rejected.".into());
    }
    let mut record: ExecutionJournalRecord = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Unable to parse operation journal {}: {error}", path.display()))?;
    record.path = Some(path.to_string_lossy().to_string());
    record.recoverable = is_recoverable(&record);
    Ok(record)
}

fn summary(record: &ExecutionJournalRecord, path: &Path) -> JournalSummary {
    let completed_steps = record.steps.iter().filter(|step| step.status == "success").count();
    let failed_steps = record.steps.iter().filter(|step| step.status == "failed").count();
    JournalSummary {
        operation_id: record.operation_id.clone(),
        serial: record.serial.clone(),
        product: record.product.clone(),
        rom_path: record.rom_path.clone(),
        status: record.status.clone(),
        started_unix_ms: record.started_unix_ms,
        updated_unix_ms: record.updated_unix_ms,
        completed_steps,
        failed_steps,
        total_steps: record.steps.len(),
        recoverable: record.recoverable,
        path: path.to_string_lossy().to_string(),
        diagnostic: record.diagnostic.clone(),
    }
}

#[tauri::command]
pub fn list_execution_journals() -> Result<Vec<JournalSummary>, String> {
    let directory = journal_directory();
    if !directory.is_dir() {
        return Ok(Vec::new());
    }

    let mut values = fs::read_dir(&directory)
        .map_err(|error| format!("Unable to list operation journals: {error}"))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                return None;
            }
            read_journal(&path).ok().map(|record| summary(&record, &path))
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.updated_unix_ms.cmp(&left.updated_unix_ms));
    values.truncate(50);
    Ok(values)
}

#[tauri::command]
pub fn inspect_execution_journal(path: String) -> Result<ExecutionJournalRecord, String> {
    read_journal(Path::new(&path))
}

#[tauri::command]
pub fn delete_execution_journal(path: String, confirmation: String) -> Result<(), String> {
    if confirmation != "DELETE JOURNAL" {
        return Err("Journal deletion requires the exact confirmation DELETE JOURNAL.".into());
    }
    let path = PathBuf::from(path);
    validate_journal_path(&path)?;
    fs::remove_file(&path)
        .map_err(|error| format!("Unable to delete operation journal {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::journal_directory;

    #[test]
    fn journal_directory_is_scoped() {
        assert!(
            journal_directory().ends_with("FlashROM\\journals")
                || journal_directory().ends_with("FlashROM/journals")
        );
    }
}
