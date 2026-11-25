//! Coordinator-owned nested path preferences and bounded, metadata-only access checks.
use super::{catalog::CatalogEntry, registry::Registry, transport::Connection};
use crate::{HkError, adapter::AgentAdapter, store::Store};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
pub fn yes() -> bool {
    true
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Access {
    pub state: String,
    pub count: Option<usize>,
    pub error: Option<String>,
}
impl Default for Access {
    fn default() -> Self {
        Self {
            state: "unchecked".into(),
            count: None,
            error: None,
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct EntrySetting {
    pub path: Option<String>,
    pub enabled: Option<bool>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub components: Vec<super::discovery::Component>,
    pub id: String,
    pub environment: String,
    pub agent: String,
    pub root: String,
    #[serde(default)]
    pub access: Access,
    #[serde(default = "yes")]
    pub enabled: bool,
}
pub fn key(env: &str, id: &str) -> String {
    format!("{env}/{id}")
}
pub fn access(path: &str, count: bool) -> Access {
    let result = (|| -> std::io::Result<usize> {
        let entries = std::fs::read_dir(path)?;
        let mut n = 0;
        for entry in entries.take(10001) {
            let entry = entry?;
            if !count
                || entry.file_type()?.is_dir()
                    && (entry.path().join("SKILL.md").is_file()
                        || entry.path().join("SKILL.md.disabled").is_file())
            {
                n += 1;
            }
        }
        Ok(n)
    })();
    match result {
        Ok(n) => Access {
            state: if count && n == 0 {
                "empty"
            } else {
                "available"
            }
            .into(),
            count: count.then_some(n),
            error: None,
        },
        Err(e) => Access {
            state: match e.kind() {
                std::io::ErrorKind::NotFound => "missing",
                std::io::ErrorKind::PermissionDenied => "denied",
                _ => "error",
            }
            .into(),
            count: None,
            error: Some(e.to_string()),
        },
    }
}
pub fn component_access(path: &str, kind: &str, directory: bool) -> Access {
    if directory {
        return access(path, kind == "skills");
    }
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => Access {
            state: "available".into(),
            count: None,
            error: None,
        },
        Ok(_) => Access {
            state: "error".into(),
            count: None,
            error: Some("Expected a file".into()),
        },
        Err(e) => Access {
            state: match e.kind() {
                std::io::ErrorKind::NotFound => "missing",
                std::io::ErrorKind::PermissionDenied => "denied",
                _ => "error",
            }
            .into(),
            count: None,
            error: Some(e.to_string()),
        },
    }
}
pub fn validate(r: &Registry, env: &str, path: &str) -> Result<String, HkError> {
    let e = r
        .environments
        .iter()
        .find(|e| e.id == env)
        .ok_or_else(|| HkError::NotFound("Environment".into()))?;
    let expanded = if path == "~" || path.starts_with("~/") {
        if matches!(e.connection, Connection::Local) {
            let home = r.path_homes.get(env).ok_or_else(|| {
                HkError::Validation(
                    "Target environment home is unknown; choose an absolute path".into(),
                )
            })?;
            Path::new(home)
                .join(path.strip_prefix("~/").unwrap_or(""))
                .to_string_lossy()
                .into_owned()
        } else {
            let value = super::transport::run(
                &e.connection,
                serde_json::json!({"action":"expand_path","path":path}),
            )?;
            value
                .as_str()
                .ok_or_else(|| HkError::Validation("Invalid remote path expansion".into()))?
                .to_owned()
        }
    } else {
        path.to_owned()
    };
    let path = super::paths::for_connection(&expanded, &e.connection);
    if path.trim().is_empty()
        || (!path.starts_with('/') && !(path.len() > 2 && path.as_bytes()[1] == b':'))
    {
        return Err(HkError::Validation("Use an absolute directory path".into()));
    }
    Ok(path)
}
pub fn reconcile(
    r: &mut Registry,
    adapters: &[Box<dyn AgentAdapter>],
    store: &Store,
) -> Result<(), HkError> {
    for e in &mut r.environments {
        e.providers.retain(|p| {
            !r.forgotten.contains(&format!("agent:{}/{}", e.id, p.name))
                && !r
                    .forgotten
                    .contains(&format!("location:{}/{}/{}", e.id, p.name, p.root))
        });
    }
    // Preserve registered projects and associate them only with agents whose markers exist.
    for p in store.list_projects()? {
        for a in adapters {
            if !r.forgotten.contains(&format!("agent:local/{}", a.name()))
                && !r
                    .forgotten
                    .contains(&format!("project:local/{}/{}", a.name(), p.path))
                && a.project_markers().iter().any(|m| match m {
                    crate::adapter::ProjectMarker::Dir(x) => Path::new(&p.path).join(x).is_dir(),
                    crate::adapter::ProjectMarker::File(x) => Path::new(&p.path).join(x).is_file(),
                })
                && !r
                    .projects
                    .iter()
                    .any(|x| x.environment == "local" && x.agent == a.name() && x.root == p.path)
            {
                r.projects.push(Project {
                    id: format!("legacy:{}:{}", p.id, a.name()),
                    components: vec![],
                    environment: "local".into(),
                    agent: a.name().into(),
                    root: p.path.clone(),
                    access: Access::default(),
                    enabled: true,
                });
            }
        }
    }
    let previous: BTreeMap<_, _> = r
        .catalog
        .iter()
        .map(|c| (key(&c.environment, &c.id), c.clone()))
        .collect();
    r.catalog.retain(|c| c.project.is_none());
    let mut indices = BTreeMap::<(String, String), usize>::new();
    for c in &mut r.catalog {
        let index = indices
            .entry((c.environment.clone(), c.name.clone()))
            .or_default();
        if c.id.is_empty() {
            c.id = format!("{}:{}", c.name, index);
        }
        *index += 1;
        if c.scope.is_empty() {
            c.scope = "global".into();
        }
    }
    for p in &r.projects {
        if let Some(a) = adapters.iter().find(|a| a.name() == p.agent) {
            let components = if p.components.is_empty() {
                a.project_skill_dirs()
                    .into_iter()
                    .filter(|p| !p.contains('*'))
                    .map(|relative| super::discovery::Component {
                        relative,
                        kind: "skills".into(),
                        is_directory: true,
                    })
                    .collect()
            } else {
                p.components.clone()
            };
            for (i, component) in components.iter().enumerate() {
                let rel = &component.relative;
                r.catalog.push(CatalogEntry {
                    id: format!("project:{}:{i}", p.id),
                    kind: component.kind.clone(),
                    is_directory: component.is_directory,
                    environment: p.environment.clone(),
                    name: p.agent.clone(),
                    home: p.root.clone(),
                    root: format!("{}/{}", p.root.trim_end_matches(['/', '\\']), rel),
                    scope: format!("project:{}", p.id),
                    project: Some(p.id.clone()),
                    enabled: true,
                    home_enabled: true,
                    ..Default::default()
                });
            }
        }
    }
    for c in r.catalog.iter_mut().filter(|c| c.project.is_some()) {
        if let Some(old) = previous.get(&key(&c.environment, &c.id)) {
            c.provider = old.provider.clone();
            c.access = old.access.clone();
            c.home_access = old.home_access.clone();
        }
    }
    for e in &r.environments {
        for p in &e.providers {
            if !r.catalog.iter().any(|c| {
                c.environment == e.id && (c.provider.as_deref() == Some(&p.id) || c.root == p.root)
            }) {
                r.catalog.push(CatalogEntry {
                    id: format!("mapped:{}", p.id),
                    environment: e.id.clone(),
                    name: p.name.clone(),
                    home: Path::new(&p.root)
                        .parent()
                        .unwrap_or(Path::new(&p.root))
                        .to_string_lossy()
                        .into_owned(),
                    root: p.root.clone(),
                    scope: p.scope.clone(),
                    provider: Some(p.id.clone()),
                    enabled: !p.disabled,
                    home_enabled: true,
                    ..Default::default()
                });
            }
        }
    }
    r.catalog.retain(|c| {
        !r.forgotten
            .contains(&format!("agent:{}/{}", c.environment, c.name))
            && !r
                .forgotten
                .contains(&format!("path:{}/{}", c.environment, c.id))
    });
    for c in &mut r.catalog {
        if c.kind.is_empty() {
            c.kind = "skills".into();
            c.is_directory = true;
        }
        let h = r
            .catalog_settings
            .get(&key(&c.environment, &format!("home:{}", c.name)));
        c.home_enabled = if c.environment == "local" {
            store.get_agent_setting(&c.name)?.1
        } else {
            h.and_then(|s| s.enabled).unwrap_or(c.home_enabled)
        };
        if c.project.is_none() && c.environment != "local" {
            if let Some(path) = h.and_then(|s| s.path.as_ref()) {
                if let Some(rel) = c
                    .root
                    .strip_prefix(&format!("{}/", c.home.trim_end_matches('/')))
                {
                    c.root = format!("{}/{}", path.trim_end_matches('/'), rel);
                }
                c.home = path.clone();
            }
        }
        if let Some(s) = r.catalog_settings.get(&key(&c.environment, &c.id)) {
            if let Some(path) = &s.path {
                c.root = path.clone();
            }
            if let Some(enabled) = s.enabled {
                c.enabled = enabled;
            }
        }
        let e = r
            .environments
            .iter_mut()
            .find(|e| e.id == c.environment)
            .unwrap();
        if c.provider.is_none() {
            c.provider = e
                .providers
                .iter()
                .find(|p| p.root == c.root && p.name == c.name && p.scope == c.scope)
                .map(|p| p.id.clone());
        }
        c.tracked = r
            .catalog_settings
            .get(&key(&c.environment, &c.id))
            .and_then(|s| s.enabled)
            .unwrap_or(c.provider.is_some() || c.project.is_some())
            && c.enabled
            && c.project
                .as_ref()
                .is_none_or(|id| r.projects.iter().all(|p| &p.id != id || p.enabled));
        if let Some(p) = e
            .providers
            .iter_mut()
            .find(|p| Some(&p.id) == c.provider.as_ref())
        {
            p.disabled = !c.enabled
                || !c.home_enabled
                || c.project
                    .as_ref()
                    .is_some_and(|id| r.projects.iter().any(|p| &p.id == id && !p.enabled));
            if p.root != c.root {
                p.paused = true;
                p.root = c.root.clone();
                p.last_scanned = None;
                p.last_error = Some("Path changed; scan required".into());
                for s in &mut r.skills {
                    for m in &mut s.members {
                        if m.environment == e.id && m.provider == p.id {
                            m.path = format!("{}/{}", p.root.trim_end_matches('/'), s.name);
                            m.previous = None;
                            m.fingerprint = None;
                            m.scanned_at = None;
                            m.error = Some("Path changed; scan required".into());
                        }
                    }
                }
            }
        }
        if matches!(e.connection, Connection::Local) {
            c.access = component_access(&c.root, &c.kind, c.is_directory);
            c.home_access = access(&c.home, false);
            c.detected = c.home_access.error.is_none();
            c.error = c.access.error.clone();
        }
    }
    for p in &mut r.projects {
        if r.environments
            .iter()
            .any(|e| e.id == p.environment && matches!(e.connection, Connection::Local))
        {
            p.access = access(&p.root, false);
        }
    }
    access_errors(r);
    Ok(())
}
fn access_errors(r: &mut Registry) {
    for c in &r.catalog {
        if !c.tracked {
            continue;
        }
        if let Some(error) = &c.access.error {
            if let Some(p) = r
                .environments
                .iter_mut()
                .find(|e| e.id == c.environment)
                .and_then(|e| {
                    e.providers
                        .iter_mut()
                        .find(|p| Some(&p.id) == c.provider.as_ref())
                })
            {
                p.last_error = Some(error.clone());
            }
        }
    }
}
pub fn check(r: &mut Registry, environment: &str) -> Result<(), HkError> {
    let e = r
        .environments
        .iter()
        .find(|e| e.id == environment)
        .ok_or_else(|| HkError::NotFound("Environment".into()))?;
    if matches!(e.connection, Connection::Local) {
        for p in r
            .projects
            .iter_mut()
            .filter(|p| p.environment == environment)
        {
            p.access = access(&p.root, false);
        }
        for c in r
            .catalog
            .iter_mut()
            .filter(|c| c.environment == environment)
        {
            c.access = component_access(&c.root, &c.kind, c.is_directory);
            c.home_access = access(&c.home, false);
            c.error = c.access.error.clone();
        }
        access_errors(r);
        return Ok(());
    }
    let mut paths: Vec<_> = r
        .catalog
        .iter()
        .filter(|c| c.environment == environment)
        .flat_map(|c| {
            [
                serde_json::json!({"path":c.root,"count":c.kind=="skills","is_directory":c.is_directory}),
                serde_json::json!({"path":c.home,"count":false}),
            ]
        })
        .collect();
    let catalog_path_count = paths.len();
    paths.extend(
        r.projects
            .iter()
            .filter(|p| p.environment == environment)
            .map(|p| serde_json::json!({"path":p.root,"count":false})),
    );
    let response = super::transport::run(
        &e.connection,
        serde_json::json!({"action":"path_access","paths":paths}),
    );
    match response {
        Ok(value) => {
            let values: Vec<Access> = serde_json::from_value(value)?;
            for (p, value) in r
                .projects
                .iter_mut()
                .filter(|p| p.environment == environment)
                .zip(values.iter().skip(catalog_path_count))
            {
                p.access = value.clone();
            }
            for (c, pair) in r
                .catalog
                .iter_mut()
                .filter(|c| c.environment == environment)
                .zip(values.chunks_exact(2))
            {
                c.access = pair[0].clone();
                c.home_access = pair[1].clone();
                c.error = c.access.error.clone();
            }
        }
        Err(error) => {
            for p in r
                .projects
                .iter_mut()
                .filter(|p| p.environment == environment)
            {
                p.access = Access {
                    state: "error".into(),
                    count: None,
                    error: Some(error.to_string()),
                };
            }
            for c in r
                .catalog
                .iter_mut()
                .filter(|c| c.environment == environment)
            {
                c.access = Access {
                    state: "error".into(),
                    count: None,
                    error: Some(error.to_string()),
                };
                c.home_access = c.access.clone();
                c.error = c.access.error.clone();
            }
        }
    }
    for p in r
        .projects
        .iter_mut()
        .filter(|p| p.environment == environment)
    {
        if let Some(c) = r
            .catalog
            .iter()
            .find(|c| c.project.as_deref() == Some(&p.id))
        {
            p.access = c.home_access.clone();
        }
    }
    access_errors(r);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::service::{Request, execute};
    use super::*;
    fn setup() -> (tempfile::TempDir, Store, Vec<Box<dyn AgentAdapter>>) {
        let t = tempfile::tempdir().unwrap();
        let store = Store::open(&t.path().join("test.db")).unwrap();
        let a: Vec<Box<dyn AgentAdapter>> = vec![Box::new(
            crate::adapter::codex::CodexAdapter::with_home(t.path().into()),
        )];
        (t, store, a)
    }
    fn command(
        home: &Path,
        s: &Store,
        a: &[Box<dyn AgentAdapter>],
        value: serde_json::Value,
    ) -> Result<super::super::registry::Overview, HkError> {
        execute(home, a, s, serde_json::from_value(value)?)
    }
    #[test]
    fn explicit_project_readdition_clears_instruction_and_config_tombstones() {
        let (t, s, a) = setup();
        let home = t.path();
        let root = home.join("project");
        std::fs::create_dir_all(root.join(".codex")).unwrap();
        std::fs::write(root.join("AGENTS.md"), "instructions").unwrap();
        std::fs::write(root.join(".codex/config.toml"), "x=1").unwrap();
        let add = || {
            command(
                home,
                &s,
                &a,
                serde_json::json!({"action":"add_agent_project","environment":"local","agent":"codex","root":root}),
            )
        };
        let v = add().unwrap();
        let project = &v.registry.projects[0];
        command(home,&s,&a,serde_json::json!({"action":"remove_project","environment":"local","project":project.id})).unwrap();
        let v = add().unwrap();
        for rel in ["AGENTS.md", ".codex/config.toml"] {
            let path = root.join(rel).to_string_lossy().into_owned();
            assert!(
                !v.registry
                    .forgotten
                    .contains(&format!("location:local/codex/{path}")),
                "Explicit readdition clears component tombstones"
            );
            assert!(
                v.registry
                    .catalog
                    .iter()
                    .any(|c| c.project.is_some() && c.root == path)
            );
        }
    }
    #[test]
    fn project_discovery_requires_files_and_retains_existing_empty_components() {
        let (t, s, a) = setup();
        let home = t.path();
        let root = home.join("project");
        let skills = root.join(".agents/skills");
        std::fs::create_dir_all(&skills).unwrap();
        let add = || {
            command(
                home,
                &s,
                &a,
                serde_json::json!({"action":"add_agent_project","environment":"local","agent":"codex","root":root}),
            )
        };
        assert!(
            add().is_err(),
            "Empty skills directory is not a valid project"
        );
        std::fs::create_dir(root.join(".codex")).unwrap();
        std::fs::write(root.join(".codex/config.toml"), "x = 1").unwrap();
        let v = add().unwrap();
        assert!(
            v.registry
                .catalog
                .iter()
                .filter(|c| c.project.is_some())
                .all(|c| c.kind != "skills")
        );
        let other = home.join("with-skill");
        std::fs::create_dir_all(other.join(".agents/skills/example")).unwrap();
        let file = other.join(".agents/skills/example/SKILL.md.disabled");
        std::fs::write(&file, "skill").unwrap();
        let v=command(home,&s,&a,serde_json::json!({"action":"add_agent_project","environment":"local","agent":"codex","root":other})).unwrap();
        let c = v
            .registry
            .catalog
            .iter()
            .find(|c| c.project.is_some() && c.kind == "skills")
            .unwrap();
        let id = c.id.clone();
        std::fs::remove_file(file).unwrap();
        let v = execute(home, &a, &s, Request::Overview).unwrap();
        assert!(
            v.registry
                .catalog
                .iter()
                .any(|c| c.id == id && c.tracked && c.access.state == "empty")
        );
    }
    #[test]
    fn home_expansion_uses_target_environment_home() {
        let mut r = Registry::default();
        r.environments.push(super::super::registry::Environment {
            id: "mapped".into(),
            machine: "peer".into(),
            name: "Windows".into(),
            os: "windows".into(),
            connection: Connection::Local,
            paused: true,
            providers: vec![],
        });
        assert!(validate(&r, "mapped", "~/code").is_err());
        r.path_homes
            .insert("mapped".into(), "/mnt/c/Users/person".into());
        assert_eq!(
            validate(&r, "mapped", "~/code").unwrap(),
            "/mnt/c/Users/person/code"
        );
    }
    #[test]
    fn forgotten_legacy_project_and_path_stay_removed_without_deleting_files() {
        let (t, s, a) = setup();
        let home = t.path();
        let root = home.join("legacy");
        std::fs::create_dir_all(root.join(".codex")).unwrap();
        s.insert_project(&crate::models::Project {
            id: "old".into(),
            name: "Legacy".into(),
            path: root.to_string_lossy().into(),
            created_at: chrono::Utc::now(),
            exists: true,
        })
        .unwrap();
        std::fs::create_dir_all(home.join(".codex/skills/safe")).unwrap();
        std::fs::write(home.join(".codex/skills/safe/SKILL.md"), "safe").unwrap();
        let v = execute(home, &a, &s, Request::Refresh).unwrap();
        let project = v.registry.projects[0].id.clone();
        let c = v
            .registry
            .catalog
            .iter()
            .find(|c| c.project.is_none() && c.provider.is_some())
            .unwrap();
        command(
            home,
            &s,
            &a,
            serde_json::json!({"action":"remove_path","environment":"local","entry":c.id}),
        )
        .unwrap();
        command(
            home,
            &s,
            &a,
            serde_json::json!({"action":"remove_project","environment":"local","project":project}),
        )
        .unwrap();
        let v = execute(home, &a, &s, Request::Refresh).unwrap();
        assert!(v.registry.projects.is_empty());
        assert!(!v.registry.catalog.iter().any(|x| x.id == c.id));
        assert!(v.registry.environments[0].providers.is_empty());
        assert_eq!(
            std::fs::read_to_string(home.join(".codex/skills/safe/SKILL.md")).unwrap(),
            "safe"
        );
    }
    #[test]
    fn environment_discovery_categorizes_agents_and_rejects_empty_folders() {
        let (t, s, mut a) = setup();
        let home = t.path();
        a.push(Box::new(crate::adapter::claude::ClaudeAdapter::with_home(
            home.into(),
        )));
        let root = home.join("code");
        std::fs::create_dir_all(root.join("one/.claude")).unwrap();
        std::fs::write(root.join("one/CLAUDE.md"), "test").unwrap();
        std::fs::write(root.join("one/AGENTS.md"), "test").unwrap();
        let v = command(
            home,
            &s,
            &a,
            serde_json::json!({"action":"discover_projects","environment":"local","root":root}),
        )
        .unwrap();
        assert!(v.registry.projects.iter().any(|p| p.agent == "codex"));
        assert!(v.registry.projects.iter().any(|p| p.agent == "claude"));
        let empty = home.join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert!(
            command(
                home,
                &s,
                &a,
                serde_json::json!({"action":"discover_projects","environment":"local","root":empty})
            )
            .is_err()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, home.join("alias")).unwrap();
            assert!(command(home,&s,&a,serde_json::json!({"action":"discover_projects","environment":"local","root":home.join("alias")})).is_err());
        }
    }
    #[test]
    fn quiet_tree_recursive_discovery_browse_remove_and_tracking() {
        let (t, s, a) = setup();
        let home = t.path();
        let root = home.join("code");
        std::fs::create_dir_all(root.join("one/.codex")).unwrap();
        std::fs::write(root.join("one/.codex/config.toml"), "x=1").unwrap();
        std::fs::write(root.join("one/AGENTS.md"), "hello").unwrap();
        std::fs::create_dir_all(root.join("node_modules/ignored/.codex")).unwrap();
        std::fs::write(root.join("node_modules/ignored/AGENTS.md"), "ignore").unwrap();
        let v=command(home,&s,&a,serde_json::json!({"action":"add_agent_project","environment":"local","agent":"codex","root":root})).unwrap();
        assert_eq!(v.registry.projects.len(), 1);
        assert!(
            v.registry
                .catalog
                .iter()
                .any(|c| c.project.is_some() && c.root.ends_with("config.toml"))
        );
        command(home,&s,&a,serde_json::json!({"action":"add_agent_project","environment":"local","agent":"codex","root":root})).unwrap();
        let b=command(home,&s,&a,serde_json::json!({"action":"browse_paths","environment":"local","path":root.join("one")})).unwrap();
        let b = serde_json::to_value(b).unwrap();
        assert!(
            b["browse"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["name"] == "AGENTS.md" && e["is_directory"] == false)
        );
        let id = v.registry.projects[0].id.clone();
        command(
            home,
            &s,
            &a,
            serde_json::json!({"action":"remove_project","environment":"local","project":id}),
        )
        .unwrap();
        let v = execute(home, &a, &s, Request::Refresh).unwrap();
        assert!(v.registry.projects.is_empty());
        assert!(root.join("one/AGENTS.md").exists());
        assert!(command(home,&s,&a,serde_json::json!({"action":"add_agent_project","environment":"local","agent":"codex","root":home.join("empty")})).is_err());
        let c = v
            .registry
            .catalog
            .iter()
            .find(|c| c.project.is_none())
            .unwrap();
        let v=command(home,&s,&a,serde_json::json!({"action":"catalog_enabled","environment":"local","entry":c.id,"enabled":true})).unwrap();
        let row = serde_json::to_value(v.registry.catalog.iter().find(|x| x.id == c.id).unwrap())
            .unwrap();
        assert_eq!(row["tracked"], true);
        command(
            home,
            &s,
            &a,
            serde_json::json!({"action":"remove_agent","environment":"local","agent":"codex"}),
        )
        .unwrap();
        assert!(
            execute(home, &a, &s, Request::Refresh)
                .unwrap()
                .registry
                .catalog
                .is_empty()
        );
    }
    #[test]
    fn missing_edits_persist_without_enrollment_and_pause_preserves_diagnostics() {
        let (t, s, a) = setup();
        let home = t.path();
        let v = execute(home, &a, &s, Request::Overview).unwrap();
        let c = &v.registry.catalog[0];
        let id = c.id.clone();
        let next = home.join("missing").to_string_lossy().into_owned();
        execute(
            home,
            &a,
            &s,
            Request::CatalogPath {
                environment: "local".into(),
                entry: id.clone(),
                expected: c.root.clone(),
                path: next.clone(),
            },
        )
        .unwrap();
        execute(
            home,
            &a,
            &s,
            Request::CatalogEnabled {
                environment: "local".into(),
                entry: id.clone(),
                enabled: false,
            },
        )
        .unwrap();
        let v = execute(
            home,
            &a,
            &s,
            Request::Pause {
                environment: "local".into(),
                provider: None,
                paused: true,
            },
        )
        .unwrap();
        let c = v.registry.catalog.iter().find(|c| c.id == id).unwrap();
        assert_eq!(c.root, next);
        assert!(!c.enabled);
        assert_eq!(c.access.state, "missing");
        assert!(c.provider.is_none());
        std::fs::create_dir(&next).unwrap();
        let v = execute(home, &a, &s, Request::Overview).unwrap();
        let c = v.registry.catalog.iter().find(|c| c.id == id).unwrap();
        assert_eq!(c.access.state, "empty");
        assert!(c.provider.is_none());
        assert!(
            execute(
                home,
                &a,
                &s,
                Request::CatalogPath {
                    environment: "local".into(),
                    entry: id,
                    expected: "stale".into(),
                    path: next
                }
            )
            .is_err()
        );
    }
    #[test]
    fn projects_keep_custom_child_paths_and_scopes_independent() {
        let (t, s, a) = setup();
        let home = t.path();
        std::fs::create_dir_all(home.join("one/.codex")).unwrap();
        std::fs::write(home.join("one/.codex/config.toml"), "x=1").unwrap();
        let v = execute(
            home,
            &a,
            &s,
            Request::AddAgentProject {
                environment: "local".into(),
                agent: "codex".into(),
                root: home.join("one").to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let project = v.registry.projects[0].clone();
        let c = v
            .registry
            .catalog
            .iter()
            .find(|c| c.project.as_deref() == Some(&project.id))
            .unwrap();
        let id = c.id.clone();
        let custom = home.join("custom").to_string_lossy().into_owned();
        execute(
            home,
            &a,
            &s,
            Request::CatalogPath {
                environment: "local".into(),
                entry: id.clone(),
                expected: c.root.clone(),
                path: custom.clone(),
            },
        )
        .unwrap();
        let v = execute(
            home,
            &a,
            &s,
            Request::ProjectPath {
                environment: "local".into(),
                project: project.id,
                expected: project.root,
                path: home.join("two").to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        assert_eq!(
            v.registry.catalog.iter().find(|c| c.id == id).unwrap().root,
            custom
        );
        assert!(
            v.registry
                .catalog
                .iter()
                .filter(|c| c.project.is_none())
                .all(|c| c.scope == "global")
        );
        assert!(v.registry.environments[0].providers.is_empty());
    }
    #[test]
    fn tracked_global_edit_keeps_scan_baselines_on_overview() {
        let (t, s, a) = setup();
        let home = t.path();
        std::fs::create_dir_all(home.join(".codex/skills/first")).unwrap();
        std::fs::write(home.join(".codex/skills/first/SKILL.md"), "old").unwrap();
        let next = home.join("next");
        std::fs::create_dir_all(next.join("second")).unwrap();
        std::fs::write(next.join("second/SKILL.md"), "new").unwrap();
        let v = execute(home, &a, &s, Request::Refresh).unwrap();
        let c = v
            .registry
            .catalog
            .iter()
            .find(|c| c.provider.is_some())
            .unwrap();
        execute(
            home,
            &a,
            &s,
            Request::CatalogPath {
                environment: "local".into(),
                entry: c.id.clone(),
                expected: c.root.clone(),
                path: next.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let scanned = execute(home, &a, &s, Request::Refresh).unwrap();
        let v = execute(home, &a, &s, Request::Overview).unwrap();
        assert_eq!(
            serde_json::to_value(&scanned.registry.skills).unwrap(),
            serde_json::to_value(&v.registry.skills).unwrap()
        );
        assert_eq!(
            v.registry.environments[0].providers[0].root,
            next.to_string_lossy()
        );
    }
    #[test]
    fn tracked_project_root_edit_preserves_provider_identity() {
        let (t, s, a) = setup();
        let home = t.path();
        let one = home.join("one");
        let two = home.join("two");
        std::fs::create_dir_all(one.join(".agents/skills/example")).unwrap();
        std::fs::write(one.join(".agents/skills/example/SKILL.md"), "skill").unwrap();
        std::fs::create_dir_all(two.join(".agents/skills")).unwrap();
        let v = execute(
            home,
            &a,
            &s,
            Request::AddAgentProject {
                environment: "local".into(),
                agent: "codex".into(),
                root: one.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let project = v.registry.projects[0].clone();
        let c = v
            .registry
            .catalog
            .iter()
            .find(|c| c.project.as_deref() == Some(&project.id))
            .unwrap();
        let id = c.id.clone();
        // Adapter path, rather than a guessed root, is the tracking contract.
        std::fs::create_dir_all(&c.root).unwrap();
        assert!(
            c.provider.is_some(),
            "Discovery enrolls skill roots without writing"
        );
        assert!(
            v.registry.environments[0]
                .providers
                .iter()
                .all(|p| p.paused)
        );
        let provider = v
            .registry
            .catalog
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .provider
            .clone()
            .unwrap();
        execute(
            home,
            &a,
            &s,
            Request::ProjectPath {
                environment: "local".into(),
                project: project.id,
                expected: project.root,
                path: two.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let v = execute(home, &a, &s, Request::Overview).unwrap();
        let c = v.registry.catalog.iter().find(|c| c.id == id).unwrap();
        assert_eq!(c.provider.as_ref(), Some(&provider));
        assert!(c.root.starts_with(two.to_str().unwrap()));
        assert_eq!(
            v.registry.environments[0]
                .providers
                .iter()
                .find(|p| p.id == provider)
                .unwrap()
                .root,
            c.root
        );
        assert_eq!(
            v.registry
                .catalog
                .iter()
                .filter(|c| c.provider.as_ref() == Some(&provider))
                .count(),
            1
        );
    }
    #[test]
    fn relocated_provider_waits_for_resume_before_automatic_writes() {
        let (t, s, a) = setup();
        let home = t.path();
        let peer = home.join("peer");
        let next = home.join("next");
        for root in [home.join(".codex/skills"), peer.clone(), next.clone()] {
            std::fs::create_dir_all(root.join("skill")).unwrap();
            std::fs::write(root.join("skill/SKILL.md"), "original").unwrap();
        }
        let v = execute(
            home,
            &a,
            &s,
            Request::Add {
                machine: "peer".into(),
                name: "peer".into(),
                os: "linux".into(),
                provider: "codex".into(),
                root: peer.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let env = v
            .registry
            .environments
            .iter()
            .find(|e| e.name == "peer")
            .unwrap()
            .id
            .clone();
        execute(
            home,
            &a,
            &s,
            Request::Pause {
                environment: env,
                provider: None,
                paused: false,
            },
        )
        .unwrap();
        execute(home, &a, &s, Request::Refresh).unwrap();
        let coordinator =
            super::super::registry::Coordinator::new(home.join(".harnesskit/environments"));
        coordinator
            .transaction(|r| {
                for skill in &mut r.skills {
                    skill.identity_confirmed = true;
                }
                Ok(())
            })
            .unwrap();
        std::fs::write(next.join("skill/SKILL.md"), "replacement").unwrap();
        let v = execute(home, &a, &s, Request::Overview).unwrap();
        let c = v
            .registry
            .catalog
            .iter()
            .find(|c| c.environment == "local" && c.provider.is_some())
            .unwrap();
        execute(
            home,
            &a,
            &s,
            Request::CatalogPath {
                environment: "local".into(),
                entry: c.id.clone(),
                expected: c.root.clone(),
                path: next.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let v = execute(home, &a, &s, Request::Tick).unwrap();
        assert!(
            v.registry
                .environments
                .iter()
                .find(|e| e.id == "local")
                .unwrap()
                .providers
                .iter()
                .find(|p| p.root == next.to_string_lossy())
                .unwrap()
                .paused
        );
        assert_eq!(
            std::fs::read_to_string(peer.join("skill/SKILL.md")).unwrap(),
            "original"
        );
    }
    #[test]
    fn access_counts_skills_and_distinguishes_non_directory() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("not-skill")).unwrap();
        assert_eq!(access(t.path().to_str().unwrap(), true).count, Some(0));
        std::fs::create_dir(t.path().join("skill")).unwrap();
        std::fs::write(t.path().join("skill/SKILL.md"), "test").unwrap();
        assert_eq!(access(t.path().to_str().unwrap(), true).count, Some(1));
        assert_eq!(
            access(t.path().join("skill/SKILL.md").to_str().unwrap(), true).state,
            "error"
        );
    }
}
