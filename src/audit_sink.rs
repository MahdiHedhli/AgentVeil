use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use thiserror::Error;

use crate::audit::AuditEvent;

pub struct AuditSink {
    file: Mutex<File>,
}

impl AuditSink {
    pub fn open(path: &Path) -> Result<Self, AuditSinkError> {
        let parent = path.parent().ok_or(AuditSinkError::MissingParent)?;
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        let secure_parent = ensure_private_directory(parent)?;
        let file_name = path.file_name().ok_or(AuditSinkError::UnsafeFile)?;
        let secure_path = secure_parent.join(file_name);

        if let Ok(metadata) = fs::symlink_metadata(&secure_path) {
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
            .open(&secure_path)
            .map_err(|_| AuditSinkError::Open)?;
        let metadata = file.metadata().map_err(|_| AuditSinkError::FileMetadata)?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.permissions().mode() & 0o077 != 0
        {
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
        file.flush().map_err(|_| AuditSinkError::Flush)?;
        file.sync_data().map_err(|_| AuditSinkError::Sync)
    }
}

fn ensure_private_directory(path: &Path) -> Result<PathBuf, AuditSinkError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let base = path.parent().ok_or(AuditSinkError::MissingParent)?;
            let base = if base.as_os_str().is_empty() {
                Path::new(".")
            } else {
                base
            };
            let canonical_base =
                fs::canonicalize(base).map_err(|_| AuditSinkError::DirectoryMetadata)?;
            let leaf = path.file_name().ok_or(AuditSinkError::UnsafeDirectory)?;
            let secure_path = canonical_base.join(leaf);
            let mut builder = DirBuilder::new();
            builder.mode(0o700);
            match builder.create(&secure_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(AuditSinkError::CreateDirectory),
            }
            return validate_private_directory(&secure_path);
        }
        Err(_) => return Err(AuditSinkError::DirectoryMetadata),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AuditSinkError::UnsafeDirectory);
    }
    validate_private_directory(path)
}

fn validate_private_directory(path: &Path) -> Result<PathBuf, AuditSinkError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| AuditSinkError::DirectoryMetadata)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AuditSinkError::UnsafeDirectory);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(AuditSinkError::PermissiveDirectoryMode);
    }
    fs::canonicalize(path).map_err(|_| AuditSinkError::DirectoryMetadata)
}

#[derive(Debug, Error)]
pub enum AuditSinkError {
    #[error("audit path has no parent directory")]
    MissingParent,
    #[error("audit directory creation failed")]
    CreateDirectory,
    #[error("audit directory metadata could not be read")]
    DirectoryMetadata,
    #[error("audit directory is unsafe")]
    UnsafeDirectory,
    #[error("audit directory permissions are too broad")]
    PermissiveDirectoryMode,
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
    #[error("audit event durability sync failed")]
    Sync,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;
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
        let parent = path.parent().expect("parent should exist");
        fs::create_dir_all(parent).expect("directory should create");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .expect("parent mode should set");
        fs::write(&path, b"").expect("fixture should create");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("fixture mode should set");
        assert!(matches!(
            AuditSink::open(&path),
            Err(AuditSinkError::PermissiveFileMode)
        ));
        let _ = fs::remove_dir_all(path.parent().expect("parent should exist"));
    }

    #[test]
    fn rejects_hard_linked_audit_file() {
        let path = temporary_path("audit.jsonl");
        let parent = path.parent().expect("parent should exist");
        fs::create_dir_all(parent).expect("directory should create");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .expect("parent mode should set");
        fs::write(&path, b"").expect("fixture should create");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("fixture mode should set");
        fs::hard_link(&path, parent.join("second-link")).expect("hard link should create");

        assert!(matches!(
            AuditSink::open(&path),
            Err(AuditSinkError::PermissiveFileMode)
        ));
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn rejects_permissive_parent_without_changing_its_mode() {
        let path = temporary_path("audit.jsonl");
        let parent = path.parent().expect("parent should exist");
        fs::create_dir_all(parent).expect("directory should create");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o755))
            .expect("fixture mode should set");

        assert!(matches!(
            AuditSink::open(&path),
            Err(AuditSinkError::PermissiveDirectoryMode)
        ));
        let mode = fs::metadata(parent)
            .expect("parent metadata should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn rejects_symlink_parent_without_changing_target_mode() {
        let path = temporary_path("audit.jsonl");
        let link_parent = path.parent().expect("link parent should exist");
        let root = link_parent.parent().expect("test root should exist");
        let target = root.join(format!(
            "agentveil-audit-target-{}",
            link_parent
                .file_name()
                .and_then(|name| name.to_str())
                .expect("fixture name should be UTF-8")
        ));
        fs::create_dir_all(&target).expect("target should create");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
            .expect("target mode should set");
        symlink(&target, link_parent).expect("parent symlink should create");

        assert!(matches!(
            AuditSink::open(&path),
            Err(AuditSinkError::UnsafeDirectory)
        ));
        let mode = fs::metadata(&target)
            .expect("target metadata should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);
        let _ = fs::remove_file(link_parent);
        let _ = fs::remove_dir_all(&target);
    }
}
