use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

#[derive(Clone, Debug)]
struct ActiveOperation {
    kind: String,
    serial: String,
    started_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    active: bool,
    kind: Option<String>,
    serial: Option<String>,
    started_unix_ms: Option<u64>,
}

#[derive(Clone, Default)]
pub struct OperationManager {
    current: Arc<Mutex<Option<ActiveOperation>>>,
}

pub struct OperationPermit {
    manager: OperationManager,
    kind: String,
    serial: String,
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

impl OperationManager {
    pub fn acquire(&self, kind: &str, serial: &str) -> Result<OperationPermit, String> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| "Operation lock is poisoned.".to_string())?;

        if let Some(active) = current.as_ref() {
            return Err(format!(
                "Another protected operation is already running: {} on device {}.",
                active.kind, active.serial
            ));
        }

        *current = Some(ActiveOperation {
            kind: kind.to_string(),
            serial: serial.to_string(),
            started_unix_ms: now_unix_ms(),
        });

        Ok(OperationPermit {
            manager: self.clone(),
            kind: kind.to_string(),
            serial: serial.to_string(),
        })
    }

    pub fn status(&self) -> Result<OperationStatus, String> {
        let current = self
            .current
            .lock()
            .map_err(|_| "Operation lock is poisoned.".to_string())?;

        if let Some(active) = current.as_ref() {
            Ok(OperationStatus {
                active: true,
                kind: Some(active.kind.clone()),
                serial: Some(active.serial.clone()),
                started_unix_ms: Some(active.started_unix_ms),
            })
        } else {
            Ok(OperationStatus {
                active: false,
                kind: None,
                serial: None,
                started_unix_ms: None,
            })
        }
    }
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        if let Ok(mut current) = self.manager.current.lock() {
            let matches_permit = current
                .as_ref()
                .map(|active| active.kind == self.kind && active.serial == self.serial)
                .unwrap_or(false);
            if matches_permit {
                *current = None;
            }
        }
    }
}

#[tauri::command]
pub fn get_operation_status(
    manager: tauri::State<'_, OperationManager>,
) -> Result<OperationStatus, String> {
    manager.status()
}

#[cfg(test)]
mod tests {
    use super::OperationManager;

    #[test]
    fn serializes_protected_operations() {
        let manager = OperationManager::default();
        let permit = manager.acquire("flash", "ABC").expect("first acquire");
        assert!(manager.acquire("wipe", "ABC").is_err());
        drop(permit);
        assert!(manager.acquire("wipe", "ABC").is_ok());
    }
}
