//! Bounded metadata-only filesystem navigation and project recognition.
use super::{
    nested,
    registry::Registry,
    transport::{self, Connection},
};
use crate::{HkError, adapter::AgentAdapter};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Serialize, Deserialize)]
pub struct Component {
    pub relative: String,
    pub kind: String,
    pub is_directory: bool,
}
#[derive(Serialize, Deserialize)]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
}
#[derive(Serialize, Deserialize)]
pub struct Browse {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<BrowseEntry>,
    pub truncated: bool,
}
#[derive(Serialize, Deserialize)]
struct Found {
    root: String,
    agent: String,
    components: Vec<Component>,
}
fn safe(path: &Path) -> Result<PathBuf, HkError> {
    for p in path.ancestors() {
        if std::fs::symlink_metadata(p)?.file_type().is_symlink() {
            return Err(HkError::Validation(
                "Symlink paths are not traversed".into(),
            ));
        }
    }
    Ok(std::fs::canonicalize(path)?)
}
fn supported(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|x| x.to_str()),
        Some(
            "md" | "mdc" | "toml" | "json" | "jsonc" | "yaml" | "yml" | "txt" | "sh" | "js" | "ts"
        )
    )
}
pub fn browse(r: &Registry, environment: &str, path: &str) -> Result<Browse, HkError> {
    let path = nested::validate(r, environment, path)?;
    let e = r.environments.iter().find(|e| e.id == environment).unwrap();
    if !matches!(e.connection, Connection::Local) {
        return Ok(serde_json::from_value(transport::run(
            &e.connection,
            serde_json::json!({"action":"browse_paths","path":path}),
        )?)?);
    }
    let mut candidate = PathBuf::from(&path);
    while !candidate.is_dir() {
        if std::fs::symlink_metadata(&candidate).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(HkError::Validation(
                "Symlink paths are not traversed".into(),
            ));
        }
        if !candidate.pop() {
            return Err(HkError::Validation("No accessible parent directory".into()));
        }
    }
    let path = safe(&candidate)?;
    let mut entries = vec![];
    let mut truncated = false;
    for (i, e) in std::fs::read_dir(&path)?.enumerate() {
        if i >= 10000 {
            truncated = true;
            break;
        }
        let e = e?;
        let t = e.file_type()?;
        if t.is_symlink() || (!t.is_dir() && !(t.is_file() && supported(&e.path()))) {
            continue;
        }
        entries.push(BrowseEntry {
            name: e.file_name().to_string_lossy().into(),
            path: e.path().to_string_lossy().into(),
            is_directory: t.is_dir(),
        });
    }
    entries.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then(a.name.cmp(&b.name))
    });
    Ok(Browse {
        parent: path.parent().map(|p| p.to_string_lossy().into()),
        path: path.to_string_lossy().into(),
        entries,
        truncated,
    })
}
fn specifications(a: &dyn AgentAdapter) -> Vec<Component> {
    let mut specs = vec![];
    for (kind, patterns, dir) in [
        ("skills", a.project_skill_dirs(), true),
        ("instructions", a.project_rules_patterns(), false),
        ("instructions", a.project_memory_patterns(), false),
        ("config", a.project_settings_patterns(), false),
        (
            "config",
            a.project_mcp_config_relpath().into_iter().collect(),
            false,
        ),
        (
            "hooks",
            a.project_hook_config_relpath().into_iter().collect(),
            false,
        ),
    ] {
        for relative in patterns {
            specs.push(Component {
                relative,
                kind: kind.into(),
                is_directory: dir,
            });
        }
    }
    specs
}
fn contains_skill_files(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.take(10000).filter_map(Result::ok).any(|entry| {
        entry.file_type().is_ok_and(|kind| kind.is_dir())
            && ["SKILL.md", "SKILL.md.disabled"].iter().any(|name| {
                std::fs::symlink_metadata(entry.path().join(name))
                    .is_ok_and(|meta| meta.file_type().is_file())
            })
    })
}
fn component_paths(root: &Path, pattern: &str) -> Vec<PathBuf> {
    if !pattern.contains(['*', '?', '[']) {
        return vec![root.join(pattern)];
    }
    let mut base = root.to_path_buf();
    for part in pattern.split('/') {
        if part.contains(['*', '?', '[']) {
            break;
        }
        base.push(part);
    }
    if safe(&base).is_err() {
        return vec![];
    }
    let Ok(pattern) = glob::Pattern::new(pattern) else {
        return vec![];
    };
    walkdir::WalkDir::new(base)
        .follow_links(false)
        .max_depth(12)
        .into_iter()
        .filter_entry(|e| !e.file_type().is_symlink() && !skip(&e.file_name().to_string_lossy()))
        .take(1000)
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| {
            p.strip_prefix(root).is_ok_and(|rel| {
                pattern.matches_path_with(
                    rel,
                    glob::MatchOptions {
                        case_sensitive: true,
                        require_literal_separator: true,
                        require_literal_leading_dot: false,
                    },
                )
            })
        })
        .collect()
}
fn skip(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "vendor"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".next"
            | ".cache"
    )
}
pub fn discover(
    r: &mut Registry,
    environment: &str,
    agent: Option<&str>,
    root: &str,
    adapters: &[Box<dyn AgentAdapter>],
) -> Result<(), HkError> {
    let root = nested::validate(r, environment, root)?;
    let selected: Vec<_> = adapters
        .iter()
        .filter(|a| agent.is_none_or(|n| n == a.name()))
        .collect();
    if selected.is_empty() {
        return Err(HkError::NotFound("Agent".into()));
    }
    let e = r.environments.iter().find(|e| e.id == environment).unwrap();
    let specs: Vec<_> = selected
        .iter()
        .map(|a| serde_json::json!({"agent":a.name(),"components":specifications(a.as_ref())}))
        .collect();
    let found: Vec<Found> = if !matches!(e.connection, Connection::Local) {
        serde_json::from_value(transport::run(
            &e.connection,
            serde_json::json!({"action":"discover_projects","root":root,"specifications":specs}),
        )?)?
    } else {
        let root = safe(Path::new(&root))?;
        if !root.is_dir() {
            return Err(HkError::Validation("Select a directory".into()));
        }
        let mut found = vec![];
        for item in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .max_depth(12)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !skip(&e.file_name().to_string_lossy()))
            .take(10000)
        {
            let item = match item {
                Ok(x) => x,
                Err(_) => continue,
            };
            if !item.file_type().is_dir() {
                continue;
            }
            for a in &selected {
                let mut components = vec![];
                for spec in specifications(a.as_ref()) {
                    for p in component_paths(item.path(), &spec.relative) {
                        if safe(&p).is_err()
                            || (spec.is_directory && !p.is_dir())
                            || (!spec.is_directory && !p.is_file())
                            || (spec.kind == "skills" && !contains_skill_files(&p))
                        {
                            continue;
                        }
                        let relative = p
                            .strip_prefix(item.path())
                            .unwrap()
                            .to_string_lossy()
                            .into_owned();
                        if !components
                            .iter()
                            .any(|c: &Component| c.relative == relative)
                        {
                            components.push(Component {
                                relative,
                                kind: spec.kind.clone(),
                                is_directory: spec.is_directory,
                            });
                        }
                    }
                }
                if !components.is_empty() {
                    found.push(Found {
                        root: item.path().to_string_lossy().into(),
                        agent: a.name().into(),
                        components,
                    });
                }
            }
        }
        found
    };
    if found.is_empty() {
        return Err(HkError::Validation(
            "No recognized project files for the selected agent(s) in this folder".into(),
        ));
    }
    for f in found {
        if r.projects.iter().any(|p| {
            p.environment == environment
                && p.agent == f.agent
                && (p.root == f.root
                    || matches!(
                        r.environments
                            .iter()
                            .find(|e| e.id == environment)
                            .unwrap()
                            .connection,
                        Connection::Local
                    ) && std::fs::canonicalize(&p.root)
                        .is_ok_and(|root| root == Path::new(&f.root)))
        }) {
            continue;
        }
        r.forgotten
            .remove(&format!("project:{environment}/{}/{}", f.agent, f.root));
        r.forgotten
            .remove(&format!("agent:{environment}/{}", f.agent));
        let project_id = uuid::Uuid::new_v4().to_string();
        for component in &f.components {
            let root = format!(
                "{}/{}",
                f.root.trim_end_matches(['/', '\\']),
                component.relative
            );
            let scope = format!("project:{project_id}");
            r.forgotten
                .remove(&format!("location:{environment}/{}/{}", f.agent, root));
            if component.kind != "skills" {
                continue;
            }
            let e = r
                .environments
                .iter_mut()
                .find(|e| e.id == environment)
                .unwrap();
            if !e
                .providers
                .iter()
                .any(|p| p.name == f.agent && p.root == root && p.scope == scope)
            {
                e.providers.push(super::registry::Provider {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: f.agent.clone(),
                    root,
                    scope,
                    paused: true,
                    disabled: false,
                    last_scanned: None,
                    last_error: None,
                });
            }
        }
        r.projects.push(nested::Project {
            id: project_id,
            environment: environment.into(),
            agent: f.agent,
            root: f.root,
            access: Default::default(),
            enabled: true,
            components: f.components,
        });
    }
    Ok(())
}
pub fn remove(
    r: &mut Registry,
    environment: &str,
    entry: Option<&str>,
    project: Option<&str>,
    agent: Option<&str>,
) -> Result<(), HkError> {
    let rows: Vec<_> = r
        .catalog
        .iter()
        .filter(|c| {
            c.environment == environment
                && (entry == Some(c.id.as_str())
                    || project.is_some() && project == c.project.as_deref()
                    || agent == Some(c.name.as_str()))
        })
        .cloned()
        .collect();
    if rows.is_empty()
        && !r
            .projects
            .iter()
            .any(|p| p.environment == environment && project == Some(p.id.as_str()))
    {
        return Err(HkError::NotFound("Registration".into()));
    }
    if let Some(a) = agent {
        r.forgotten.insert(format!("agent:{environment}/{a}"));
    }
    for p in &r.projects {
        if p.environment == environment
            && (project == Some(p.id.as_str()) || agent == Some(p.agent.as_str()))
        {
            r.forgotten
                .insert(format!("project:{environment}/{}/{}", p.agent, p.root));
        }
    }
    r.projects.retain(|p| {
        !(p.environment == environment
            && (project == Some(p.id.as_str()) || agent == Some(p.agent.as_str())))
    });
    for c in rows {
        r.forgotten.insert(format!("path:{environment}/{}", c.id));
        r.forgotten
            .insert(format!("location:{environment}/{}/{}", c.name, c.root));
        if let Some(id) = c.provider {
            if let Some(e) = r.environments.iter_mut().find(|e| e.id == environment) {
                e.providers.retain(|p| p.id != id);
            }
            for s in &mut r.skills {
                s.members
                    .retain(|m| !(m.environment == environment && m.provider == id));
            }
        }
        r.catalog
            .retain(|x| !(x.environment == environment && x.id == c.id));
    }
    Ok(())
}
