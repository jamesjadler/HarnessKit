use super::digest;
use crate::HkError;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Component, Path},
};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Folder {
    pub files: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub modes: BTreeMap<String, u32>,
    #[serde(default)]
    pub directories: Vec<String>,
    pub fingerprint: String,
}
impl Folder {
    pub fn new(files: BTreeMap<String, Vec<u8>>) -> Self {
        let modes = files.keys().map(|k| (k.clone(), 0o644)).collect();
        let mut directories = std::collections::BTreeSet::new();
        for name in files.keys() {
            let mut parent = Path::new(name).parent();
            while let Some(p) = parent {
                if p.as_os_str().is_empty() {
                    break;
                }
                directories.insert(p.to_string_lossy().replace('\\', "/"));
                parent = p.parent();
            }
        }
        Self::with_metadata(files, modes, directories.into_iter().collect())
    }
    pub fn with_metadata(
        files: BTreeMap<String, Vec<u8>>,
        modes: BTreeMap<String, u32>,
        mut directories: Vec<String>,
    ) -> Self {
        directories.sort();
        let fingerprint = digest(
            &serde_json::to_vec(&(&files, &modes, &directories)).expect("snapshot serializes"),
        );
        Self {
            files,
            modes,
            directories,
            fingerprint,
        }
    }
}
pub fn read_folder(root: &Path) -> Result<Option<Folder>, HkError> {
    recover_folder(root)?;
    match fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
        Ok(m) if m.file_type().is_symlink() => {
            return Err(HkError::Validation(
                "Skill folder is a symlink; configure its resolved directory explicitly".into(),
            ));
        }
        Ok(m) if !m.is_dir() => {
            return Err(HkError::Validation("Skill path must be a directory".into()));
        }
        _ => {}
    }
    let mut files = BTreeMap::new();
    let mut modes = BTreeMap::new();
    let mut directories = vec![];
    let mut size = 0;
    for e in walkdir::WalkDir::new(root).follow_links(false) {
        let e = e.map_err(|e| HkError::Internal(e.to_string()))?;
        if e.file_type().is_symlink() {
            return Err(HkError::Validation(format!(
                "Symlink needs review: {}",
                e.path().display()
            )));
        }
        if e.file_type().is_dir() && e.path() != root {
            directories.push(
                e.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
        if !e.file_type().is_file() {
            continue;
        }
        size += e
            .metadata()
            .map_err(|e| HkError::Internal(e.to_string()))?
            .len();
        if size > 64 * 1024 * 1024 || files.len() >= 10000 {
            return Err(HkError::Validation(
                "Skill exceeds 64 MiB / 10,000 files".into(),
            ));
        }
        let name = e
            .path()
            .strip_prefix(root)
            .map_err(|e| HkError::Internal(e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            e.metadata()
                .map_err(|e| HkError::Internal(e.to_string()))?
                .permissions()
                .mode()
                & 0o777
        };
        #[cfg(not(unix))]
        let mode = 0o644;
        modes.insert(name.clone(), mode);
        files.insert(name, fs::read(e.path())?);
    }
    Ok(Some(Folder::with_metadata(files, modes, directories)))
}
fn safe_relative(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && !name.contains(':')
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
/// Stage a whole folder beside its destination; preserve the old folder until commit succeeds.
pub fn replace_folder(
    path: &Path,
    expected: Option<&str>,
    folder: Option<&Folder>,
    history: &Path,
) -> Result<(), HkError> {
    let _lock = super::WRITES.lock();
    let current = read_folder(path)?;
    if current.as_ref().map(|f| f.fingerprint.as_str()) != expected {
        return Err(HkError::Conflict(
            "Target changed after preview; rescan required".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| HkError::Validation("Missing parent".into()))?;
    fs::create_dir_all(parent)?;
    fs::create_dir_all(history)?;
    let staged = tempfile::Builder::new()
        .prefix(".hk-stage-")
        .tempdir_in(parent)?;
    if let Some(folder) = folder {
        if Folder::with_metadata(
            folder.files.clone(),
            folder.modes.clone(),
            folder.directories.clone(),
        )
        .fingerprint
            != folder.fingerprint
        {
            return Err(HkError::Validation("Snapshot checksum mismatch".into()));
        }
        for name in &folder.directories {
            if !safe_relative(name) {
                return Err(HkError::Validation("Unsafe directory path".into()));
            }
            fs::create_dir_all(staged.path().join(name))?;
        }
        for (name, data) in &folder.files {
            if !safe_relative(name) {
                return Err(HkError::Validation("Unsafe snapshot path".into()));
            }
            let target = staged.path().join(name);
            fs::create_dir_all(target.parent().unwrap())?;
            let mut f = fs::File::create(target)?;
            f.write_all(data)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                f.set_permissions(fs::Permissions::from_mode(
                    folder.modes.get(name).copied().unwrap_or(0o644) & 0o777,
                ))?;
            }
            f.sync_all()?;
        }
    }
    if read_folder(path)?.as_ref().map(|f| f.fingerprint.as_str()) != expected {
        return Err(HkError::Conflict("Target changed while staging".into()));
    }
    let old = parent.join(format!(".hk-old-{}", uuid::Uuid::new_v4()));
    let journal = journal_path(path)?;
    let record = serde_json::json!({"old":old.file_name().unwrap().to_string_lossy(),"stage":staged.path().file_name().unwrap().to_string_lossy()});
    let mut transaction = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut transaction, &record)?;
    transaction.as_file().sync_all()?;
    transaction
        .persist(&journal)
        .map_err(|e| HkError::Internal(e.to_string()))?;
    if let Some(current) = &current {
        let backup = history.join(format!("{}.json", current.fingerprint));
        if !backup.exists() {
            let mut f = tempfile::NamedTempFile::new_in(history)?;
            serde_json::to_writer(&mut f, current)?;
            f.as_file().sync_all()?;
            f.persist_noclobber(backup)
                .map_err(|e| HkError::Internal(e.to_string()))?;
        }
        fs::rename(path, &old)?;
    }
    if folder.is_some() {
        if let Err(e) = fs::rename(staged.path(), path) {
            if current.is_some() {
                fs::rename(&old, path)?;
            }
            return Err(e.into());
        }
    }
    fs::remove_file(&journal)?;
    if old.exists() {
        fs::remove_dir_all(old)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_folder_is_preserved() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("skill");
        fs::create_dir(&p).unwrap();
        fs::write(p.join("SKILL.md"), "one").unwrap();
        let before = read_folder(&p).unwrap().unwrap();
        fs::write(p.join("SKILL.md"), "two").unwrap();
        assert!(
            replace_folder(
                &p,
                Some(&before.fingerprint),
                None,
                &t.path().join("history")
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(p.join("SKILL.md")).unwrap(), "two");
    }
    #[test]
    fn snapshot_preserves_support_files_and_restores_removal() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("skill");
        let f = Folder::new(BTreeMap::from([
            ("SKILL.md".into(), b"skill".to_vec()),
            ("assets/a.bin".into(), vec![0, 255]),
        ]));
        let h = t.path().join("history");
        replace_folder(&p, None, Some(&f), &h).unwrap();
        assert_eq!(read_folder(&p).unwrap(), Some(f.clone()));
        replace_folder(&p, Some(&f.fingerprint), None, &h).unwrap();
        assert!(!p.exists());
        replace_folder(&p, None, Some(&f), &h).unwrap();
        assert_eq!(read_folder(&p).unwrap(), Some(f));
    }
    #[test]
    fn snapshot_path_traversal_rejected() {
        let t = tempfile::tempdir().unwrap();
        let f = Folder::new(BTreeMap::from([("../escape".into(), vec![1])]));
        assert!(
            replace_folder(
                &t.path().join("skill"),
                None,
                Some(&f),
                &t.path().join("history")
            )
            .is_err()
        );
        assert!(!t.path().join("escape").exists());
    }
}

fn journal_path(path: &Path) -> Result<std::path::PathBuf, HkError> {
    let parent = path
        .parent()
        .ok_or_else(|| HkError::Validation("Missing parent".into()))?;
    let name = path
        .file_name()
        .ok_or_else(|| HkError::Validation("Missing name".into()))?
        .to_string_lossy();
    Ok(parent.join(format!(".hk-transaction-{}.json", digest(name.as_bytes()))))
}
fn recover_folder(path: &Path) -> Result<(), HkError> {
    let journal = journal_path(path)?;
    let bytes = match fs::read(&journal) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let old = value["old"]
        .as_str()
        .ok_or_else(|| HkError::ConfigCorrupted("Invalid folder transaction".into()))?;
    if !safe_relative(old) || old.contains('/') || !old.starts_with(".hk-old-") {
        return Err(HkError::ConfigCorrupted("Unsafe transaction backup".into()));
    }
    let backup = path.parent().unwrap().join(old);
    if !path.exists() && backup.exists() {
        fs::rename(backup, path)?;
    }
    fs::remove_file(journal)?;
    Ok(())
}
#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn interrupted_folder_replacement_restores_original_before_scanning() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("skill");
        let backup = t.path().join(".hk-old-test");
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("SKILL.md"), "original").unwrap();
        fs::write(journal_path(&path).unwrap(), r#"{"old":".hk-old-test"}"#).unwrap();
        let snapshot = read_folder(&path).unwrap().unwrap();
        assert_eq!(snapshot.files["SKILL.md"], b"original");
        assert!(!journal_path(&path).unwrap().exists());
    }
}
