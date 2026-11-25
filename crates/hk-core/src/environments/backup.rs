use super::{
    folders::Folder,
    registry::{Coordinator, Registry},
};
use crate::HkError;
use std::collections::HashSet;
fn valid_folder(folder: &Folder) -> bool {
    folder.files.len() <= 10000
        && folder.directories.len() <= 10000
        && folder.files.values().map(Vec::len).sum::<usize>() <= 64 * 1024 * 1024
        && folder
            .files
            .keys()
            .chain(folder.directories.iter())
            .all(|p| {
                !p.is_empty()
                    && !p.starts_with('/')
                    && !p.contains(['\\', ':'])
                    && !p.split('/').any(|v| v == ".." || v == ".")
            })
        && Folder::with_metadata(
            folder.files.clone(),
            folder.modes.clone(),
            folder.directories.clone(),
        )
        .fingerprint
            == folder.fingerprint
}
pub fn restore(c: &Coordinator, mut backup: Registry) -> Result<(), HkError> {
    if backup.version != 1 {
        return Err(HkError::Validation("Unsupported backup version".into()));
    }
    let mut env_ids = HashSet::new();
    let mut skill_ids = HashSet::new();
    let mut member_ids = HashSet::new();
    for e in &backup.environments {
        if !env_ids.insert(e.id.clone()) {
            return Err(HkError::Validation(
                "Duplicate environment in backup".into(),
            ));
        }
        let mut roots = HashSet::new();
        for p in &e.providers {
            if !roots.insert(&p.id) {
                return Err(HkError::Validation("Duplicate provider in backup".into()));
            }
        }
    }
    for s in &backup.skills {
        if !skill_ids.insert(&s.id)
            || s.name.is_empty()
            || s.name.contains(['/', '\\', ':'])
            || s.name == "."
            || s.name == ".."
        {
            return Err(HkError::Validation(
                "Invalid skill identity in backup".into(),
            ));
        }
        for m in &s.members {
            if !member_ids.insert(&m.id)
                || !backup.environments.iter().any(|e| {
                    e.id == m.environment
                        && e.providers.iter().any(|p| {
                            p.id == m.provider
                                && super::registry::installation_path(e, p, &s.name) == m.path
                        })
                })
            {
                return Err(HkError::Validation(
                    "Invalid installation mapping in backup".into(),
                ));
            }
        }
        for revision in &s.revisions {
            if revision.snapshot.as_ref().is_some_and(|f| !valid_folder(f)) {
                return Err(HkError::Validation(
                    "Invalid revision snapshot in backup".into(),
                ));
            }
        }
    }
    for job in &backup.jobs {
        if !valid_folder(&job.snapshot)
            || job.name.is_empty()
            || job.name.contains(['/', '\\', ':'])
            || job.name == "."
            || job.name == ".."
        {
            return Err(HkError::Validation(
                "Invalid queued install in backup".into(),
            ));
        }
    }
    backup.preferences.automatic = false;
    for env in &mut backup.environments {
        env.paused = true;
    }
    c.transaction(|current| {
        let history = c.root.join("backups");
        std::fs::create_dir_all(&history)?;
        let mut saved = tempfile::NamedTempFile::new_in(&history)?;
        serde_json::to_writer(&mut saved, &current)?;
        saved.as_file().sync_all()?;
        saved
            .persist(history.join(format!("before-restore-{}.json", uuid::Uuid::new_v4())))
            .map_err(|e| HkError::Internal(e.to_string()))?;
        backup.generation = current.generation;
        *current = backup;
        Ok(())
    })
}
pub fn prune(c: &Coordinator, skill: &str, keep: usize) -> Result<(), HkError> {
    if keep == 0 {
        return Err(HkError::Validation(
            "Keep at least one recent revision".into(),
        ));
    }
    c.transaction(|r| {
        let s = r
            .skills
            .iter_mut()
            .find(|s| s.id == skill)
            .ok_or_else(|| HkError::NotFound("Skill".into()))?;
        let protected: HashSet<_> = s
            .members
            .iter()
            .flat_map(|m| [m.fingerprint.clone(), m.previous.clone()])
            .flatten()
            .collect();
        let recent: HashSet<_> = s
            .revisions
            .iter()
            .rev()
            .take(keep)
            .map(|r| r.number)
            .collect();
        s.revisions.retain(|r| {
            recent.contains(&r.number)
                || r.snapshot
                    .as_ref()
                    .is_some_and(|f| protected.contains(&f.fingerprint))
        });
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::super::{registry::Environment, transport::Connection};
    use super::*;
    #[test]
    fn restore_archives_existing_registry_and_pauses_all_writes() {
        let temp = tempfile::tempdir().unwrap();
        let c = Coordinator::new(temp.path().join("state"));
        c.transaction(|r| {
            r.preferences.policy = "like".into();
            Ok(())
        })
        .unwrap();
        let mut backup = Registry {
            version: 1,
            ..Registry::default()
        };
        backup.environments.push(Environment {
            connection: Connection::Local,
            id: "local".into(),
            machine: "Desktop".into(),
            name: "Windows".into(),
            os: "windows".into(),
            paused: false,
            providers: vec![],
        });
        restore(&c, backup).unwrap();
        let view = c.overview().unwrap();
        assert!(!view.registry.preferences.automatic);
        assert!(view.registry.environments[0].paused);
        let file = std::fs::read_dir(c.root.join("backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let before: Registry = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        assert_eq!(before.preferences.policy, "like");
    }
    #[test]
    fn unsupported_backup_does_not_replace_existing_registry() {
        let temp = tempfile::tempdir().unwrap();
        let c = Coordinator::new(temp.path().join("state"));
        c.transaction(|r| {
            r.preferences.policy = "none".into();
            Ok(())
        })
        .unwrap();
        assert!(
            restore(
                &c,
                Registry {
                    version: 99,
                    ..Registry::default()
                }
            )
            .is_err()
        );
        assert_eq!(c.overview().unwrap().registry.preferences.policy, "none");
    }
}
