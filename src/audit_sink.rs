use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::Mutex;

use thiserror::Error;

use crate::audit::AuditEvent;

pub struct AuditSink {
    file: Mutex<File>,
}

impl AuditSink {
    pub fn open(path: &Path) -> Result<Self, AuditSinkError> {
        let parent = path.parent().ok_or(AuditSinkError::MissingParent)?;
        fs::create_dir_all(parent).map_err(|_| AuditSinkError::CreateDirectory)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| AuditSinkError::DirectoryPermissions)?;
        let parent_metadata =
            fs::symlink_metadata(parent).map_err(|_| AuditSinkError::DirectoryMetadata)?;
        if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
            return Err(AuditSinkError::UnsafeDirectory);
        }

        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(AuditSinkError::UnsafeFile);
            }
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(AuditSinkError::PermissiveFileMode);
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| AuditSinkError::Open)?;
        let metadata = file.metadata().map_err(|_| AuditSinkError::FileMetadata)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(AuditSinkError::PermissiveFileMode);
        }
        Ok(Self {
            file: Mutex::new(file),
        })
    }

    pub fn write(&self, event: &AuditEvent) -> Result<(), AuditSinkError> {
        let bytes = event
            .to_json_line()
            .map_err(|_| AuditSinkError::Serialize)?;
        let mut file = self.file.lock().map_err(|_| AuditSinkError::Poisoned)?;
        file.write_all(&bytes).map_err(|_| AuditSinkError::Write)?;
        file.flush().map_err(|_| AuditSinkError::Flush)
    }
}

#[derive(Debug, Error)]
pub enum AuditSinkError {
    #[error("audit path has no parent directory")]
    MissingParent,
    #[error("audit directory creation failed")]
    CreateDirectory,
    #[error("audit directory permissions could not be secured")]
    DirectoryPermissions,
    #[error("audit directory metadata could not be read")]
    DirectoryMetadata,
    #[error("audit directory is unsafe")]
    UnsafeDirectory,
    #[error("audit file is unsafe")]
    UnsafeFile,
    #[error("audit file permissions are too broad")]
    PermissiveFileMode,
    #[error("audit file could not be opened securely")]
    Open,
    #[error("audit file metadata could not be read")]
    FileMetadata,
    #[error("audit event serialization failed")]
    Serialize,
    #[error("audit writer lock is unavailable")]
    Poisoned,
    #[error("audit event write failed")]
    Write,
    #[error("audit event flush failed")]
    Flush,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use crate::audit::{AuditDecision, UpstreamOutcome};

    use super::*;

    fn temporary_path(name: &str) -> PathBuf {
        let mut random = [0_u8; 8];
        getrandom::fill(&mut random).expect("test randomness should be available");
        let suffix = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        std::env::temp_dir()
            .join(format!("agentveil-audit-{suffix}"))
            .join(name)
    }

    fn event() -> AuditEvent {
        AuditEvent::new(
            "av_evt_12345678".to_string(),
            1,
            "av_session_12345678".to_string(),
            1,
            AuditDecision::Allowed,
            Vec::new(),
            UpstreamOutcome::Completed,
            1,
            "default".to_string(),
            "a".repeat(64),
            "0.1.0".to_string(),
        )
        .expect("event should validate")
    }

    #[test]
    fn creates_private_directory_and_file() {
        let path = temporary_path("audit.jsonl");
        let sink = AuditSink::open(&path).expect("audit sink should open");
        sink.write(&event()).expect("audit should write");
        let parent_mode = fs::metadata(path.parent().expect("parent should exist"))
            .expect("parent metadata should exist")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = fs::metadata(&path)
            .expect("file metadata should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(parent_mode, 0o700);
        assert_eq!(file_mode, 0o600);
        let _ = fs::remove_dir_all(path.parent().expect("parent should exist"));
    }

    #[test]
    fn rejects_existing_permissive_file() {
        let path = temporary_path("audit.jsonl");
        fs::create_dir_all(path.parent().expect("parent should exist"))
            .expect("directory should create");
        fs::write(&path, b"").expect("fixture should create");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("fixture mode should set");
        assert!(matches!(
            AuditSink::open(&path),
            Err(AuditSinkError::PermissiveFileMode)
        ));
        let _ = fs::remove_dir_all(path.parent().expect("parent should exist"));
    }
}
