//! Discovery metadata is separate from enrollment: absent optional roots are not failures.
use super::registry::Registry;
use crate::{HkError, adapter::AgentAdapter, store::Store};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn skill_kind() -> String {
    "skills".into()
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CatalogEntry {
    #[serde(default = "skill_kind")]
    pub kind: String,
    #[serde(default = "super::nested::yes")]
    pub is_directory: bool,
    #[serde(default)]
    pub tracked: bool,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub access: super::nested::Access,
    #[serde(default)]
    pub home_access: super::nested::Access,
    #[serde(default = "super::nested::yes")]
    pub home_enabled: bool,
    pub environment: String,
    pub name: String,
    pub home: String,
    pub root: String,
    pub detected: bool,
    pub enabled: bool,
    pub provider: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PathOverride {
    pub native_root: String,
    pub root: String,
}

pub fn provider_root(
    r: &mut Registry,
    environment: &str,
    provider: &str,
    native: PathBuf,
) -> PathBuf {
    let key = format!("{environment}/{provider}");
    if let Some(entry) = r.provider_path_overrides.get(&key) {
        if Path::new(&entry.native_root) == native {
            return PathBuf::from(&entry.root);
        }
        // A deliberate native settings change supersedes the old coordinator override.
        r.provider_path_overrides.remove(&key);
    }
    native
}

pub fn reconcile_local(
    r: &mut Registry,
    _home: &Path,
    adapters: &[Box<dyn AgentAdapter>],
    store: &Store,
) -> Result<(), HkError> {
    r.catalog
        .retain(|c| c.environment != "local" || c.project.is_some());
    for adapter in adapters {
        let (custom, enabled) = store.get_agent_setting(adapter.name())?;
        let base = custom
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| adapter.base_dir());
        let mut roots = adapter.skill_dirs();
        // Dynamic adapters may have no skill roots before their first installation.
        if roots.is_empty() {
            roots.push(adapter.base_dir().join("skills"));
        }
        for (index, default) in roots.into_iter().enumerate() {
            let root = match (&custom, default.strip_prefix(adapter.base_dir())) {
                (Some(_), Ok(relative)) => base.join(relative),
                _ => default,
            };
            let id = format!("{}:{index}", adapter.name());
            if r.forgotten
                .contains(&format!("agent:local/{}", adapter.name()))
                || r.forgotten.contains(&format!("path:local/{id}"))
            {
                continue;
            }
            let root = provider_root(r, "local", &id, root);
            let root = r
                .catalog_settings
                .get(&super::nested::key("local", &id))
                .and_then(|s| s.path.as_ref())
                .map(PathBuf::from)
                .unwrap_or(root);
            let provider = r
                .environments
                .iter()
                .find(|e| e.id == "local")
                .and_then(|e| {
                    e.providers.iter().find(|p| {
                        p.id == id || (p.name == adapter.name() && Path::new(&p.root) == root)
                    })
                });
            r.catalog.push(CatalogEntry {
                id: id.clone(),
                kind: skill_kind(),
                is_directory: true,
                environment: "local".into(),
                name: adapter.name().into(),
                home: base.to_string_lossy().into_owned(),
                root: root.to_string_lossy().into_owned(),
                detected: base.is_dir(),
                enabled: true,
                home_enabled: enabled,
                provider: provider.map(|p| p.id.clone()),
                error: provider.and_then(|p| p.last_error.clone()),
                ..Default::default()
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    fn probe(home: &Path, request: serde_json::Value) -> serde_json::Value {
        let mut child = Command::new("python3")
            .args(["-c", include_str!("remote.py")])
            .env("HOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.to_string().as_bytes())
            .unwrap();
        serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap()
    }
    #[test]
    fn remote_discovery_rejects_empty_skills_directory() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("one/.agents/skills")).unwrap();
        let request = serde_json::json!({"action":"discover_projects","root":t.path(),"specifications":[{"agent":"codex","components":[{"relative":".agents/skills","kind":"skills","is_directory":true}]}]});
        let v = probe(t.path(), request.clone());
        let v = v.get("result").unwrap_or(&v);
        assert!(v.as_array().unwrap().is_empty());
        std::fs::create_dir(t.path().join("one/.agents/skills/example")).unwrap();
        std::fs::write(
            t.path().join("one/.agents/skills/example/SKILL.md"),
            "skill",
        )
        .unwrap();
        let v = probe(t.path(), request);
        let v = v.get("result").unwrap_or(&v);
        assert_eq!(v.as_array().unwrap().len(), 1);
    }
    #[test]
    fn remote_browse_and_discovery_are_scoped_and_do_not_follow_symlinks() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("one/.codex")).unwrap();
        std::fs::write(t.path().join("one/.codex/config.toml"), "test").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(t.path().join("one"), t.path().join("alias")).unwrap();
        let v = probe(
            t.path(),
            serde_json::json!({"action":"discover_projects","root":t.path(),"specifications":[{"agent":"codex","components":[{"relative":".codex/config.toml","kind":"config","is_directory":false}]}]}),
        );
        let v = v.get("result").unwrap_or(&v);
        assert_eq!(v.as_array().unwrap().len(), 1);
        let b = probe(
            t.path(),
            serde_json::json!({"action":"browse_paths","path":t.path().join("one/.codex")}),
        );
        let b = b.get("result").unwrap_or(&b);
        assert_eq!(b["entries"][0]["name"], "config.toml");
        let access = probe(
            t.path(),
            serde_json::json!({"action":"path_access","paths":[{"path":t.path().join("one/.codex/config.toml"),"is_directory":false}]}),
        );
        let access = access.get("result").unwrap_or(&access);
        assert_eq!(access[0]["state"], "available");
    }
    #[test]
    fn remote_catalog_contains_every_supported_agent_without_enrolling_missing_roots() {
        let t = tempfile::tempdir().unwrap();
        let result = probe(t.path(), serde_json::json!({"action":"probe"}));
        let result = result.get("result").unwrap_or(&result);
        let catalog = result["catalog"].as_array().unwrap();
        for adapter in crate::adapter::all_adapters() {
            assert!(
                catalog.iter().any(|c| c["name"] == adapter.name()),
                "{} absent",
                adapter.name()
            );
        }
        assert!(result["providers"].as_array().unwrap().is_empty());
        assert!(catalog.iter().all(|c| c["error"].is_null()));
    }
    #[cfg(unix)]
    #[test]
    fn container_mount_guard_follows_symlinks_and_keeps_other_providers() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let mounted = t.path().join("mounted");
        std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
        std::fs::create_dir_all(mounted.join("skills")).unwrap();
        std::os::unix::fs::symlink(&mounted, home.join(".codex")).unwrap();
        let result = probe(
            &home,
            serde_json::json!({"action":"probe","_blocked_mounts":[mounted]}),
        );
        let result = result.get("result").unwrap_or(&result);
        assert!(
            result["catalog"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["name"] == "codex" && c["error"].is_string())
        );
        assert!(
            result["providers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p[0] == "claude")
        );
        let result = probe(
            &home,
            serde_json::json!({"action":"list","root":home.join(".codex/skills"),"_blocked_mounts":[t.path().join("mounted")]}),
        );
        assert!(
            result["error"]
                .as_str()
                .unwrap()
                .contains("shared container mount")
        );
    }
}
