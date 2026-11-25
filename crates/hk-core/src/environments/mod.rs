//! Coordinator-owned snapshots and optimistic file writes shared by web and desktop.
use crate::HkError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
static WRITES: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
const MAX_TEXT: u64 = 4 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
pub struct TextSnapshot {
    pub path: String,
    pub content: String,
    pub revision: String,
    pub captured_at: String,
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn allowed(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, HkError> {
    let canonical = fs::canonicalize(paths::local(&path.to_string_lossy()))?;
    if !roots.iter().filter(|r| !r.as_os_str().is_empty()).any(|r| {
        fs::canonicalize(paths::local(&r.to_string_lossy())).is_ok_and(|r| canonical.starts_with(r))
    }) {
        return Err(HkError::PathNotAllowed(path.display().to_string()));
    }
    Ok(canonical)
}
pub fn read_text(path: &Path, roots: &[PathBuf]) -> Result<TextSnapshot, HkError> {
    let path = allowed(path, roots)?;
    let meta = fs::metadata(&path)?;
    if !meta.is_file() || meta.len() > MAX_TEXT {
        return Err(HkError::Validation(
            "Select a text file smaller than 4 MiB".into(),
        ));
    }
    let bytes = fs::read(&path)?;
    if bytes.contains(&0) {
        return Err(HkError::Validation(
            "Binary files cannot be edited as text".into(),
        ));
    }
    let revision = digest(&bytes);
    let content = String::from_utf8(bytes)
        .map_err(|_| HkError::Validation("File is not UTF-8 text".into()))?;
    Ok(TextSnapshot {
        path: path.to_string_lossy().into_owned(),
        content,
        revision,
        captured_at: chrono::Utc::now().to_rfc3339(),
    })
}
pub fn save_text(
    path: &Path,
    expected: &str,
    content: &str,
    roots: &[PathBuf],
    history: &Path,
) -> Result<TextSnapshot, HkError> {
    let _lock = WRITES.lock();
    if content.len() as u64 > MAX_TEXT || content.contains('\0') {
        return Err(HkError::Validation("Invalid text or file too large".into()));
    }
    let canonical = allowed(path, roots)?;
    let current = read_text(&canonical, roots)?;
    if current.revision != expected {
        return Err(HkError::Conflict(
            "File changed since comparison opened. Reload before saving.".into(),
        ));
    }
    fs::create_dir_all(history)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(history, fs::Permissions::from_mode(0o700))?;
    }
    let mut backup = tempfile::NamedTempFile::new_in(history)?;
    backup.write_all(current.content.as_bytes())?;
    backup.as_file().sync_all()?;
    let backup_path = history.join(&current.revision);
    if !backup_path.exists() {
        backup
            .persist_noclobber(&backup_path)
            .map_err(|e| HkError::Internal(e.to_string()))?;
    }
    let parent = canonical
        .parent()
        .ok_or_else(|| HkError::Validation("No parent directory".into()))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(content.as_bytes())?;
    staged
        .as_file()
        .set_permissions(fs::metadata(&canonical)?.permissions())?;
    staged.as_file().sync_all()?;
    if allowed(path, roots)? != canonical || digest(&fs::read(&canonical)?) != expected {
        return Err(HkError::Conflict("File changed while saving".into()));
    }
    staged
        .persist(&canonical)
        .map_err(|e| HkError::Internal(e.to_string()))?;
    read_text(&canonical, roots)
}
pub fn local_roots(store: &crate::store::Store, home: &Path) -> Result<Vec<PathBuf>, HkError> {
    let mut roots = vec![home.to_path_buf()];
    roots.extend(
        store
            .list_projects()?
            .into_iter()
            .map(|p| PathBuf::from(p.path)),
    );
    Ok(roots)
}
#[cfg(test)]
mod tests;

pub mod reconcile;

pub mod folders;

pub mod registry;

pub mod service;

pub mod transport;

pub mod inventory;

pub fn environment_text(
    home: &Path,
    environment: &str,
    provider: &str,
    path: &str,
    save: Option<(&str, &str)>,
) -> Result<TextSnapshot, HkError> {
    let c = registry::Coordinator::new(home.join(".harnesskit/environments"));
    let view = c.overview()?;
    let env = view
        .registry
        .environments
        .iter()
        .find(|e| e.id == environment)
        .ok_or_else(|| HkError::NotFound("Environment".into()))?;
    let p = env
        .providers
        .iter()
        .find(|p| p.id == provider)
        .ok_or_else(|| HkError::NotFound("Provider".into()))?;
    let resolved_path = paths::for_connection(path, &env.connection);
    let path = resolved_path.as_str();
    let file = path.rsplit(['/', '\\']).next().unwrap_or("");
    if ![
        "config.toml",
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        "settings.json",
        "config.json",
        "mcp.json",
    ]
    .contains(&file)
    {
        return Err(HkError::Validation(
            "Unsupported provider configuration file".into(),
        ));
    }
    let root = p.root.replace('\\', "/");
    let parent = root
        .trim_end_matches('/')
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    if path.replace('\\', "/") != format!("{parent}/{file}") {
        return Err(HkError::Conflict(
            "Provider path changed; reload this comparison".into(),
        ));
    }
    if matches!(env.connection, transport::Connection::Local) {
        let parent = Path::new(&p.root)
            .parent()
            .ok_or_else(|| HkError::Validation("Provider root has no parent".into()))?;
        let file = parent.join(file);
        let roots = [parent.to_path_buf()];
        return match save {
            Some((expected, content)) => save_text(
                &file,
                expected,
                content,
                &roots,
                &home.join(".harnesskit/comparison-history"),
            ),
            None => read_text(&file, &roots),
        };
    }
    let mut req = serde_json::json!({"action":"text_read","root":p.root,"file":file});
    if let Some((expected, content)) = save {
        req["action"] = serde_json::json!("text_save");
        req["expected"] = serde_json::json!(expected);
        req["content"] = serde_json::json!(content);
    }
    Ok(serde_json::from_value(transport::run(
        &env.connection,
        req,
    )?)?)
}

pub mod installs;

pub mod backup;

pub mod windows_host;

pub mod paths;

pub mod wsl_host;

pub mod catalog;

pub mod discovery;
pub mod nested;

pub mod managed_files;
