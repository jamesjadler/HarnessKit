//! Windows profile discovery from WSL; no SSH or remote helper required.
use super::registry::{Environment, Registry};
use crate::{HkError, adapter::AgentAdapter};
use std::path::Path;

/// Resolve the signed-in Windows profile once per process. Tests and isolated
/// application homes never probe the real host.
pub fn detect(home: &Path) -> Option<(std::path::PathBuf, String)> {
    if std::env::var_os("WSL_DISTRO_NAME").is_none()
        || std::env::var_os("HARNESSKIT_HOME").is_some()
        || home != crate::home_dir()
    {
        return None;
    }
    static HOST: std::sync::OnceLock<Option<(std::path::PathBuf, String)>> =
        std::sync::OnceLock::new();
    HOST.get_or_init(|| {
        let cmd = if Path::new("/mnt/c/Windows/System32/cmd.exe").is_file() {
            "/mnt/c/Windows/System32/cmd.exe"
        } else {
            "cmd.exe"
        };
        let output = crate::command(cmd)
            .current_dir("/mnt/c")
            .args(["/d", "/c", "echo %USERPROFILE%&echo %COMPUTERNAME%"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let mut lines = text.lines().map(str::trim).filter(|s| !s.is_empty());
        let profile = translate(lines.next()?)?;
        let machine = lines.next()?.to_owned();
        let profile = std::path::PathBuf::from(profile);
        profile.is_dir().then_some((profile, machine))
    })
    .clone()
}

pub fn translate(path: &str) -> Option<String> {
    let out = crate::command("wslpath").args(["-u", path]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    Path::new(&path).is_absolute().then_some(path)
}

pub fn reconcile(
    registry: &mut Registry,
    profile: &Path,
    machine: &str,
    home: &Path,
    adapters: &[Box<dyn AgentAdapter>],
    translate: impl Fn(&str) -> Option<String>,
) -> Result<(), HkError> {
    use rusqlite::OptionalExtension;
    registry.path_homes.insert(
        "windows-host".into(),
        profile.to_string_lossy().into_owned(),
    );
    let db_path = profile.join(".harnesskit/metadata.db");
    let db = if db_path.is_file() {
        Some(rusqlite::Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?)
    } else {
        None
    };
    if !registry.environments.iter().any(|e| e.id == "windows-host") {
        registry.environments.push(Environment {
            id: "windows-host".into(),
            machine: machine.into(),
            name: "Windows".into(),
            os: "windows".into(),
            connection: super::transport::Connection::Local,
            paused: true,
            providers: vec![],
        });
    }
    if let Some(local) = registry.environments.iter_mut().find(|e| e.id == "local") {
        local.machine = machine.into();
    }
    registry
        .catalog
        .retain(|c| c.environment != "windows-host" || c.project.is_some());
    for adapter in adapters {
        let setting: Option<(Option<String>, bool)> = match &db {
            Some(db) => db
                .query_row(
                    "SELECT custom_path, enabled FROM agent_settings WHERE name = ?1",
                    [adapter.name()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?,
            None => None,
        };
        let (custom, enabled) = setting.unwrap_or((None, true));
        let (base, path_error) = match custom {
            Some(path) => match translate(&path) {
                Some(mapped) => (std::path::PathBuf::from(mapped), None),
                None => (
                    std::path::PathBuf::from(&path),
                    Some(format!(
                        "Cannot access Windows {} path: {path}",
                        adapter.name()
                    )),
                ),
            },
            None => match adapter.base_dir().strip_prefix(home) {
                Ok(relative) => (profile.join(relative), None),
                Err(_) => (profile.join(format!(".{}", adapter.name())), None),
            },
        };
        let target_roots = match adapter.name() {
            "penguin" => {
                Some(crate::adapter::penguin::PenguinAdapter::with_root(base.clone()).skill_dirs())
            }
            "antigravity" => {
                let mut roots = vec![
                    base.join("skills"),
                    profile.join(".gemini/antigravity/skills"),
                ];
                if profile.join(".gemini/antigravity-cli").is_dir() {
                    roots.extend([
                        profile.join(".gemini/antigravity-cli/skills"),
                        profile.join(".gemini/skills"),
                    ]);
                }
                Some(roots)
            }
            _ => None,
        };
        let target_specific = target_roots.is_some();
        let mut defaults = target_roots.unwrap_or_else(|| adapter.skill_dirs());
        if defaults.is_empty() {
            defaults.push(if target_specific {
                base.join("skills")
            } else {
                adapter.base_dir().join("skills")
            });
        }
        for (index, default) in defaults.iter().enumerate() {
            let adapter_base = adapter.base_dir();
            let mapped = if target_specific {
                default.clone()
            } else if let Ok(relative) = default.strip_prefix(&adapter_base) {
                base.join(relative)
            } else if let Ok(relative) = default.strip_prefix(home) {
                profile.join(relative)
            } else {
                continue;
            };
            let id = format!("windows:{}:{index}", adapter.name());
            let mapped = super::catalog::provider_root(registry, "windows-host", &id, mapped);
            let mapped = registry
                .catalog_settings
                .get(&super::nested::key(
                    "windows-host",
                    &format!("home:{}", adapter.name()),
                ))
                .and_then(|s| s.path.as_ref())
                .and_then(|home| {
                    mapped
                        .strip_prefix(&base)
                        .ok()
                        .map(|relative| std::path::PathBuf::from(home).join(relative))
                })
                .unwrap_or(mapped);
            let entry_id = format!("{}:{index}", adapter.name());
            let mapped = registry
                .catalog_settings
                .get(&super::nested::key("windows-host", &entry_id))
                .and_then(|s| s.path.as_ref())
                .map(std::path::PathBuf::from)
                .unwrap_or(mapped);
            let root = mapped.to_string_lossy().into_owned();
            let host = registry
                .environments
                .iter_mut()
                .find(|e| e.id == "windows-host")
                .unwrap();
            registry.catalog.push(super::catalog::CatalogEntry {
                id: entry_id.clone(),
                environment: "windows-host".into(),
                name: adapter.name().into(),
                home: base.to_string_lossy().into_owned(),
                root: root.clone(),
                detected: path_error.is_none() && base.is_dir(),
                enabled: true,
                home_enabled: enabled,
                provider: host
                    .providers
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.id.clone())
                    .or_else(|| (path_error.is_none() && mapped.is_dir()).then(|| id.clone())),
                error: path_error.clone(),
                ..Default::default()
            });
            if let Some(error) = &path_error {
                if let Some(provider) = host.providers.iter_mut().find(|p| p.id == id) {
                    provider.last_error = Some(error.clone());
                }
                continue;
            }
            if let Some(provider) = host.providers.iter_mut().find(|p| p.id == id) {
                provider.disabled = !enabled;
                if provider.root != root {
                    provider.paused = true;
                    provider.root = root.clone();
                    for skill in &mut registry.skills {
                        for member in skill
                            .members
                            .iter_mut()
                            .filter(|m| m.environment == "windows-host" && m.provider == id)
                        {
                            member.path = Path::new(&root)
                                .join(&skill.name)
                                .to_string_lossy()
                                .into_owned();
                            member.fingerprint = None;
                            member.previous = None;
                            member.scanned_at = None;
                            member.error = Some("Windows path changed; scan required".into());
                        }
                    }
                }
            } else if mapped.is_dir()
                && !registry
                    .catalog_settings
                    .contains_key(&super::nested::key("windows-host", &entry_id))
            {
                host.providers.push(super::registry::Provider {
                    id,
                    name: adapter.name().into(),
                    root,
                    scope: "global".into(),
                    paused: false,
                    disabled: !enabled,
                    last_error: None,
                    last_scanned: None,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_provider_path_survives_reconcile_until_native_setting_changes() {
        let t = tempfile::tempdir().unwrap();
        let linux = t.path().join("linux");
        let windows = t.path().join("windows");
        let native = windows.join(".codex/skills");
        let selected = t.path().join("selected");
        std::fs::create_dir_all(&native).unwrap();
        std::fs::create_dir(&selected).unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(
            crate::adapter::codex::CodexAdapter::with_home(linux.clone()),
        )];
        let c = super::super::registry::Coordinator::new(t.path().join("coordinator"));
        c.transaction(|r| reconcile(r, &windows, "host", &linux, &adapters, |_| None))
            .unwrap();
        c.update_provider_path(
            "windows-host",
            "windows:codex:1",
            native.to_str().unwrap(),
            selected.to_str().unwrap(),
        )
        .unwrap();
        c.transaction(|r| reconcile(r, &windows, "host", &linux, &adapters, |_| None))
            .unwrap();
        let v = c.overview().unwrap();
        assert_eq!(
            v.registry.environments[0].providers[0].root,
            selected.to_string_lossy()
        );
        assert!(
            v.registry
                .catalog
                .iter()
                .any(|entry| entry.provider.as_deref() == Some("windows:codex:1")
                    && entry.root == selected.to_string_lossy())
        );
    }
    #[test]
    fn windows_dynamic_roots_use_windows_installations() {
        let t = tempfile::tempdir().unwrap();
        let linux = t.path().join("linux");
        let windows = t.path().join("windows");
        std::fs::create_dir_all(linux.join(".gemini/antigravity-cli/skills")).unwrap();
        let penguin = windows.join(".penguin/data/project/agents/main/agent_state/skills");
        std::fs::create_dir_all(&penguin).unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![
            Box::new(crate::adapter::penguin::PenguinAdapter::with_home(
                linux.clone(),
            )),
            Box::new(crate::adapter::antigravity::AntigravityAdapter::with_home(
                linux.clone(),
            )),
        ];
        let mut r = Registry::default();
        reconcile(&mut r, &windows, "host", &linux, &adapters, |_| None).unwrap();
        assert!(
            r.environments[0]
                .providers
                .iter()
                .any(|p| p.root == penguin.to_string_lossy())
        );
        assert!(!r.catalog.iter().any(|c| c.root.contains("antigravity-cli")));
    }
    #[test]
    fn absent_optional_roots_are_cataloged_without_enrollment() {
        let t = tempfile::tempdir().unwrap();
        let linux = t.path().join("linux");
        let windows = t.path().join("windows");
        std::fs::create_dir_all(windows.join(".codex")).unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![
            Box::new(crate::adapter::codex::CodexAdapter::with_home(
                linux.clone(),
            )),
            Box::new(crate::adapter::claude::ClaudeAdapter::with_home(
                linux.clone(),
            )),
        ];
        let mut r = Registry::default();
        reconcile(&mut r, &windows, "host", &linux, &adapters, |_| None).unwrap();
        assert!(r.environments[0].providers.is_empty());
        assert!(
            r.catalog
                .iter()
                .any(|c| c.name == "claude" && !c.detected && c.error.is_none())
        );
        assert!(
            r.catalog
                .iter()
                .any(|c| c.name == "codex" && c.detected && c.provider.is_none())
        );
    }
    #[test]
    fn one_untranslatable_custom_path_does_not_hide_healthy_providers() {
        let t = tempfile::tempdir().unwrap();
        let linux = t.path().join("linux");
        let windows = t.path().join("windows");
        std::fs::create_dir_all(windows.join(".claude/skills")).unwrap();
        std::fs::create_dir_all(windows.join(".harnesskit")).unwrap();
        let store = crate::store::Store::open(&windows.join(".harnesskit/metadata.db")).unwrap();
        store
            .set_agent_path("codex", Some("Z:\\unreachable"))
            .unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![
            Box::new(crate::adapter::codex::CodexAdapter::with_home(
                linux.clone(),
            )),
            Box::new(crate::adapter::claude::ClaudeAdapter::with_home(
                linux.clone(),
            )),
        ];
        let mut r = Registry::default();
        reconcile(&mut r, &windows, "host", &linux, &adapters, |_| None).unwrap();
        assert!(
            r.environments[0]
                .providers
                .iter()
                .any(|p| p.name == "claude")
        );
        assert!(
            r.catalog
                .iter()
                .any(|c| c.name == "codex" && c.error.is_some())
        );
    }
    #[test]
    fn windows_profile_and_settings_are_separate_from_wsl() {
        let temp = tempfile::tempdir().unwrap();
        let linux = temp.path().join("linux");
        let windows = temp.path().join("windows");
        std::fs::create_dir_all(windows.join(".codex/skills")).unwrap();
        std::fs::create_dir_all(windows.join(".harnesskit")).unwrap();
        let store = crate::store::Store::open(&windows.join(".harnesskit/metadata.db")).unwrap();
        store
            .set_agent_path("codex", Some("D:\\Tools\\Codex"))
            .unwrap();
        store.set_agent_enabled("codex", false).unwrap();
        std::fs::create_dir_all(windows.join(".agents/skills")).unwrap();
        let custom = temp.path().join("custom");
        std::fs::create_dir_all(custom.join("skills")).unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(
            crate::adapter::codex::CodexAdapter::with_home(linux.clone()),
        )];
        let mut registry = Registry::default();
        reconcile(
            &mut registry,
            &windows,
            "DESKTOP",
            &linux,
            &adapters,
            |_| Some(custom.to_string_lossy().into_owned()),
        )
        .unwrap();
        assert_eq!(registry.environments.len(), 1);
        let host = &registry.environments[0];
        assert_eq!(host.id, "windows-host");
        assert_eq!(host.machine, "DESKTOP");
        assert_eq!(host.os, "windows");
        assert!(host.paused);
        assert!(
            host.providers
                .iter()
                .any(|p| p.root == windows.join(".agents/skills").to_string_lossy())
        );
        assert!(
            host.providers
                .iter()
                .any(|p| p.root == custom.join("skills").to_string_lossy() && p.disabled)
        );
        registry.environments[0].paused = false;
        reconcile(
            &mut registry,
            &windows,
            "DESKTOP",
            &linux,
            &adapters,
            |_| Some(custom.to_string_lossy().into_owned()),
        )
        .unwrap();
        assert_eq!(registry.environments.len(), 1);
        assert!(!registry.environments[0].paused);
        assert_eq!(
            rusqlite::Connection::open_with_flags(
                windows.join(".harnesskit/metadata.db"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            )
            .unwrap()
            .query_row(
                "SELECT custom_path, enabled FROM agent_settings WHERE name='codex'",
                [],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, bool>(1)?))
            )
            .unwrap(),
            (Some("D:\\Tools\\Codex".into()), false)
        );
    }
}
