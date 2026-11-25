//! Cached remote inventory uses the same scanner and permission inference as local skills.
use super::registry::Registry;
use crate::{HkError, models::Extension};
use serde::Serialize;
#[derive(Serialize)]
pub struct Entry {
    #[serde(flatten)]
    pub extension: Extension,
    pub environment_id: String,
    pub sync_skill_id: String,
    pub sync_member_id: String,
    pub sync_revision: Option<String>,
    pub sync_excluded: bool,
}
static CACHE: std::sync::LazyLock<
    parking_lot::Mutex<std::collections::HashMap<String, Vec<Extension>>>,
> = std::sync::LazyLock::new(|| parking_lot::Mutex::new(std::collections::HashMap::new()));
pub fn inventory(registry: &Registry) -> Result<Vec<Entry>, HkError> {
    let mut result = vec![];
    for skill in &registry.skills {
        for member in &skill.members {
            let Some(env) = registry
                .environments
                .iter()
                .find(|e| e.id == member.environment)
            else {
                continue;
            };
            let Some(provider) = env.providers.iter().find(|p| p.id == member.provider) else {
                continue;
            };
            let Some(snapshot) = skill.revisions.iter().rev().find_map(|r| {
                r.snapshot
                    .as_ref()
                    .filter(|f| Some(&f.fingerprint) == member.fingerprint.as_ref())
            }) else {
                continue;
            };
            let key = format!("{}:{}:{}", snapshot.fingerprint, provider.name, skill.name);
            let cached = { CACHE.lock().get(&key).cloned() };
            let entries = if let Some(entries) = cached {
                entries
            } else {
                let temp = tempfile::tempdir()?;
                super::folders::replace_folder(
                    &temp.path().join(&skill.name),
                    None,
                    Some(snapshot),
                    &temp.path().join("history"),
                )?;
                let entries = crate::scanner::scan_skill_dir(temp.path(), &provider.name, false);
                let mut cache = CACHE.lock();
                if cache.len() >= 1000 {
                    cache.clear();
                }
                cache.insert(key, entries.clone());
                entries
            };
            for mut extension in entries {
                extension.id = format!("environment:{}", member.id);
                extension.source_path = Some(format!("{}/SKILL.md", member.path));
                if let Some(time) = member
                    .scanned_at
                    .as_ref()
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                {
                    extension.installed_at = time.into();
                    extension.updated_at = time.into();
                }
                if let Some(origin) = &member.provenance {
                    if let Some((url, _)) = origin.rsplit_once('#') {
                        extension.source.url = Some(url.into());
                        extension.source.origin = crate::models::SourceOrigin::Git;
                    }
                }
                if let Some(job) = registry.jobs.iter().rev().find(|j| {
                    j.environment == env.id
                        && j.provider == provider.id
                        && j.name == skill.name
                        && j.status == "complete"
                }) {
                    extension.source.url = Some(job.source.clone());
                    extension.source.origin = crate::models::SourceOrigin::Git;
                    extension.source.commit_hash = job.revision.clone();
                    extension.install_meta = Some(crate::models::InstallMeta {
                        install_type: "marketplace".into(),
                        url: Some(job.source.clone()),
                        url_resolved: Some(job.source.clone()),
                        branch: None,
                        subpath: Some(job.name.clone()),
                        revision: job.revision.clone(),
                        remote_revision: None,
                        checked_at: None,
                        check_error: None,
                    });
                }
                extension.scope = if skill.scope == "global" {
                    crate::models::ConfigScope::Global
                } else {
                    crate::models::ConfigScope::Project {
                        name: skill.scope.clone(),
                        path: skill.scope.clone(),
                    }
                };
                result.push(Entry {
                    extension,
                    environment_id: env.id.clone(),
                    sync_skill_id: skill.id.clone(),
                    sync_member_id: member.id.clone(),
                    sync_revision: member.fingerprint.clone(),
                    sync_excluded: skill.stopped
                        || member.excluded
                        || registry.preferences.policy == "none",
                });
            }
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_inventory_retains_disabled_state_and_permission_inference() {
        use super::super::{folders::Folder, registry::*, transport::Connection};
        let f=Folder::new(std::collections::BTreeMap::from([("SKILL.md.disabled".into(),b"---\nname: remote-review\ndescription: Review docs\n---\nRead files with filesystem tools.".to_vec())]));
        let mut registry = Registry::default();
        registry.environments.push(Environment {
            connection: Connection::Local,
            id: "remote".into(),
            machine: "MacBook Air".into(),
            name: "macOS".into(),
            os: "darwin".into(),
            paused: false,
            providers: vec![Provider {
                disabled: false,
                last_error: None,
                last_scanned: None,
                scope: "global".into(),
                id: "codex:0".into(),
                name: "codex".into(),
                root: "/remote/skills".into(),
                paused: false,
            }],
        });
        registry.skills.push(Skill {
            identity_confirmed: true,
            id: "s".into(),
            name: "remote-review".into(),
            scope: "global".into(),
            original: None,
            stopped: false,
            members: vec![Member {
                provenance: None,
                id: "m".into(),
                environment: "remote".into(),
                provider: "codex:0".into(),
                path: "/remote/skills/remote-review".into(),
                excluded: true,
                fingerprint: Some(f.fingerprint.clone()),
                previous: None,
                error: Some("Offline".into()),
                scanned_at: None,
            }],
            revisions: vec![Revision {
                path: None,
                number: 1,
                environment: "remote".into(),
                captured_at: "now".into(),
                snapshot: Some(f),
            }],
        });
        let items = inventory(&registry).unwrap();
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.extension.name, "remote-review");
        assert!(!item.extension.enabled);
        assert!(item.sync_excluded);
        assert_eq!(
            item.extension.source_path.as_deref(),
            Some("/remote/skills/remote-review/SKILL.md")
        );
        assert!(!item.extension.permissions.is_empty());
    }
}
#[derive(serde::Deserialize)]
pub struct ActionRequest {
    pub member: String,
    pub action: String,
    pub expected: Option<String>,
    pub enabled: Option<bool>,
}
pub fn action(home: &std::path::Path, req: ActionRequest) -> Result<serde_json::Value, HkError> {
    super::registry::Coordinator::new(home.join(".harnesskit/environments")).extension_action(
        &req.member,
        &req.action,
        req.expected.as_deref(),
        req.enabled,
    )
}
