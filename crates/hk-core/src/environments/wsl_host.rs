//! Windows-side discovery mirrors the WSL-side Windows profile discovery.
use super::{
    registry::{Environment, Provider, Registry},
    transport::Connection,
};
use crate::HkError;

pub fn register(
    registry: &mut Registry,
    machine: &str,
    distributions: &[(String, bool)],
    mut probe: impl FnMut(&str) -> Result<Vec<(String, String, bool)>, HkError>,
) {
    for (name, running) in distributions {
        let existing = registry.environments.iter().position(
            |e| matches!(&e.connection,Connection::Wsl{distribution} if distribution==name),
        );
        let index = existing.unwrap_or_else(|| {
            registry.environments.push(Environment {
                id: format!("wsl:{name}"),
                machine: machine.into(),
                name: name.clone(),
                os: "linux".into(),
                paused: true,
                connection: Connection::Wsl {
                    distribution: name.clone(),
                },
                providers: vec![],
            });
            registry.environments.len() - 1
        });
        if !running {
            continue;
        }
        let roots = match probe(name) {
            Ok(roots) => roots,
            Err(e) => {
                eprintln!("WSL discovery ({name}): {e}");
                continue;
            }
        };
        let env = &mut registry.environments[index];
        for (index, (provider, root, disabled)) in roots.into_iter().enumerate() {
            if let Some(existing) = env
                .providers
                .iter_mut()
                .find(|p| p.name == provider && p.root == root)
            {
                existing.disabled = disabled;
                continue;
            }
            env.providers.push(Provider {
                id: format!("{provider}:{index}:{}", super::digest(root.as_bytes())),
                name: provider,
                root,
                scope: "global".into(),
                paused: false,
                disabled,
                last_error: None,
                last_scanned: None,
            });
        }
    }
}

pub fn discover(registry: &mut Registry, machine: &str) {
    if !cfg!(windows) || std::env::var_os("HARNESSKIT_HOME").is_some() {
        return;
    }
    // Revisit on explicit overview/refresh, bounded to once per minute.
    static LAST: std::sync::LazyLock<parking_lot::Mutex<Option<std::time::Instant>>> =
        std::sync::LazyLock::new(|| parking_lot::Mutex::new(None));
    let mut last = LAST.lock();
    if last.is_some_and(|t| t.elapsed().as_secs() < 60) {
        return;
    }
    *last = Some(std::time::Instant::now());
    let Ok(all) = crate::command("wsl.exe")
        .args(["--list", "--quiet"])
        .output()
    else {
        return;
    };
    if !all.status.success() {
        return;
    }
    let Ok(running) = crate::command("wsl.exe")
        .args(["--list", "--running", "--quiet"])
        .output()
    else {
        return;
    };
    if !running.status.success() {
        return;
    }
    let running = super::transport::decode_wsl(&running.stdout);
    let distributions = super::transport::decode_wsl(&all.stdout)
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| (n.to_owned(), running.lines().any(|r| r.trim() == n)))
        .collect::<Vec<_>>();
    let mut probes = Vec::new();
    register(registry, machine, &distributions, |name| {
        let value = super::transport::run(
            &Connection::Wsl {
                distribution: name.into(),
            },
            serde_json::json!({"action":"probe"}),
        )?;
        let roots: Vec<(String, String)> = serde_json::from_value(value["providers"].clone())?;
        probes.push((name.to_owned(), value.clone()));
        Ok(roots
            .into_iter()
            .map(|(name, root)| {
                let disabled = value["disabled_providers"].as_array().is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.as_str() == Some(name.as_str()))
                });
                (name, root, disabled)
            })
            .collect())
    });
    for (distribution, probe) in probes {
        if let Some(id) = registry.environments.iter().find(|e| matches!(&e.connection, Connection::Wsl { distribution: d } if *d == distribution)).map(|e| e.id.clone()) {
            if let Err(error) = super::service::reconcile_probe_catalog(registry, &id, &probe) {
                eprintln!("WSL catalog ({distribution}): {error}");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_keeps_stopped_distros_stopped_and_preserves_existing_settings() {
        let mut registry = Registry::default();
        let mut calls = vec![];
        let distributions = vec![("Ubuntu".into(), true), ("Debian".into(), false)];
        register(&mut registry, "DESKTOP", &distributions, |name| {
            calls.push(name.to_owned());
            Ok(vec![(
                "codex".into(),
                "/home/user/.codex/skills".into(),
                false,
            )])
        });
        super::super::service::reconcile_probe_catalog(&mut registry, "wsl:Ubuntu", &serde_json::json!({"catalog":[
            {"name":"codex","home":"/home/user/.codex","root":"/home/user/.codex/skills","detected":true,"enabled":true,"error":null},
            {"name":"gemini","home":"/home/user/.gemini","root":"/home/user/.gemini/skills","detected":false,"enabled":true,"error":null}
        ]})).unwrap();
        assert!(registry.catalog[0].provider.is_some());
        assert!(registry.catalog[1].provider.is_none());
        assert_eq!(registry.environments[0].providers.len(), 1);
        assert_eq!(calls, vec!["Ubuntu"]);
        assert_eq!(registry.environments.len(), 2);
        assert!(registry.environments.iter().all(|e| e.paused));
        registry.environments[0].paused = false;
        register(&mut registry, "DESKTOP", &distributions, |_| {
            Ok(vec![(
                "codex".into(),
                "/home/user/.codex/skills".into(),
                false,
            )])
        });
        assert_eq!(registry.environments.len(), 2);
        assert_eq!(registry.environments[0].providers.len(), 1);
        assert!(!registry.environments[0].paused);
    }
}
