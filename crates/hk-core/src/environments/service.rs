use super::registry::*;
use super::transport::Connection;
use crate::{HkError, adapter::AgentAdapter, store::Store};
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    BrowsePaths {
        environment: String,
        path: String,
    },
    DiscoverProjects {
        environment: String,
        root: String,
    },
    RemovePath {
        environment: String,
        entry: String,
    },
    RemoveProject {
        environment: String,
        project: String,
    },
    RemoveAgent {
        environment: String,
        agent: String,
    },
    CatalogPath {
        environment: String,
        entry: String,
        expected: String,
        path: String,
    },
    CatalogEnabled {
        environment: String,
        entry: String,
        enabled: bool,
    },
    AgentHome {
        environment: String,
        agent: String,
        expected: String,
        path: String,
    },
    AgentEnabled {
        environment: String,
        agent: String,
        enabled: bool,
    },
    AddAgentProject {
        environment: String,
        agent: String,
        root: String,
    },
    ProjectEnabled {
        environment: String,
        project: String,
        enabled: bool,
    },
    ProjectPath {
        environment: String,
        project: String,
        expected: String,
        path: String,
    },
    CheckPaths {
        environment: String,
    },
    Overview,
    Refresh,
    Tick,
    Restore {
        registry: Registry,
    },
    Prune {
        skill: String,
        keep: usize,
    },
    MapProvider {
        environment: String,
        provider: String,
        root: String,
        scope: String,
    },
    Install {
        environment: String,
        provider: String,
        source: String,
        skill_id: String,
    },
    CancelInstall {
        job: String,
    },
    DeliverInstalls,
    CreateProviderDirectory {
        environment: String,
        provider: String,
        expected: String,
    },
    ProviderPath {
        environment: String,
        provider: String,
        expected: String,
        root: String,
    },
    ConfirmIdentity {
        skill: String,
    },
    Participation {
        skill: String,
        member: String,
        enabled: bool,
    },
    Revision {
        skill: String,
        member: String,
        number: usize,
        variant: Option<String>,
    },
    Edit {
        skill: String,
        member: String,
        file: String,
        expected: String,
        content: String,
    },
    Rename {
        environment: String,
        name: String,
    },
    ConnectPassword {
        machine: String,
        name: String,
        host: String,
        user: String,
        port: u16,
        password: String,
        #[serde(default)]
        host_key: String,
    },
    Connect {
        machine: String,
        name: String,
        connection: Connection,
    },
    Start {
        environment: String,
    },
    Preferences {
        preferences: Preferences,
    },
    Pause {
        environment: String,
        provider: Option<String>,
        paused: bool,
    },
    PauseMachine {
        machine: String,
        paused: bool,
    },
    Add {
        machine: String,
        name: String,
        os: String,
        provider: String,
        root: String,
    },
    Keep {
        skill: String,
        member: String,
        generation: u64,
    },
    Stop {
        skill: String,
        stopped: bool,
    },
    Ignore {
        skill: String,
        member: String,
        always: bool,
    },
}
pub(super) fn reconcile_probe_catalog(
    registry: &mut Registry,
    environment: &str,
    probe: &serde_json::Value,
) -> Result<(), HkError> {
    let Some(entries) = probe["catalog"].as_array() else {
        return Ok(());
    };
    let env = registry
        .environments
        .iter()
        .find(|e| e.id == environment)
        .ok_or_else(|| HkError::NotFound("Environment".into()))?;
    let mut catalog = Vec::new();
    let mut indices = std::collections::BTreeMap::<String, usize>::new();
    for entry in entries {
        let mut entry = entry.clone();
        entry["home_enabled"] = entry["enabled"].clone();
        entry["enabled"] = serde_json::json!(true);
        entry["environment"] = serde_json::json!(environment);
        let name = entry["name"].as_str().unwrap_or("").to_string();
        let index = indices.entry(name.clone()).or_default();
        let id = format!("{name}:{index}");
        *index += 1;
        entry["id"] = serde_json::json!(id);
        entry["provider"] = env
            .providers
            .iter()
            .find(|p| {
                entry["name"].as_str() == Some(&p.name) && entry["root"].as_str() == Some(&p.root)
            })
            .map(|p| serde_json::json!(p.id))
            .or_else(|| {
                registry
                    .catalog
                    .iter()
                    .find(|c| c.environment == environment && c.id == id)
                    .and_then(|c| c.provider.as_ref())
                    .map(|id| serde_json::json!(id))
            })
            .unwrap_or(serde_json::Value::Null);
        catalog.push(serde_json::from_value(entry)?);
    }
    registry
        .catalog
        .retain(|c| c.environment != environment || c.project.is_some());
    registry.catalog.extend(catalog);
    Ok(())
}
pub fn execute(
    home: &Path,
    adapters: &[Box<dyn AgentAdapter>],
    store: &Store,
    req: Request,
) -> Result<Overview, HkError> {
    let coordinator = Coordinator::new(home.join(".harnesskit/environments"));
    coordinator.transaction(|r| {
        r.path_homes
            .insert("local".into(), home.to_string_lossy().into_owned());
        if !r.environments.iter().any(|e| e.id == "local") {
            r.environments.push(Environment {
                connection: Connection::Local,
                id: "local".into(),
                machine: std::env::var("COMPUTERNAME")
                    .or_else(|_| std::env::var("HOSTNAME"))
                    .unwrap_or_else(|_| "This machine".into()),
                name: std::env::var("WSL_DISTRO_NAME")
                    .unwrap_or_else(|_| std::env::consts::OS.into()),
                os: std::env::consts::OS.into(),
                paused: false,
                providers: vec![],
            });
        }
        for adapter in adapters {
            let (custom, enabled) = store.get_agent_setting(adapter.name())?;
            for (index, default_root) in adapter.skill_dirs().into_iter().enumerate() {
                let id = format!("{}:{index}", adapter.name());
                if r.forgotten
                    .contains(&format!("agent:local/{}", adapter.name()))
                    || r.forgotten.contains(&format!("path:local/{id}"))
                {
                    continue;
                }
                let root = match (&custom, default_root.strip_prefix(adapter.base_dir())) {
                    (Some(custom), Ok(relative)) => PathBuf::from(custom).join(relative),
                    _ => default_root.clone(),
                };
                let root = super::catalog::provider_root(r, "local", &id, root);
                let root = r
                    .catalog_settings
                    .get(&super::nested::key("local", &id))
                    .and_then(|s| s.path.as_ref())
                    .map(PathBuf::from)
                    .unwrap_or(root);
                let local = r.environments.iter_mut().find(|e| e.id == "local").unwrap();
                if let Some(provider) = local
                    .providers
                    .iter_mut()
                    .find(|p| p.id == id && p.scope == "global")
                {
                    provider.disabled = !enabled;
                    let root = root.to_string_lossy().into_owned();
                    if root != provider.root {
                        provider.paused = true;
                        provider.root = root.clone();
                        for skill in &mut r.skills {
                            for member in &mut skill.members {
                                if member.environment == "local" && member.provider == id {
                                    member.path = Path::new(&root)
                                        .join(&skill.name)
                                        .to_string_lossy()
                                        .into_owned();
                                    member.previous = None;
                                    member.fingerprint = None;
                                    member.scanned_at = None;
                                    member.error = Some("Path changed; scan required".into());
                                }
                            }
                        }
                    }
                } else if root.is_dir()
                    && !r
                        .catalog_settings
                        .contains_key(&super::nested::key("local", &id))
                {
                    local.providers.push(Provider {
                        disabled: !enabled,
                        last_error: None,
                        last_scanned: None,
                        scope: "global".into(),
                        id,
                        name: adapter.name().into(),
                        root: root.to_string_lossy().into_owned(),
                        paused: false,
                    });
                }
            }
        }
        super::catalog::reconcile_local(r, home, adapters, store)?;
        if let Some((profile, machine)) = super::windows_host::detect(home) {
            super::windows_host::reconcile(
                r,
                &profile,
                &machine,
                home,
                adapters,
                super::windows_host::translate,
            )?;
        }
        if home == crate::home_dir() {
            let machine = r
                .environments
                .iter()
                .find(|e| e.id == "local")
                .map(|e| e.machine.clone())
                .unwrap_or_default();
            super::wsl_host::discover(r, &machine);
        }
        Ok(())
    })?;
    coordinator.transaction(|r| super::nested::reconcile(r, adapters, store))?;
    let check_after: Vec<String> = match &req {
        Request::AgentHome { environment, .. }
        | Request::ProjectPath { environment, .. }
        | Request::DiscoverProjects { environment, .. }
        | Request::AddAgentProject { environment, .. } => vec![environment.clone()],
        Request::Refresh => coordinator
            .overview()?
            .registry
            .environments
            .iter()
            .map(|e| e.id.clone())
            .collect(),
        _ => vec![],
    };
    match req {
        Request::CheckPaths { environment } => {
            coordinator.transaction(|r| super::nested::check(r, &environment))?
        }
        Request::CatalogPath {
            environment,
            entry,
            expected,
            path,
        } => coordinator.transaction(|r| {
            let mut path = super::nested::validate(r, &environment, &path)?;
            let c = r
                .catalog
                .iter()
                .find(|c| c.environment == environment && c.id == entry)
                .ok_or_else(|| HkError::NotFound("Path entry".into()))?;
            if c.root != expected {
                return Err(HkError::Conflict("Path changed; reload settings".into()));
            }
            if let Some(provider) = &c.provider {
                let env = r.environments.iter().find(|e| e.id == environment).unwrap();
                let mut next = env
                    .providers
                    .iter()
                    .find(|p| &p.id == provider)
                    .ok_or_else(|| HkError::NotFound("Provider".into()))?
                    .clone();
                if matches!(env.connection, Connection::Local) {
                    path = std::fs::canonicalize(&path)?.to_string_lossy().into_owned();
                }
                next.root = path.clone();
                super::registry::list_provider(env, &next)?;
            }
            r.catalog_settings
                .entry(super::nested::key(&environment, &entry))
                .or_default()
                .path = Some(path);
            super::nested::reconcile(r, adapters, store)?;
            super::nested::check(r, &environment)
        })?,
        Request::CatalogEnabled {
            environment,
            entry,
            enabled,
        } => coordinator.transaction(|r| {
            if !r
                .catalog
                .iter()
                .any(|c| c.environment == environment && c.id == entry)
            {
                return Err(HkError::NotFound("Path entry".into()));
            }
            r.catalog_settings
                .entry(super::nested::key(&environment, &entry))
                .or_default()
                .enabled = Some(enabled);
            Ok(())
        })?,
        Request::AgentHome {
            environment,
            agent,
            expected,
            path,
        } => coordinator.transaction(|r| {
            let path = super::nested::validate(r, &environment, &path)?;
            let c = r
                .catalog
                .iter()
                .find(|c| c.environment == environment && c.name == agent && c.project.is_none())
                .ok_or_else(|| HkError::NotFound("Agent".into()))?;
            if c.home != expected {
                return Err(HkError::Conflict("Home changed; reload settings".into()));
            }
            if environment == "local" {
                store.set_agent_path(&agent, Some(&path))?;
            }
            for c in r
                .catalog
                .iter()
                .filter(|c| c.environment == environment && c.name == agent && c.project.is_none())
            {
                r.catalog_settings
                    .entry(super::nested::key(&environment, &c.id))
                    .or_default();
            }
            r.catalog_settings
                .entry(super::nested::key(&environment, &format!("home:{agent}")))
                .or_default()
                .path = Some(path);
            Ok(())
        })?,
        Request::AgentEnabled {
            environment,
            agent,
            enabled,
        } => coordinator.transaction(|r| {
            if !r
                .catalog
                .iter()
                .any(|c| c.environment == environment && c.name == agent)
            {
                return Err(HkError::NotFound("Agent".into()));
            }
            if environment == "local" {
                store.set_agent_enabled(&agent, enabled)?;
            }
            r.catalog_settings
                .entry(super::nested::key(&environment, &format!("home:{agent}")))
                .or_default()
                .enabled = Some(enabled);
            Ok(())
        })?,
        Request::AddAgentProject {
            environment,
            agent,
            root,
        } => coordinator.transaction(|r| {
            super::discovery::discover(r, &environment, Some(&agent), &root, adapters)
        })?,
        Request::DiscoverProjects { environment, root } => coordinator
            .transaction(|r| super::discovery::discover(r, &environment, None, &root, adapters))?,
        Request::BrowsePaths { environment, path } => {
            let mut view = coordinator.overview()?;
            view.browse = Some(super::discovery::browse(
                &view.registry,
                &environment,
                &path,
            )?);
            return Ok(view);
        }
        Request::RemovePath { environment, entry } => coordinator
            .transaction(|r| super::discovery::remove(r, &environment, Some(&entry), None, None))?,
        Request::RemoveProject {
            environment,
            project,
        } => coordinator.transaction(|r| {
            super::discovery::remove(r, &environment, None, Some(&project), None)
        })?,
        Request::RemoveAgent { environment, agent } => coordinator
            .transaction(|r| super::discovery::remove(r, &environment, None, None, Some(&agent)))?,
        Request::ProjectEnabled {
            environment,
            project,
            enabled,
        } => coordinator.transaction(|r| {
            let p = r
                .projects
                .iter_mut()
                .find(|p| p.environment == environment && p.id == project)
                .ok_or_else(|| HkError::NotFound("Project".into()))?;
            p.enabled = enabled;
            Ok(())
        })?,
        Request::ProjectPath {
            environment,
            project,
            expected,
            path,
        } => coordinator.transaction(|r| {
            let path = super::nested::validate(r, &environment, &path)?;
            let p = r
                .projects
                .iter_mut()
                .find(|p| p.environment == environment && p.id == project)
                .ok_or_else(|| HkError::NotFound("Project".into()))?;
            if p.root != expected {
                return Err(HkError::Conflict(
                    "Project path changed; reload settings".into(),
                ));
            }
            p.root = path;
            Ok(())
        })?,
        Request::Restore { registry } => super::backup::restore(&coordinator, registry)?,
        Request::Prune { skill, keep } => super::backup::prune(&coordinator, &skill, keep)?,
        Request::MapProvider {
            environment,
            provider,
            root,
            scope,
        } => coordinator.map_provider(&environment, &provider, &root, &scope)?,
        Request::Install {
            environment,
            provider,
            source,
            skill_id,
        } => {
            super::installs::prepare(&coordinator, &environment, &provider, &source, &skill_id)?;
            super::installs::deliver(&coordinator)?;
            coordinator.refresh()?;
        }
        Request::CancelInstall { job } => coordinator.transaction(|r| {
            let job = r
                .jobs
                .iter_mut()
                .find(|j| j.id == job && j.status == "pending")
                .ok_or_else(|| HkError::NotFound("Pending install".into()))?;
            job.status = "cancelled".into();
            Ok(())
        })?,
        Request::DeliverInstalls => {
            super::installs::deliver(&coordinator)?;
            coordinator.refresh()?;
        }
        Request::Participation {
            skill,
            member,
            enabled,
        } => coordinator.transaction(|r| {
            let s = r
                .skills
                .iter_mut()
                .find(|s| s.id == skill)
                .ok_or_else(|| HkError::NotFound("Skill".into()))?;
            let m = s
                .members
                .iter_mut()
                .find(|m| m.id == member)
                .ok_or_else(|| HkError::NotFound("Installation".into()))?;
            m.excluded = !enabled;
            Ok(())
        })?,
        Request::CreateProviderDirectory {
            environment,
            provider,
            expected,
        } => {
            coordinator.create_provider_directory(&environment, &provider, &expected)?;
            coordinator.refresh()?;
        }
        Request::ProviderPath {
            environment,
            provider,
            expected,
            root,
        } => coordinator.update_provider_path(&environment, &provider, &expected, &root)?,
        Request::Revision {
            skill,
            member,
            number,
            variant,
        } => coordinator.revision_action(&skill, &member, number, variant.as_deref())?,
        Request::ConfirmIdentity { skill } => coordinator.transaction(|r| {
            r.skills
                .iter_mut()
                .find(|s| s.id == skill)
                .ok_or_else(|| HkError::NotFound("Skill".into()))?
                .identity_confirmed = true;
            Ok(())
        })?,
        Request::Overview => {}
        Request::Refresh => {
            let environments = coordinator.overview()?.registry.environments;
            for env in environments
                .iter()
                .filter(|e| !matches!(e.connection, Connection::Local))
            {
                // A disconnected/stopped target retains its last useful catalog. Refresh
                // updates discovery metadata only; enrollment remains an explicit choice.
                if let Ok(probe) =
                    super::transport::run(&env.connection, serde_json::json!({"action":"probe"}))
                {
                    if let Err(error) =
                        coordinator.transaction(|r| reconcile_probe_catalog(r, &env.id, &probe))
                    {
                        eprintln!("Environment catalog ({}): {error}", env.name);
                    }
                }
            }
            coordinator.refresh()?;
        }
        Request::Edit {
            skill,
            member,
            file,
            expected,
            content,
        } => coordinator.edit(&skill, &member, &file, &expected, &content)?,
        Request::Tick => {
            super::installs::deliver(&coordinator)?;
            coordinator.refresh()?;
            let view = coordinator.overview()?;
            if view.registry.preferences.automatic {
                for status in view.statuses {
                    if let super::reconcile::Decision::Change { source, .. } = status.decision {
                        let current = coordinator.overview()?;
                        coordinator.keep(&status.id, &source, current.registry.generation)?;
                    }
                }
            }
        }
        Request::Rename { environment, name } => {
            rename_environment(&coordinator, &environment, &name)?;
        }
        Request::ConnectPassword {
            machine,
            name,
            host,
            user,
            port,
            password,
            host_key,
        } => {
            if machine.trim().is_empty() || name.trim().is_empty() {
                return Err(HkError::Validation(
                    "Machine and environment names required".into(),
                ));
            }
            if coordinator.overview()?.registry.environments.iter().any(|e| matches!(&e.connection, Connection::SshPassword {host:h,user:u,port:p,..} if h==&host && u==&user && *p==port)) {
                return Err(HkError::Conflict("Connection already registered".into()));
            }
            let connection = super::ssh_credentials::connect(host, user, port, password, host_key)?;
            let credential = match &connection {
                Connection::SshPassword { credential, .. } => credential.clone(),
                _ => unreachable!(),
            };
            let result = execute(
                home,
                adapters,
                store,
                Request::Connect {
                    machine,
                    name,
                    connection,
                },
            );
            if result.is_err() {
                super::ssh_credentials::forget(&credential);
            }
            return result;
        }
        Request::Connect {
            machine,
            name,
            connection,
        } => {
            if machine.trim().is_empty() || name.trim().is_empty() {
                return Err(HkError::Validation(
                    "Machine and environment names required".into(),
                ));
            }
            let connection = super::transport::normalize_connection(connection)?;
            let stopped = match &connection {
                Connection::Wsl { distribution } => !super::transport::wsl_running(distribution)?,
                _ => false,
            };
            let probe = if stopped {
                serde_json::json!({"os":"linux","providers":[]})
            } else {
                super::transport::run(&connection, serde_json::json!({"action":"probe"}))?
            };
            let roots: Vec<(String, String)> = serde_json::from_value(probe["providers"].clone())?;
            coordinator.transaction(|r| {
                let serialized = serde_json::to_value(&connection)?;
                if r.environments
                    .iter()
                    .any(|e| {
                        matches!((&connection, &e.connection), (Connection::Docker { container: a, .. }, Connection::Docker { container: b, .. }) if a == b)
                            || serde_json::to_value(&e.connection).is_ok_and(|v| v == serialized)
                    })
                {
                    return Err(HkError::Conflict("Connection already registered".into()));
                }
                let id = uuid::Uuid::new_v4().to_string();
                r.environments.push(Environment {
                    connection,
                    id: id.clone(),
                    machine,
                    name,
                    os: probe["os"].as_str().unwrap_or("unknown").into(),
                    paused: true,
                    providers: roots
                        .into_iter()
                        .enumerate()
                        .map(|(index, (name, root))| Provider {
                            disabled: probe["disabled_providers"].as_array().is_some_and(|items| {
                                items
                                    .iter()
                                    .any(|item| item.as_str() == Some(name.as_str()))
                            }),
                            last_error: None,
                            last_scanned: None,
                            scope: "global".into(),
                            id: format!("{name}:{index}"),
                            name,
                            root,
                            paused: false,
                        })
                        .collect(),
                });
                reconcile_probe_catalog(r, &id, &probe)?;
                Ok(())
            })?;
        }
        Request::Start { environment } => {
            let v = coordinator.overview()?;
            let env = v
                .registry
                .environments
                .iter()
                .find(|e| e.id == environment)
                .ok_or_else(|| HkError::NotFound("Environment".into()))?;
            if let Connection::Wsl { distribution } = &env.connection {
                let out = crate::command("wsl.exe")
                    .args(["--distribution", distribution, "--exec", "true"])
                    .output()?;
                if !out.status.success() {
                    return Err(HkError::CommandFailed(
                        String::from_utf8_lossy(&out.stderr).into_owned(),
                    ));
                }
                {
                    let probe = super::transport::run(
                        &env.connection,
                        serde_json::json!({"action":"probe"}),
                    )?;
                    let roots: Vec<(String, String)> =
                        serde_json::from_value(probe["providers"].clone())?;
                    coordinator.transaction(|r| {
                        let e = r
                            .environments
                            .iter_mut()
                            .find(|e| e.id == environment)
                            .unwrap();
                        if e.providers.is_empty() {
                            e.providers = roots
                                .into_iter()
                                .enumerate()
                                .map(|(index, (name, root))| Provider {
                                    disabled: probe["disabled_providers"].as_array().is_some_and(
                                        |items| {
                                            items
                                                .iter()
                                                .any(|item| item.as_str() == Some(name.as_str()))
                                        },
                                    ),
                                    last_error: None,
                                    last_scanned: None,
                                    scope: "global".into(),
                                    id: format!("{name}:{index}"),
                                    name,
                                    root,
                                    paused: false,
                                })
                                .collect();
                        }
                        reconcile_probe_catalog(r, &environment, &probe)?;
                        Ok(())
                    })?;
                }
                coordinator.refresh()?;
            } else {
                return Err(HkError::Validation(
                    "Only WSL environments can be started".into(),
                ));
            }
        }
        Request::Preferences { preferences } => coordinator.transaction(|r| {
            if !["all", "like", "none"].contains(&preferences.policy.as_str())
                || !["bidirectional", "host"].contains(&preferences.direction.as_str())
                || !["propagate", "ask", "ignore"].contains(&preferences.removal.as_str())
            {
                return Err(HkError::Validation("Invalid sync preferences".into()));
            }
            if preferences.direction == "host"
                && preferences
                    .host
                    .as_ref()
                    .is_none_or(|id| !r.environments.iter().any(|e| e.id == *id))
            {
                return Err(HkError::Validation("Select a host environment".into()));
            }
            r.preferences = preferences;
            Ok(())
        })?,
        Request::PauseMachine { machine, paused } => coordinator.transaction(|r| {
            for env in &mut r.environments {
                if env.machine == machine {
                    env.paused = paused;
                }
            }
            Ok(())
        })?,
        Request::Pause {
            environment,
            provider,
            paused,
        } => coordinator.transaction(|r| {
            let env = r
                .environments
                .iter_mut()
                .find(|e| e.id == environment)
                .ok_or_else(|| HkError::NotFound("Environment".into()))?;
            if let Some(id) = provider {
                env.providers
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or_else(|| HkError::NotFound("Provider".into()))?
                    .paused = paused;
            } else {
                env.paused = paused;
            }
            Ok(())
        })?,
        Request::Add {
            machine,
            name,
            os,
            provider,
            root,
        } => coordinator.transaction(|r| {
            if machine.trim().is_empty() || name.trim().is_empty() || provider.trim().is_empty() {
                return Err(HkError::Validation(
                    "Machine, environment and provider are required".into(),
                ));
            }
            let root = super::paths::local(&root);
            if !root.is_absolute() || !root.is_dir() {
                return Err(HkError::Validation(
                    "Choose an accessible absolute skills directory".into(),
                ));
            }
            let canonical = std::fs::canonicalize(root)?;
            if r.environments
                .iter()
                .flat_map(|e| &e.providers)
                .any(|p| std::fs::canonicalize(&p.root).is_ok_and(|p| p == canonical))
            {
                return Err(HkError::Conflict("Directory already registered".into()));
            }
            r.environments.push(Environment {
                connection: Connection::Local,
                id: uuid::Uuid::new_v4().to_string(),
                machine,
                name,
                os,
                paused: true,
                providers: vec![Provider {
                    disabled: false,
                    last_error: None,
                    last_scanned: None,
                    scope: "global".into(),
                    id: format!("{provider}:0"),
                    name: provider,
                    root: canonical.to_string_lossy().into_owned(),
                    paused: false,
                }],
            });
            Ok(())
        })?,
        Request::Keep {
            skill,
            member,
            generation,
        } => coordinator.keep(&skill, &member, generation)?,
        Request::Stop { skill, stopped } => coordinator.transaction(|r| {
            r.skills
                .iter_mut()
                .find(|s| s.id == skill)
                .ok_or_else(|| HkError::NotFound("Skill".into()))?
                .stopped = stopped;
            Ok(())
        })?,
        Request::Ignore {
            skill,
            member,
            always,
        } => coordinator.transaction(|r| {
            let s = r
                .skills
                .iter_mut()
                .find(|s| s.id == skill)
                .ok_or_else(|| HkError::NotFound("Skill".into()))?;
            let m = s
                .members
                .iter_mut()
                .find(|m| m.id == member)
                .ok_or_else(|| HkError::NotFound("Installation".into()))?;
            if m.fingerprint.is_some() || m.error.is_some() {
                return Err(HkError::Validation(
                    "Only a confirmed external removal can be ignored".into(),
                ));
            }
            m.excluded = true;
            if always {
                r.preferences.removal = "ignore".into();
            }
            Ok(())
        })?,
    }
    coordinator.transaction(|r| {
        super::catalog::reconcile_local(r, home, adapters, store)?;
        super::nested::reconcile(r, adapters, store)
    })?;
    for environment in check_after {
        coordinator.transaction(|r| super::nested::check(r, &environment))?;
    }
    coordinator.overview()
}

/// Keeps the coordinator alive independently of the UI. The owning process must stay running.
pub fn start(home: std::path::PathBuf, adapters: std::sync::Arc<Vec<Box<dyn AgentAdapter>>>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            if let Ok(store) = Store::open(&home.join(".harnesskit/metadata.db")) {
                if let Err(error) = execute(&home, &adapters, &store, Request::Tick) {
                    eprintln!("Environment sync: {error}");
                }
            }
        }
    });
}

#[cfg(test)]
mod provider_tests {
    use super::*;
    #[test]
    fn new_mounted_connection_waits_for_sync_review() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let root = temp.path().join("skills");
        std::fs::create_dir_all(&root).unwrap();
        let store = Store::open(&home.join("test.db")).unwrap();
        let view = execute(
            &home,
            &[],
            &store,
            Request::Add {
                machine: "Mounted machine".into(),
                name: "Mounted files".into(),
                os: "linux".into(),
                provider: "codex".into(),
                root: root.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        assert!(
            view.registry
                .environments
                .iter()
                .find(|e| e.name == "Mounted files")
                .unwrap()
                .paused
        );
    }
    #[test]
    fn custom_paths_and_new_provider_directories_are_reconciled_after_startup() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        let store = Store::open(&home.join("test.db")).unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(
            crate::adapter::codex::CodexAdapter::with_home(home.into()),
        )];
        execute(home, &adapters, &store, Request::Overview).unwrap();
        let custom = home.join("custom");
        std::fs::create_dir_all(custom.join("skills")).unwrap();
        store
            .set_agent_path("codex", Some(custom.to_str().unwrap()))
            .unwrap();
        let view = execute(home, &adapters, &store, Request::Overview).unwrap();
        assert!(
            view.registry.environments[0]
                .providers
                .iter()
                .any(|p| p.root == custom.join("skills").to_string_lossy())
        );
        let second = home.join("second");
        std::fs::create_dir_all(second.join("skills")).unwrap();
        store
            .set_agent_path("codex", Some(second.to_str().unwrap()))
            .unwrap();
        let view = execute(home, &adapters, &store, Request::Overview).unwrap();
        assert!(
            view.registry.environments[0]
                .providers
                .iter()
                .any(|p| p.root == second.join("skills").to_string_lossy())
        );
        store.set_agent_enabled("codex", false).unwrap();
        execute(home, &adapters, &store, Request::Overview).unwrap();
        store.set_agent_enabled("codex", true).unwrap();
        let view = execute(home, &adapters, &store, Request::Overview).unwrap();
        // Relocating an enrolled root requires an explicit resume after review.
        assert!(view.registry.environments[0].providers[0].paused);
    }
}

fn rename_environment(
    coordinator: &Coordinator,
    environment: &str,
    name: &str,
) -> Result<(), HkError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
        return Err(HkError::Validation(
            "Use an environment name of 1–120 characters".into(),
        ));
    }
    coordinator.transaction(|r| {
        let env = r
            .environments
            .iter_mut()
            .find(|e| e.id == environment)
            .ok_or_else(|| HkError::NotFound("Environment".into()))?;
        env.name = name.into();
        for file in &mut r.managed_files {
            for copy in &mut file.copies {
                if copy.environment_id == environment {
                    copy.environment = name.into();
                }
            }
        }
        Ok(())
    })
}
#[cfg(test)]
mod connection_edit_tests {
    use super::*;
    #[test]
    fn rename_preserves_environment_identity_and_connection() {
        let tmp = tempfile::tempdir().unwrap();
        let c = Coordinator::new(tmp.path().to_owned());
        c.transaction(|r| {
            r.environments.push(Environment {
                id: "remote-id".into(),
                machine: "MacBook".into(),
                name: "old".into(),
                os: "macos".into(),
                paused: true,
                providers: vec![],
                connection: Connection::Ssh {
                    host: "mac".into(),
                    user: "demonlord".into(),
                    port: 22,
                },
            });
            Ok(())
        })
        .unwrap();
        rename_environment(&c, "remote-id", "  MacBook Air  ").unwrap();
        let env = c.overview().unwrap().registry.environments.remove(0);
        assert_eq!(env.id, "remote-id");
        assert_eq!(env.name, "MacBook Air");
        assert_eq!(env.machine, "MacBook");
        assert!(env.paused);
        assert!(matches!(env.connection, Connection::Ssh { .. }));
        assert!(rename_environment(&c, "remote-id", " ").is_err());
        assert!(rename_environment(&c, "missing", "Name").is_err());
    }
}
