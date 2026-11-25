//! Persistent text-file versions. Observation never accepts or propagates a change.
use super::{
    TextSnapshot,
    registry::{Coordinator, Registry},
    transport::{self, Connection},
};
use crate::{HkError, adapter::AgentAdapter, store::Store};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Revision {
    pub number: usize,
    pub content: String,
    pub fingerprint: String,
    pub created_at: String,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Copy {
    pub id: String,
    pub environment_id: String,
    pub environment: String,
    pub machine: String,
    pub path: String,
    pub provider_id: String,
    pub content: Option<String>,
    pub fingerprint: Option<String>,
    pub accepted_fingerprint: Option<String>,
    pub error: Option<String>,
    pub enabled: bool,
    pub pending: bool,
    /// Root captured from the approved inventory, never supplied by command callers.
    pub root: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ManagedFile {
    pub id: String,
    pub agent: String,
    pub category: String,
    pub scope: String,
    pub scope_path: Option<String>,
    pub file_name: String,
    pub logical_path: String,
    pub variant_name: Option<String>,
    pub original_id: Option<String>,
    pub revisions: Vec<Revision>,
    pub copies: Vec<Copy>,
}
#[derive(Default, Deserialize)]
pub struct Request {
    pub action: String,
    pub file_id: Option<String>,
    pub copy_id: Option<String>,
    pub expected: Option<String>,
    pub content: Option<String>,
    pub revision: Option<usize>,
    pub name: Option<String>,
    pub observed: Option<BTreeMap<String, String>>,
}
#[derive(Serialize)]
pub struct Response {
    pub files: Vec<ManagedFile>,
}
fn identity(agent: &str, category: &str, scope: &str, path: &str) -> String {
    super::digest(
        serde_json::to_string(&(agent, category, scope, path))
            .unwrap()
            .as_bytes(),
    )
}
fn revision(content: String, number: usize, reason: &str) -> Revision {
    Revision {
        fingerprint: super::digest(content.as_bytes()),
        content,
        number,
        created_at: chrono::Utc::now().to_rfc3339(),
        reason: reason.into(),
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Source {
    #[serde(default)]
    group: String,
    root: String,
    pattern: String,
    category: String,
}
#[derive(Clone, Deserialize)]
struct Observed {
    #[serde(default)]
    group: String,
    path: String,
    relative: String,
    category: String,
    root: String,
    content: Option<String>,
    fingerprint: Option<String>,
    error: Option<String>,
}
fn sources(c: &super::catalog::CatalogEntry, adapters: &[Box<dyn AgentAdapter>]) -> Vec<Source> {
    let mut out = vec![];
    let mut push = |root: &str, pattern: String, category: &str| {
        out.push(Source {
            group: String::new(),
            root: root.into(),
            pattern,
            category: category.into(),
        })
    };
    if c.project.is_some() {
        if let Some(a) = adapters.iter().find(|a| a.name() == c.name) {
            for (kind, patterns) in [
                ("instructions", a.project_rules_patterns()),
                ("instructions", a.project_memory_patterns()),
                ("config", a.project_settings_patterns()),
                (
                    "config",
                    a.project_mcp_config_relpath().into_iter().collect(),
                ),
                (
                    "hooks",
                    a.project_hook_config_relpath().into_iter().collect(),
                ),
                ("skills", a.project_skill_read_dirs()),
            ] {
                for p in patterns {
                    push(&c.home, p, kind);
                }
            }
        }
        let relative = c
            .root
            .strip_prefix(&format!("{}/", c.home.trim_end_matches('/')))
            .unwrap_or(&c.root)
            .to_string();
        push(&c.home, relative, &c.kind);
    } else {
        for p in [
            "config.toml",
            "settings.json",
            "config.json",
            "mcp.json",
            "settings.yaml",
            "config.yaml",
            "opencode.json",
            "opencode.jsonc",
        ] {
            push(&c.home, p.into(), "config");
        }
        for p in [
            "AGENTS.md",
            "CLAUDE.md",
            "GEMINI.md",
            "rules",
            "memories",
            "instructions",
        ] {
            push(&c.home, p.into(), "instructions");
        }
        push(&c.home, "hooks".into(), "hooks");
        for p in ["agents", "commands", "output-styles", "workflows"] {
            push(&c.home, p.into(), "config");
        }
        if let Some(a) = adapters.iter().find(|a| a.name() == c.name) {
            for (kind, paths) in [
                ("config", a.global_settings_files()),
                ("instructions", a.global_rules_files()),
                ("instructions", a.global_memory_files()),
                ("config", a.global_subagent_files()),
                ("config", a.global_workflow_files()),
                ("config", vec![a.mcp_config_path()]),
                ("hooks", vec![a.hook_config_path()]),
            ] {
                for p in paths {
                    if let Ok(rel) = p.strip_prefix(a.base_dir()) {
                        push(&c.home, rel.to_string_lossy().into(), kind);
                    } else if c.environment == "local" {
                        if let Some(parent) = p.parent() {
                            push(
                                &parent.to_string_lossy(),
                                p.file_name().unwrap_or_default().to_string_lossy().into(),
                                kind,
                            );
                        }
                    } else if let Some(base_parent) = a.base_dir().parent() {
                        if let Ok(rel) = p.strip_prefix(base_parent) {
                            if let Some((parent, _)) = c.home.replace('\\', "/").rsplit_once('/') {
                                push(parent, rel.to_string_lossy().into(), kind);
                            }
                        }
                    }
                }
            }
        }
        // Skills roots may be outside the agent's home (for example ~/.agents/skills).
        push(&c.root, "".into(), "skills");
    }
    out
}
fn local_observe(sources: &[Source]) -> Vec<Observed> {
    let mut found = BTreeMap::new();
    for s in sources {
        let pattern = Path::new(&s.root)
            .join(&s.pattern)
            .to_string_lossy()
            .into_owned();
        let matches = glob::glob(&pattern)
            .into_iter()
            .flatten()
            .filter_map(Result::ok);
        for p in matches {
            for entry in walkdir::WalkDir::new(&p)
                .follow_links(false)
                .max_depth(16)
                .into_iter()
                .filter_entry(|e| {
                    ![".git", "node_modules", "target", ".venv", "__pycache__"]
                        .contains(&e.file_name().to_string_lossy().as_ref())
                })
                .take(10000)
                .filter_map(Result::ok)
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                // Never read a symlink through an ancestor, even when a glob crossed it.
                if path
                    .ancestors()
                    .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
                {
                    continue;
                }
                let snapshot = super::read_text(path, &[PathBuf::from(&s.root)]);
                let (content, fingerprint, error) = match snapshot {
                    Ok(t) => (Some(t.content), Some(t.revision), None),
                    Err(e) => (None, None, Some(e.to_string())),
                };
                let relative = path
                    .strip_prefix(&s.root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let path = path.to_string_lossy().into_owned();
                found.entry(path.clone()).or_insert(Observed {
                    group: s.group.clone(),
                    path,
                    relative,
                    category: s.category.clone(),
                    root: s.root.clone(),
                    content,
                    fingerprint,
                    error,
                });
            }
        }
    }
    found.into_values().collect()
}
fn scan(r: &mut Registry, adapters: &[Box<dyn AgentAdapter>]) -> Result<(), HkError> {
    let mut seen = BTreeSet::new();
    let mut groups = BTreeMap::<
        (String, String, String, String),
        (super::catalog::CatalogEntry, Vec<Source>),
    >::new();
    for c in &r.catalog {
        let scope = if c.project.is_some() {
            format!("project:{}:{}", c.environment, c.home)
        } else if c.scope.is_empty() {
            "global".into()
        } else {
            c.scope.clone()
        };
        let k = (c.environment.clone(), c.name.clone(), scope, c.home.clone());
        let entry = groups.entry(k).or_insert_with(|| (c.clone(), vec![]));
        entry.1.extend(sources(c, adapters));
    }
    // One transport invocation per environment, including environments with many agents.
    let mut batches = BTreeMap::<String, Vec<Source>>::new();
    for ((environment, agent, scope, home), (_, src)) in &mut groups {
        let group = identity(agent, "", scope, home);
        for s in src.iter_mut() {
            s.group = group.clone();
        }
        batches
            .entry(environment.clone())
            .or_default()
            .extend(src.clone());
    }
    let mut remote = BTreeMap::<String, Result<Vec<Observed>, String>>::new();
    for (environment, src) in batches {
        if let Some(env) = r
            .environments
            .iter()
            .find(|e| e.id == environment && !matches!(e.connection, Connection::Local))
        {
            let result = transport::run(
                &env.connection,
                serde_json::json!({"action":"managed_scan","sources":src}),
            )
            .and_then(|v| serde_json::from_value::<Vec<Observed>>(v).map_err(Into::into))
            .map_err(|e| e.to_string());
            remote.insert(environment, result);
        }
    }
    for ((env_id, agent, scope, scope_home), (c, src)) in groups {
        let Some(env) = r.environments.iter().find(|e| e.id == env_id) else {
            continue;
        };
        let enabled = c.home_enabled && !env.paused;
        let observations = if matches!(env.connection, Connection::Local) {
            Ok(local_observe(&src))
        } else {
            let group = identity(&agent, "", &scope, &scope_home);
            remote
                .get(&env_id)
                .cloned()
                .unwrap_or_else(|| Err("Environment unavailable".into()))
                .map(|items| items.into_iter().filter(|o| o.group == group).collect())
        };
        let observations = match observations {
            Ok(v) => v,
            Err(e) => {
                for f in &mut r.managed_files {
                    for cp in &mut f.copies {
                        if cp.environment_id == env_id && f.agent == agent {
                            cp.error = Some(e.to_string());
                        }
                    }
                }
                continue;
            }
        };
        for o in observations {
            let id = identity(&agent, &o.category, &scope, &o.relative);
            let copy_id = identity(&env_id, &agent, &scope, &o.path);
            seen.insert(copy_id.clone());
            let index = if let Some(i) = r.managed_files.iter().position(|f| f.id == id) {
                i
            } else {
                r.managed_files.push(ManagedFile {
                    id: id.clone(),
                    agent: agent.clone(),
                    category: o.category.clone(),
                    scope: scope.clone(),
                    scope_path: (scope != "global").then(|| scope_home.clone()),
                    file_name: o.relative.rsplit('/').next().unwrap_or(&o.relative).into(),
                    logical_path: o.relative.clone(),
                    variant_name: None,
                    original_id: None,
                    revisions: o
                        .content
                        .clone()
                        .map(|s| vec![revision(s, 1, "discovered")])
                        .unwrap_or_default(),
                    copies: vec![],
                });
                r.managed_files.len() - 1
            };
            let f = &mut r.managed_files[index];
            let accepted = f
                .copies
                .iter()
                .find(|p| p.id == copy_id)
                .and_then(|p| p.accepted_fingerprint.clone())
                .or_else(|| f.revisions.last().map(|v| v.fingerprint.clone()));
            let error = o.error.or_else(|| {
                (o.fingerprint == accepted
                    && f.revisions
                        .last()
                        .is_some_and(|rev| Some(&rev.fingerprint) != o.fingerprint.as_ref()))
                .then(|| {
                    "Not synced to the latest version; retry when this copy is available".into()
                })
            });
            let path_enabled = r
                .catalog
                .iter()
                .filter(|entry| {
                    entry.environment == env_id
                        && entry.name == agent
                        && (entry.root == o.path
                            || (entry.is_directory
                                && o.path.starts_with(&format!(
                                    "{}/",
                                    entry.root.trim_end_matches('/')
                                ))))
                })
                .all(|entry| {
                    entry.enabled
                        && entry.home_enabled
                        && entry.provider.as_ref().is_none_or(|id| {
                            env.providers
                                .iter()
                                .find(|p| &p.id == id)
                                .is_none_or(|p| !p.paused && !p.disabled)
                        })
                });
            let cp = Copy {
                id: copy_id.clone(),
                environment_id: env_id.clone(),
                environment: env.name.clone(),
                machine: env.machine.clone(),
                path: o.path,
                provider_id: c.provider.clone().unwrap_or(c.id.clone()),
                pending: o.fingerprint != accepted,
                accepted_fingerprint: accepted,
                content: o.content,
                fingerprint: o.fingerprint,
                error,
                enabled: enabled && path_enabled,
                root: o.root,
            };
            if let Some(old) = f.copies.iter_mut().find(|p| p.id == copy_id) {
                *old = cp;
            } else {
                f.copies.push(cp);
            }
        }
    }
    for f in &mut r.managed_files {
        for c in &mut f.copies {
            if !seen.contains(&c.id) && c.error.is_none() {
                c.error = Some("File unavailable or no longer in the discovered inventory".into());
            }
        }
    }
    r.managed_files.sort_by(|a, b| {
        (&a.agent, &a.scope, &a.logical_path, &a.variant_name).cmp(&(
            &b.agent,
            &b.scope,
            &b.logical_path,
            &b.variant_name,
        ))
    });
    Ok(())
}
fn conflict() -> HkError {
    HkError::Conflict("File changed; refresh before saving or accepting".into())
}
fn mutate(r: &mut Registry, req: &Request) -> Result<(), HkError> {
    let i = r
        .managed_files
        .iter()
        .position(|f| Some(&f.id) == req.file_id.as_ref())
        .ok_or_else(|| HkError::NotFound("Managed file".into()))?;
    if req.action == "variant" {
        let family = r.managed_files[i]
            .original_id
            .as_ref()
            .unwrap_or(&r.managed_files[i].id);
        let name = req.name.as_deref().unwrap_or("").trim();
        if r.managed_files.iter().any(|f| {
            f.original_id.as_ref().unwrap_or(&f.id) == family
                && f.variant_name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        }) {
            return Err(HkError::Validation(
                "A variant with this name already exists".into(),
            ));
        }
    }
    let f = &mut r.managed_files[i];
    let latest = f
        .revisions
        .last()
        .ok_or_else(|| HkError::Validation("This file has no editable text revision".into()))?
        .clone();
    if req.action == "retry" {
        return Ok(());
    }
    if req.action != "accept" && req.expected.as_deref() != Some(&latest.fingerprint) {
        return Err(conflict());
    }
    if req.action == "variant" {
        let name = req.name.as_deref().unwrap_or("").trim();
        if name.is_empty() || name.len() > 120 || name.contains(['\0', '\n', '\r']) {
            return Err(HkError::Validation(
                "Use a variant name of 1–120 characters".into(),
            ));
        }
        let mut v = f.clone();
        v.id = uuid::Uuid::new_v4().to_string();
        v.original_id = Some(f.original_id.clone().unwrap_or_else(|| f.id.clone()));
        v.variant_name = Some(name.into());
        v.copies.clear();
        v.revisions = vec![revision(latest.content, 1, "variant")];
        r.managed_files.push(v);
        return Ok(());
    }
    let content = match req.action.as_str() {
        "accept" => {
            let c = f
                .copies
                .iter_mut()
                .find(|c| Some(&c.id) == req.copy_id.as_ref())
                .ok_or_else(|| HkError::NotFound("File copy".into()))?;
            if c.error.is_some() || req.expected.is_none() || req.expected != c.fingerprint {
                return Err(conflict());
            }
            let content = c
                .content
                .clone()
                .ok_or_else(|| HkError::Validation("File is not editable text".into()))?;
            c.accepted_fingerprint = c.fingerprint.clone();
            c.pending = false;
            content
        }
        "save" => {
            for c in f.copies.iter().filter(|c| c.enabled && c.pending) {
                if c.error.is_some()
                    || c.fingerprint.is_none()
                    || req
                        .observed
                        .as_ref()
                        .and_then(|observed| observed.get(&c.id))
                        != c.fingerprint.as_ref()
                {
                    return Err(conflict());
                }
            }
            for c in f.copies.iter_mut().filter(|c| c.enabled && c.pending) {
                c.accepted_fingerprint = c.fingerprint.clone();
                c.pending = false;
            }
            req.content
                .clone()
                .ok_or_else(|| HkError::Validation("Content required".into()))?
        }
        "restore" => f
            .revisions
            .iter()
            .find(|v| Some(v.number) == req.revision)
            .ok_or_else(|| HkError::NotFound("Revision".into()))?
            .content
            .clone(),
        _ => return Err(HkError::Validation("Unknown managed file action".into())),
    };
    if content.len() > 4 * 1024 * 1024 || content.contains('\0') {
        return Err(HkError::Validation("Invalid text or file too large".into()));
    }
    if content != latest.content {
        f.revisions
            .push(revision(content, latest.number + 1, &req.action));
    }
    Ok(())
}
fn copy_text(
    home: &Path,
    r: &Registry,
    c: &Copy,
    save: Option<(&str, &str)>,
) -> Result<TextSnapshot, HkError> {
    let e = r
        .environments
        .iter()
        .find(|e| e.id == c.environment_id)
        .ok_or_else(|| HkError::NotFound("Environment".into()))?;
    if matches!(e.connection, Connection::Local) {
        let roots = [PathBuf::from(&c.root)];
        return match save {
            Some((expected, content)) => super::save_text(
                Path::new(&c.path),
                expected,
                content,
                &roots,
                &home.join(".harnesskit/comparison-history"),
            ),
            None => super::read_text(Path::new(&c.path), &roots),
        };
    }
    let mut req = serde_json::json!({"action":"managed_read","root":c.root,"path":c.path});
    if let Some((expected, content)) = save {
        req["action"] = "managed_save".into();
        req["expected"] = expected.into();
        req["content"] = content.into();
    }
    Ok(serde_json::from_value(transport::run(&e.connection, req)?)?)
}
fn synchronize(home: &Path, r: &mut Registry, id: &str) {
    let Some(i) = r.managed_files.iter().position(|f| f.id == id) else {
        return;
    };
    let Some(latest) = r.managed_files[i].revisions.last().cloned() else {
        return;
    };
    for n in 0..r.managed_files[i].copies.len() {
        let c = r.managed_files[i].copies[n].clone();
        if !c.enabled {
            continue;
        }
        let result = (|| {
            let current = copy_text(home, r, &c, None)?;
            if Some(&current.revision) != c.accepted_fingerprint.as_ref()
                && current.revision != latest.fingerprint
            {
                return Err(conflict());
            }
            if current.revision == latest.fingerprint {
                return Ok(current);
            }
            copy_text(home, r, &c, Some((&current.revision, &latest.content)))
        })();
        let target = &mut r.managed_files[i].copies[n];
        match result {
            Ok(t) => {
                target.content = Some(t.content);
                target.fingerprint = Some(t.revision.clone());
                target.accepted_fingerprint = Some(t.revision);
                target.error = None;
                target.pending = false;
            }
            Err(e) => {
                target.error = Some(e.to_string());
            }
        }
    }
}
pub fn execute(
    home: &Path,
    adapters: &[Box<dyn AgentAdapter>],
    store: &Store,
    req: Request,
) -> Result<Response, HkError> {
    let c = Coordinator::new(home.join(".harnesskit/environments"));
    // Serialize entire command across API callers. The coordinator lock also protects each persisted phase.
    static COMMANDS: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _guard = COMMANDS.lock();
    let (changed, previous_count) = c.transaction(|r| {
        if !r.environments.iter().any(|e| e.id == "local") {
            r.environments.push(super::registry::Environment {
                id: "local".into(),
                name: "Local".into(),
                machine: std::env::var("HOSTNAME").unwrap_or_else(|_| "This machine".into()),
                os: std::env::consts::OS.into(),
                paused: false,
                providers: vec![],
                connection: Connection::Local,
            });
        }
        super::catalog::reconcile_local(r, home, adapters, store)?;
        super::nested::reconcile(r, adapters, store)?;
        scan(r, adapters)?;
        let before = r
            .managed_files
            .iter()
            .find(|f| Some(&f.id) == req.file_id.as_ref())
            .map(|f| f.revisions.len())
            .unwrap_or(0);
        if req.action != "list" {
            mutate(r, &req)?;
        }
        let after = r
            .managed_files
            .iter()
            .find(|f| Some(&f.id) == req.file_id.as_ref())
            .map(|f| f.revisions.len())
            .unwrap_or(0);
        Ok((before != after, before))
    })?;
    // Revision durability precedes attempts to write copies. A crash can be recovered with Retry.
    let response = c.transaction(|r| {
        if ["save", "accept", "restore", "retry"].contains(&req.action.as_str())
            && (req.action != "save" || changed || req.observed.is_some())
        {
            if let Some(id) = &req.file_id {
                synchronize(home, r, id);
            }
        }
        if changed && req.action == "save" {
            if let Some(f) = r
                .managed_files
                .iter_mut()
                .find(|f| Some(&f.id) == req.file_id.as_ref())
            {
                let latest = f.revisions.last().unwrap();
                if !f.copies.is_empty()
                    && !f.copies.iter().any(|c| {
                        c.enabled
                            && c.error.is_none()
                            && c.fingerprint.as_deref() == Some(&latest.fingerprint)
                    })
                {
                    f.revisions.truncate(previous_count);
                }
            }
        }
        Ok(Response {
            files: r.managed_files.clone(),
        })
    })?;
    if changed && req.action == "save" {
        if let Some(f) = response
            .files
            .iter()
            .find(|f| Some(&f.id) == req.file_id.as_ref())
        {
            if f.revisions.len() == previous_count {
                return Err(HkError::Conflict(
                    f.copies
                        .iter()
                        .find_map(|c| c.error.clone())
                        .unwrap_or_else(|| "No enabled copies could be saved".into()),
                ));
            }
        }
    }
    Ok(response)
}

/// Comparison editors reuse managed history when their selected path is already inventoried.
pub fn comparison_save(
    home: &Path,
    adapters: &[Box<dyn AgentAdapter>],
    store: &Store,
    environment: Option<&str>,
    path: &str,
    expected: &str,
    content: &str,
) -> Result<Option<TextSnapshot>, HkError> {
    let files = execute(
        home,
        adapters,
        store,
        Request {
            action: "list".into(),
            ..Default::default()
        },
    )?
    .files;
    let selected = files.iter().find_map(|f| {
        f.copies
            .iter()
            .find(|c| {
                c.environment_id == environment.unwrap_or("local")
                    && c.path.replace('\\', "/") == path.replace('\\', "/")
            })
            .map(|c| (f, c))
    });
    let Some((file, copy)) = selected else {
        return Ok(None);
    };
    if copy.fingerprint.as_deref() != Some(expected) {
        return Err(conflict());
    }
    let coordinator = Coordinator::new(home.join(".harnesskit/environments"));
    coordinator.transaction(|r| {
        let i = r
            .managed_files
            .iter()
            .position(|f| f.id == file.id)
            .ok_or_else(conflict)?;
        if r.managed_files[i].revisions.len() != file.revisions.len() {
            return Err(conflict());
        }
        let current = copy_text(home, r, copy, None)?;
        if current.revision != expected {
            return Err(conflict());
        }
        if current.content == content {
            return Ok(Some(current));
        }
        let saved = copy_text(home, r, copy, Some((expected, content)))?;
        let f = &mut r.managed_files[i];
        if f.revisions.last().is_none_or(|rev| rev.content != content) {
            let number = f.revisions.last().map_or(1, |rev| rev.number + 1);
            f.revisions
                .push(revision(content.into(), number, "comparison edit"));
        }
        for c in &mut f.copies {
            if c.id == copy.id {
                c.content = Some(saved.content.clone());
                c.fingerprint = Some(saved.revision.clone());
                c.accepted_fingerprint = Some(saved.revision.clone());
                c.pending = false;
                c.error = None;
            } else if c.fingerprint.as_deref() != Some(&saved.revision) && c.error.is_none() {
                c.error =
                    Some("Not synced to the latest version; retry to apply this version".into());
            }
        }
        Ok(Some(saved))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Registry, String) {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("agent");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("config.toml"), "one").unwrap();
        let mut r = Registry {
            version: 1,
            ..Registry::default()
        };
        r.environments.push(super::super::registry::Environment {
            id: "local".into(),
            name: "Local".into(),
            machine: "Test".into(),
            os: "linux".into(),
            paused: false,
            providers: vec![],
            connection: Connection::Local,
        });
        r.catalog.push(super::super::catalog::CatalogEntry {
            id: "a".into(),
            environment: "local".into(),
            name: "codex".into(),
            home: root.to_string_lossy().into(),
            root: root.join("skills").to_string_lossy().into(),
            enabled: true,
            home_enabled: true,
            ..Default::default()
        });
        scan(&mut r, &[]).unwrap();
        let id = r.managed_files[0].id.clone();
        (t, r, id)
    }
    fn req(action: &str, id: &str, expected: Option<String>) -> Request {
        Request {
            action: action.into(),
            file_id: Some(id.into()),
            expected,
            ..Default::default()
        }
    }

    fn remote(request: serde_json::Value) -> serde_json::Value {
        use std::io::Write;
        let mut child = std::process::Command::new("python3")
            .args(["-c", include_str!("remote.py")])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
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
    fn remote_config_without_skills_root_and_cas() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("config.toml");
        std::fs::write(&p, "one").unwrap();
        let v = remote(
            serde_json::json!({"action":"managed_scan","sources":[{"root":t.path(),"pattern":"config.toml","category":"config"},{"root":t.path().join("skills"),"pattern":"","category":"skills"}]}),
        );
        let files = v.get("result").unwrap_or(&v);
        assert_eq!(files.as_array().unwrap().len(), 1);
        assert_eq!(files[0]["content"], "one");
        let v = remote(
            serde_json::json!({"action":"managed_save","root":t.path(),"path":p,"expected":"stale","content":"two"}),
        );
        assert!(v["error"].is_string());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "one");
        let v = remote(
            serde_json::json!({"action":"managed_save","root":t.path(),"path":p,"expected":super::super::digest(b"one"),"content":"two"}),
        );
        assert!(v.get("error").is_none(), "{v}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "two");
    }
    #[test]
    fn command_persists_versions_comparison_edit_and_rejects_failed_save() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join(".codex");
        std::fs::create_dir(&root).unwrap();
        let p = root.join("config.toml");
        std::fs::write(&p, "one").unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(
            crate::adapter::codex::CodexAdapter::with_home(t.path().into()),
        )];
        let store = Store::open(&t.path().join("db")).unwrap();
        let list = || {
            execute(
                t.path(),
                &adapters,
                &store,
                Request {
                    action: "list".into(),
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let f = list().files.remove(0);
        assert_eq!(f.revisions.len(), 1);
        assert!(!root.join("skills").exists());
        comparison_save(
            t.path(),
            &adapters,
            &store,
            None,
            p.to_str().unwrap(),
            &f.revisions[0].fingerprint,
            "two",
        )
        .unwrap()
        .unwrap();
        let f = list().files.remove(0);
        assert_eq!(f.revisions.len(), 2);
        assert_eq!(f.revisions[1].content, "two");
        let mut save = req("save", &f.id, Some(f.revisions[1].fingerprint.clone()));
        save.content = Some("three".into());
        std::fs::remove_file(&p).unwrap();
        assert!(execute(t.path(), &adapters, &store, save).is_err());
        assert_eq!(list().files[0].revisions.len(), 2);
        let mut variant = req("variant", &f.id, Some(f.revisions[1].fingerprint.clone()));
        variant.name = Some("Personal".into());
        execute(t.path(), &adapters, &store, variant).unwrap();
        let mut duplicate = req("variant", &f.id, Some(f.revisions[1].fingerprint.clone()));
        duplicate.name = Some("personal".into());
        assert!(execute(t.path(), &adapters, &store, duplicate).is_err());
    }
    #[test]
    fn merge_requires_all_pending_fingerprints_and_preserves_normal_save_guard() {
        let (t, mut r, id) = fixture();
        std::fs::write(t.path().join("agent/config.toml"), "external").unwrap();
        scan(&mut r, &[]).unwrap();
        let mut save = req("save", &id, Some(super::super::digest(b"one")));
        save.content = Some("merged".into());
        assert!(mutate(&mut r, &save).is_err());
        let copy = r.managed_files[0].copies[0].clone();
        save.observed = Some(BTreeMap::from([(copy.id.clone(), "stale".into())]));
        assert!(mutate(&mut r, &save).is_err());
        save.observed = Some(BTreeMap::from([(copy.id, copy.fingerprint.unwrap())]));
        mutate(&mut r, &save).unwrap();
        synchronize(t.path(), &mut r, &id);
        assert_eq!(
            std::fs::read_to_string(t.path().join("agent/config.toml")).unwrap(),
            "merged"
        );
        assert_eq!(r.managed_files[0].revisions.len(), 2);
        assert!(!r.managed_files[0].copies[0].pending);
    }
    #[test]
    fn tracks_noop_background_accept_restore_variant_and_conflicts() {
        let (t, mut r, id) = fixture();
        let fp = r.managed_files[0].revisions[0].fingerprint.clone();
        let mut save = req("save", &id, Some(fp.clone()));
        save.content = Some("one".into());
        mutate(&mut r, &save).unwrap();
        assert_eq!(r.managed_files[0].revisions.len(), 1);
        std::fs::write(t.path().join("agent/config.toml"), "external").unwrap();
        scan(&mut r, &[]).unwrap();
        assert!(r.managed_files[0].copies[0].pending);
        assert_eq!(r.managed_files[0].revisions.len(), 1);
        save.content = Some("two".into());
        assert!(mutate(&mut r, &save).is_err());
        let copy = r.managed_files[0].copies[0].clone();
        let mut accept = req("accept", &id, copy.fingerprint);
        accept.copy_id = Some(copy.id);
        mutate(&mut r, &accept).unwrap();
        assert_eq!(r.managed_files[0].revisions.len(), 2);
        let latest = r.managed_files[0]
            .revisions
            .last()
            .unwrap()
            .fingerprint
            .clone();
        let mut variant = req("variant", &id, Some(latest.clone()));
        variant.name = Some("Personal".into());
        mutate(&mut r, &variant).unwrap();
        assert_eq!(r.managed_files[1].revisions.len(), 1);
        assert_ne!(r.managed_files[1].id, id);
        assert!(r.managed_files[1].copies.is_empty());
        let mut restore = req("restore", &id, Some(latest));
        restore.revision = Some(1);
        mutate(&mut r, &restore).unwrap();
        assert_eq!(r.managed_files[0].revisions.len(), 3);
        assert_eq!(
            std::fs::read_to_string(t.path().join("agent/config.toml")).unwrap(),
            "external"
        );
    }
    #[test]
    fn saved_revision_survives_conflicting_copy_and_retry_never_overwrites_external_content() {
        let (t, mut r, id) = fixture();
        let second = t.path().join("agent/second.toml");
        std::fs::write(&second, "one").unwrap();
        let mut cp = r.managed_files[0].copies[0].clone();
        cp.id = "second".into();
        cp.path = second.to_string_lossy().into();
        r.managed_files[0].copies.push(cp);
        let mut save = req("save", &id, Some(super::super::digest(b"one")));
        save.content = Some("two".into());
        mutate(&mut r, &save).unwrap();
        std::fs::write(&second, "outside").unwrap();
        synchronize(t.path(), &mut r, &id);
        assert_eq!(
            std::fs::read_to_string(t.path().join("agent/config.toml")).unwrap(),
            "two"
        );
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "outside");
        assert!(r.managed_files[0].copies[1].error.is_some());
        assert_eq!(r.managed_files[0].revisions.len(), 2);
        synchronize(t.path(), &mut r, &id);
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "outside");
        assert_eq!(r.managed_files[0].revisions.len(), 2);
    }
    #[test]
    fn failed_sync_issue_survives_rescan_and_new_copy_difference_is_pending() {
        let (t, mut r, id) = fixture();
        let mut save = req("save", &id, Some(super::super::digest(b"one")));
        save.content = Some("two".into());
        mutate(&mut r, &save).unwrap();
        scan(&mut r, &[]).unwrap();
        assert!(r.managed_files[0].copies[0].error.is_some());
        assert!(!r.managed_files[0].copies[0].pending);
        assert_eq!(r.managed_files[0].revisions.len(), 2);
        let coordinator = Coordinator::new(t.path().join("state"));
        coordinator
            .transaction(|state| {
                *state = r.clone();
                Ok(())
            })
            .unwrap();
        coordinator
            .transaction(|state| {
                assert_eq!(state.managed_files[0].revisions.len(), 2);
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn unavailable_retains_cached_revision_and_project_paths_do_not_coalesce() {
        let (t, mut r, _) = fixture();
        std::fs::remove_file(t.path().join("agent/config.toml")).unwrap();
        scan(&mut r, &[]).unwrap();
        assert_eq!(r.managed_files.len(), 1);
        assert!(r.managed_files[0].copies[0].error.is_some());
        assert_eq!(r.managed_files[0].revisions.len(), 1);
        assert_ne!(
            identity("a", "config", "project:/one/app", "config"),
            identity("a", "config", "project:/two/app", "config")
        );
    }
}
