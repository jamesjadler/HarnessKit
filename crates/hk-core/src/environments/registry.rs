use super::{
    folders::{Folder, read_folder, replace_folder},
    reconcile::{Decision, Observation, reconcile},
};
use crate::HkError;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Environment {
    #[serde(default)]
    pub connection: super::transport::Connection,
    pub id: String,
    pub machine: String,
    pub name: String,
    pub os: String,
    pub paused: bool,
    pub providers: Vec<Provider>,
}
fn global_scope() -> String {
    "global".into()
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Provider {
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_scanned: Option<String>,
    #[serde(default = "global_scope")]
    pub scope: String,
    pub id: String,
    pub name: String,
    pub root: String,
    pub paused: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Member {
    #[serde(default)]
    pub provenance: Option<String>,
    pub id: String,
    pub environment: String,
    pub provider: String,
    pub path: String,
    pub excluded: bool,
    pub fingerprint: Option<String>,
    pub previous: Option<String>,
    pub error: Option<String>,
    pub scanned_at: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Revision {
    #[serde(default)]
    pub path: Option<String>,
    pub number: usize,
    pub environment: String,
    pub captured_at: String,
    pub snapshot: Option<Folder>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Skill {
    #[serde(default)]
    pub identity_confirmed: bool,
    pub id: String,
    pub name: String,
    pub scope: String,
    pub original: Option<String>,
    pub stopped: bool,
    pub members: Vec<Member>,
    pub revisions: Vec<Revision>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub automatic: bool,
    pub policy: String,
    pub direction: String,
    pub host: Option<String>,
    pub removal: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            automatic: true,
            policy: "all".into(),
            direction: "bidirectional".into(),
            host: None,
            removal: "propagate".into(),
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub path_homes: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub forgotten: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub catalog_settings: std::collections::BTreeMap<String, super::nested::EntrySetting>,
    #[serde(default)]
    pub projects: Vec<super::nested::Project>,
    #[serde(default)]
    pub provider_path_overrides: std::collections::BTreeMap<String, super::catalog::PathOverride>,
    #[serde(default)]
    pub catalog: Vec<super::catalog::CatalogEntry>,
    #[serde(default)]
    pub jobs: Vec<super::installs::InstallJob>,
    pub version: u32,
    pub generation: u64,
    pub environments: Vec<Environment>,
    pub skills: Vec<Skill>,
    pub preferences: Preferences,
}
#[derive(Serialize)]
pub struct SkillStatus {
    pub id: String,
    pub decision: Decision,
}
#[derive(Serialize)]
pub struct Overview {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browse: Option<super::discovery::Browse>,
    pub storage_bytes: u64,
    pub registry: Registry,
    pub statuses: Vec<SkillStatus>,
    pub inventory: Vec<super::inventory::Entry>,
}
pub struct Coordinator {
    pub(super) root: PathBuf,
}
impl Coordinator {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn transaction<T>(
        &self,
        run: impl FnOnce(&mut Registry) -> Result<T, HkError>,
    ) -> Result<T, HkError> {
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("coordinator.lock"))?;
        lock.lock_exclusive()?;
        let path = self.root.join("registry.json");
        let mut registry = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Registry {
                version: 1,
                ..Registry::default()
            },
            Err(e) => return Err(e.into()),
        };
        if registry.version != 1 {
            return Err(HkError::Validation(
                "Unsupported coordinator version".into(),
            ));
        }
        let before = serde_json::to_vec(&registry)?;
        let result = run(&mut registry)?;
        if serde_json::to_vec(&registry)? == before {
            return Ok(result);
        }
        registry.generation += 1;
        let mut staged = tempfile::NamedTempFile::new_in(&self.root)?;
        serde_json::to_writer(&mut staged, &registry)?;
        staged.as_file().sync_all()?;
        staged
            .persist(path)
            .map_err(|e| HkError::Internal(e.to_string()))?;
        Ok(result)
    }
    pub fn extension_action(
        &self,
        member: &str,
        action: &str,
        expected: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<serde_json::Value, HkError> {
        self.transaction(|r|{
  let skill=r.skills.iter().find(|s|s.members.iter().any(|m|m.id==member)).cloned().ok_or_else(||HkError::NotFound("Installation".into()))?;
  let m=skill.members.iter().find(|m|m.id==member).unwrap();
  let env=r.environments.iter().find(|e|e.id==m.environment).ok_or_else(||HkError::NotFound("Environment".into()))?;
  let provider=env.providers.iter().find(|p|p.id==m.provider).ok_or_else(||HkError::NotFound("Provider".into()))?;
  if action=="content"{
   let snapshot=skill.revisions.iter().rev().find_map(|rev|rev.snapshot.as_ref().filter(|f|Some(&f.fingerprint)==m.fingerprint.as_ref())).ok_or_else(||HkError::NotFound("Cached snapshot".into()))?;
   let bytes=snapshot.files.get("SKILL.md").or_else(||snapshot.files.get("SKILL.md.disabled")).ok_or_else(||HkError::NotFound("Skill content".into()))?;
   let content=String::from_utf8(bytes.clone()).map_err(|_|HkError::Validation("Skill file is not UTF-8".into()))?;
   return Ok(serde_json::json!({"content":content,"path":format!("{}/SKILL.md",m.path),"symlink_target":null}));
  }
  if !["toggle","delete"].contains(&action){return Err(HkError::Validation("Unsupported environment action".into()));}
  if m.error.is_some(){return Err(HkError::Validation("Environment unavailable; refresh before changing files".into()));}
  if expected.is_none()||expected!=m.fingerprint.as_deref(){return Err(HkError::Conflict("Installation changed; refresh before changing files".into()));}
  let old=read_installation(env,provider,&skill.name,&m.path)?.ok_or_else(||HkError::NotFound("Skill folder".into()))?;
  if Some(old.fingerprint.as_str())!=expected{return Err(HkError::Conflict("Files changed since the last scan".into()));}
  let next=if action=="delete"{None}else{
   let on=enabled.ok_or_else(||HkError::Validation("Enabled state required".into()))?;
   let(from,to)=if on{("SKILL.md.disabled","SKILL.md")}else{("SKILL.md","SKILL.md.disabled")};
   let mut files=old.files.clone();let mut modes=old.modes.clone();
   if let Some(bytes)=files.remove(from){if files.contains_key(to){return Err(HkError::Conflict("Both enabled and disabled skill files exist".into()));}files.insert(to.into(),bytes);if let Some(mode)=modes.remove(from){modes.insert(to.into(),mode);}}
   Some(Folder::with_metadata(files,modes,old.directories.clone()))
  };
  if matches!(env.connection,super::transport::Connection::Local){replace_folder(&local_installation_path(provider,&skill.name,&m.path)?,expected,next.as_ref(),&self.root.join("history"))?;}else{super::transport::run(&env.connection,serde_json::json!({"action":"write","root":provider.root,"name":skill.name,"expected":expected,"folder":next}))?;}
  let updated=r.skills.iter_mut().find(|s|s.id==skill.id).unwrap();
  updated.members.iter_mut().find(|m|m.id==member).unwrap().fingerprint=next.as_ref().map(|f|f.fingerprint.clone());
  updated.revisions.push(Revision{path:Some(m.path.clone()),number:updated.revisions.last().map(|r|r.number+1).unwrap_or(1),environment:m.environment.clone(),captured_at:chrono::Utc::now().to_rfc3339(),snapshot:next});
  Ok(serde_json::json!({"redacted_secret_keys":[]}))
 })
    }
    /// Explicit recovery only for a never-observed empty provider. Never restore lost data as empty.
    pub fn create_provider_directory(
        &self,
        environment: &str,
        provider: &str,
        expected: &str,
    ) -> Result<(), HkError> {
        self.transaction(|r| {
            let env = r.environments.iter().find(|e| e.id == environment).ok_or_else(|| HkError::NotFound("Environment".into()))?;
            let p = env.providers.iter().find(|p| p.id == provider).ok_or_else(|| HkError::NotFound("Provider".into()))?;
            if p.root != expected { return Err(HkError::Conflict("Provider path changed; reload settings".into())); }
            if p.last_scanned.is_some() || r.skills.iter().any(|s| s.members.iter().any(|m| m.environment == environment && m.provider == provider && (m.fingerprint.is_some() || m.previous.is_some() || m.scanned_at.is_some() || s.revisions.iter().any(|rev| rev.environment == environment && rev.path.as_deref() == Some(m.path.as_str()))))) || r.jobs.iter().any(|j| j.environment == environment && j.provider == provider) {
                return Err(HkError::Validation("Previously tracked directory is missing; restore its data or choose another path".into()));
            }
            if !matches!(env.connection, super::transport::Connection::Local) {
                return Err(HkError::Validation("Create the directory on its host, then refresh".into()));
            }
            let root = Path::new(&p.root);
            if !root.is_absolute() || root.file_name().is_none() || root.symlink_metadata().is_ok() {
                return Err(HkError::Validation("Choose a missing absolute skills directory".into()));
            }
            let parent = root.parent().ok_or_else(|| HkError::Validation("No parent directory".into()))?;
            let canonical = fs::canonicalize(parent)?;
            if !canonical.is_dir() || fs::metadata(&canonical)?.permissions().readonly() {
                return Err(HkError::Validation("Parent directory is not writable".into()));
            }
            // create_dir is deliberately non-recursive and fails on races or existing entries.
            fs::create_dir(canonical.join(root.file_name().unwrap()))?;
            Ok(())
        })
    }
    pub fn map_provider(
        &self,
        environment: &str,
        name: &str,
        root: &str,
        scope: &str,
    ) -> Result<(), HkError> {
        self.transaction(|r| {
            if name.trim().is_empty() || scope.trim().is_empty() {
                return Err(HkError::Validation("Provider and scope required".into()));
            }
            let env = r
                .environments
                .iter()
                .find(|e| e.id == environment)
                .ok_or_else(|| HkError::NotFound("Environment".into()))?;
            let root = super::paths::for_connection(root, &env.connection);
            let root = root.as_str();
            if !root.starts_with('/') && !(root.len() > 2 && root.as_bytes()[1] == b':') {
                return Err(HkError::Validation(
                    "Use an absolute skills directory".into(),
                ));
            }
            if env.providers.iter().any(|p| {
                p.root == root
                    || (matches!(env.connection, super::transport::Connection::Local)
                        && crate::paths_equal(Path::new(&p.root), Path::new(root)))
            }) {
                return Err(HkError::Conflict(
                    "Directory already mapped in this environment".into(),
                ));
            }
            let provider = Provider {
                disabled: false,
                last_error: None,
                last_scanned: None,
                id: uuid::Uuid::new_v4().to_string(),
                name: name.into(),
                root: root.into(),
                scope: scope.into(),
                paused: true,
            };
            list_provider(env, &provider)?;
            r.environments
                .iter_mut()
                .find(|e| e.id == environment)
                .unwrap()
                .providers
                .push(provider);
            Ok(())
        })
    }
    pub fn update_provider_path(
        &self,
        environment: &str,
        provider: &str,
        expected: &str,
        root: &str,
    ) -> Result<(), HkError> {
        self.transaction(|r| {
            let env = r
                .environments
                .iter()
                .find(|e| e.id == environment)
                .ok_or_else(|| HkError::NotFound("Environment".into()))?;
            let old = env
                .providers
                .iter()
                .find(|p| p.id == provider)
                .ok_or_else(|| HkError::NotFound("Provider".into()))?;
            if old.root != expected {
                return Err(HkError::Conflict(
                    "Provider path changed; reload settings".into(),
                ));
            }
            if root.trim().is_empty() {
                return Err(HkError::Validation("Provider path cannot be empty".into()));
            }
            let mut next = old.clone();
            next.root = super::paths::for_connection(root.trim(), &env.connection);
            if matches!(env.connection, super::transport::Connection::Local) {
                let path = Path::new(&next.root);
                if !path.is_absolute() {
                    return Err(HkError::Validation(
                        "Choose an absolute provider path".into(),
                    ));
                }
                next.root = fs::canonicalize(path)?.to_string_lossy().into_owned();
            } else if !next.root.starts_with('/')
                && !(next.root.len() > 2 && next.root.as_bytes()[1] == b':')
            {
                return Err(HkError::Validation(
                    "Choose an absolute provider path".into(),
                ));
            }
            list_provider(env, &next)?;
            if next.root != old.root {
                next.paused = true;
            }
            for skill in &mut r.skills {
                for m in &mut skill.members {
                    if m.environment == environment && m.provider == provider {
                        m.path = installation_path(env, &next, &skill.name);
                        m.previous = None;
                        m.fingerprint = None;
                        m.scanned_at = None;
                        m.error = Some("Path changed; scan required".into());
                    }
                }
            }
            let key = format!("{environment}/{provider}");
            let native_root = r
                .provider_path_overrides
                .get(&key)
                .map(|entry| entry.native_root.clone())
                .unwrap_or_else(|| old.root.clone());
            r.provider_path_overrides.insert(
                key,
                super::catalog::PathOverride {
                    native_root,
                    root: next.root.clone(),
                },
            );
            *r.environments
                .iter_mut()
                .find(|e| e.id == environment)
                .unwrap()
                .providers
                .iter_mut()
                .find(|p| p.id == provider)
                .unwrap() = next;
            Ok(())
        })
    }
    pub fn overview(&self) -> Result<Overview, HkError> {
        self.transaction(|r| {
            Ok(Overview {
                browse: None,
                storage_bytes: storage_bytes(&self.root),
                inventory: super::inventory::inventory(r)?,
                statuses: r
                    .skills
                    .iter()
                    .map(|s| SkillStatus {
                        id: s.id.clone(),
                        decision: decision(r, s),
                    })
                    .collect(),
                registry: r.clone(),
            })
        })
    }
    pub fn refresh(&self) -> Result<(), HkError> {
        self.transaction(|r| {
            let envs = r.environments.clone();
            for env in &envs {
                for provider in &env.providers {
                    if matches!(env.connection, super::transport::Connection::Local) {
                        for skill in &r.skills {
                            for member in &skill.members {
                                if member.environment == env.id && member.provider == provider.id {
                                    let _ =
                                        read_installation(env, provider, &skill.name, &member.path);
                                }
                            }
                        }
                    }
                    let listing = match r
                        .catalog
                        .iter()
                        .find(|c| {
                            c.environment == env.id && c.provider.as_deref() == Some(&provider.id)
                        })
                        .and_then(|c| c.error.as_ref())
                        .filter(|error| error.starts_with("Cannot access Windows "))
                    {
                        Some(error) => Err(HkError::Validation(error.clone())),
                        None => list_provider(env, provider),
                    };
                    if let Some(current) = r
                        .environments
                        .iter_mut()
                        .find(|e| e.id == env.id)
                        .and_then(|e| e.providers.iter_mut().find(|p| p.id == provider.id))
                    {
                        current.last_error = listing.as_ref().err().map(ToString::to_string);
                        if listing.is_ok() {
                            current.last_scanned = Some(chrono::Utc::now().to_rfc3339());
                        }
                    }
                    let origins = if listing.is_ok() {
                        provider_origins(env, provider).unwrap_or_default()
                    } else {
                        std::collections::HashMap::new()
                    };
                    let entries = match listing {
                        Ok(v) => v,
                        Err(e) => {
                            for skill in &mut r.skills {
                                for m in &mut skill.members {
                                    if m.environment == env.id && m.provider == provider.id {
                                        m.error = Some(e.to_string());
                                    }
                                }
                            }
                            continue;
                        }
                    };
                    let found = entries.into_iter().map(|name| {
                        let path = installation_path(env, provider, &name);
                        (name, path)
                    });
                    for (name, path) in found {
                        let provenance = r
                            .jobs
                            .iter()
                            .rev()
                            .find(|j| {
                                j.environment == env.id
                                    && j.provider == provider.id
                                    && j.name == name
                                    && j.status == "complete"
                            })
                            .map(|j| format!("{}#{}", j.source.trim_end_matches(".git"), name))
                            .or_else(|| {
                                origins
                                    .get(&name)
                                    .map(|url| format!("{}#{}", url.trim_end_matches(".git"), name))
                            });
                        let existing = r
                            .skills
                            .iter()
                            .position(|s| {
                                s.members.iter().any(|m| {
                                    m.environment == env.id
                                        && m.provider == provider.id
                                        && m.path == path
                                })
                            })
                            .or_else(|| {
                                r.skills.iter().position(|s| {
                                    s.scope == provider.scope
                                        && s.name == name
                                        && s.original.is_none()
                                        && !s.members.iter().any(|m| {
                                            m.provenance.is_some()
                                                && provenance.is_some()
                                                && m.provenance != provenance
                                        })
                                })
                            });
                        let index = existing.unwrap_or_else(|| {
                            r.skills.push(Skill {
                                identity_confirmed: false,
                                id: uuid::Uuid::new_v4().to_string(),
                                name: name.clone(),
                                scope: provider.scope.clone(),
                                original: None,
                                stopped: false,
                                members: vec![],
                                revisions: vec![],
                            });
                            r.skills.len() - 1
                        });
                        let skill = &mut r.skills[index];
                        if !skill
                            .members
                            .iter()
                            .any(|m| m.environment == env.id && m.provider == provider.id)
                        {
                            skill.members.push(Member {
                                provenance: provenance.clone(),
                                id: uuid::Uuid::new_v4().to_string(),
                                environment: env.id.clone(),
                                provider: provider.id.clone(),
                                path: path.clone(),
                                excluded: false,
                                fingerprint: None,
                                previous: None,
                                error: None,
                                scanned_at: None,
                            });
                        }
                        if let Some(m) = skill
                            .members
                            .iter_mut()
                            .find(|m| m.environment == env.id && m.provider == provider.id)
                        {
                            if provenance.is_some() {
                                m.provenance = provenance;
                            }
                        }
                    }
                    for skill in &mut r.skills {
                        for m in &mut skill.members {
                            if m.environment != env.id || m.provider != provider.id {
                                continue;
                            }
                            match read_installation(env, provider, &skill.name, &m.path) {
                                Ok(snapshot) => {
                                    let fingerprint =
                                        snapshot.as_ref().map(|s| s.fingerprint.clone());
                                    if m.scanned_at.is_none() || m.fingerprint != fingerprint {
                                        skill.revisions.push(Revision {
                                            path: Some(m.path.clone()),
                                            number: skill
                                                .revisions
                                                .last()
                                                .map(|r| r.number + 1)
                                                .unwrap_or(1),
                                            environment: env.id.clone(),
                                            captured_at: chrono::Utc::now().to_rfc3339(),
                                            snapshot,
                                        });
                                    }
                                    if fingerprint.is_none()
                                        && m.previous.is_some()
                                        && r.preferences.removal == "ignore"
                                    {
                                        m.excluded = true;
                                    }
                                    m.fingerprint = fingerprint;
                                    m.error = None;
                                    m.scanned_at = Some(chrono::Utc::now().to_rfc3339());
                                }
                                Err(e) => m.error = Some(e.to_string()),
                            }
                        }
                    }
                }
            }
            for skill in &mut r.skills {
                let present: Vec<_> = skill
                    .members
                    .iter()
                    .filter(|m| m.fingerprint.is_some())
                    .collect();
                if !present.is_empty()
                    && present
                        .iter()
                        .all(|m| m.provenance.is_some() && m.provenance == present[0].provenance)
                {
                    skill.identity_confirmed = true;
                }
            }
            let occupied: Vec<_> = r
                .skills
                .iter()
                .flat_map(|s| {
                    s.members
                        .iter()
                        .filter(|m| m.fingerprint.is_some())
                        .map(|m| {
                            (
                                m.environment.clone(),
                                m.provider.clone(),
                                m.path.clone(),
                                s.id.clone(),
                            )
                        })
                })
                .collect();
            // Add eligible missing destinations only after the source has been observed.
            for skill in &mut r.skills {
                if skill.stopped || r.preferences.policy == "none" {
                    continue;
                }
                let provider_names: Vec<_> = skill
                    .members
                    .iter()
                    .filter_map(|m| {
                        envs.iter()
                            .find(|e| e.id == m.environment)
                            .and_then(|e| e.providers.iter().find(|p| p.id == m.provider))
                            .map(|p| p.name.clone())
                    })
                    .collect();
                for env in &envs {
                    for provider in &env.providers {
                        if provider.scope != skill.scope {
                            continue;
                        }
                        let target_path = installation_path(env, provider, &skill.name);
                        if occupied.iter().any(|(e, p, path, id)| {
                            e == &env.id
                                && p == &provider.id
                                && path == &target_path
                                && id != &skill.id
                        }) {
                            continue;
                        }
                        if r.preferences.policy == "like"
                            && !provider_names.contains(&provider.name)
                        {
                            continue;
                        }
                        if !skill
                            .members
                            .iter()
                            .any(|m| m.environment == env.id && m.provider == provider.id)
                        {
                            skill.members.push(Member {
                                provenance: None,
                                id: uuid::Uuid::new_v4().to_string(),
                                environment: env.id.clone(),
                                provider: provider.id.clone(),
                                path: installation_path(env, provider, &skill.name),
                                excluded: false,
                                fingerprint: None,
                                previous: None,
                                error: Some("Pending first scan".into()),
                                scanned_at: None,
                            });
                        }
                    }
                }
            }
            let settled: Vec<_> = r
                .skills
                .iter()
                .filter(|s| decision(r, s) == Decision::Synced)
                .map(|s| s.id.clone())
                .collect();
            for skill in &mut r.skills {
                if settled.contains(&skill.id) {
                    for m in &mut skill.members {
                        if m.error.is_none() {
                            m.previous = m.fingerprint.clone();
                        }
                    }
                }
            }
            Ok(())
        })
    }
    pub fn keep(&self, skill_id: &str, member_id: &str, generation: u64) -> Result<(), HkError> {
        let failure=self.transaction(|r|{
  if generation!=r.generation{return Err(HkError::Conflict("Inventory changed; refresh the decision".into()));}
  let skill=r.skills.iter().find(|s|s.id==skill_id).cloned().ok_or_else(||HkError::NotFound("Skill".into()))?;
  let source=skill.members.iter().find(|m|m.id==member_id).ok_or_else(||HkError::NotFound("Installation".into()))?;
  if source.error.is_some()||paused(r,source)||source.excluded||skill.stopped{return Err(HkError::Validation("Source is unavailable or not participating".into()));}
  let source_env=r.environments.iter().find(|e|e.id==source.environment).unwrap();
  let source_provider=source_env.providers.iter().find(|p|p.id==source.provider).unwrap();
  let snapshot=read_installation(source_env,source_provider,&skill.name,&source.path)?;
  if snapshot.as_ref().map(|s|&s.fingerprint)!=source.fingerprint.as_ref(){return Err(HkError::Conflict("Source changed; refresh before resolving".into()));}
  let mut unavailable = std::collections::HashSet::new();
  let mut failure = None;
  // Validate every reachable destination before the first mutation.
  for m in &skill.members {
   if m.id==source.id||m.excluded||paused(r,m)||m.error.is_some()||(r.preferences.policy=="like"&&provider_name(r,m)!=provider_name(r,source)){continue;}
   let env=r.environments.iter().find(|e|e.id==m.environment).unwrap();
   let provider=env.providers.iter().find(|p|p.id==m.provider).unwrap();
   let current=match read_installation(env,provider,&skill.name,&m.path) {
    Ok(value)=>value,
    Err(error)=>{
     unavailable.insert(m.id.clone());
     r.skills.iter_mut().find(|s|s.id==skill_id).unwrap().members.iter_mut().find(|v|v.id==m.id).unwrap().error=Some(error.to_string());
     failure=Some(error);continue;
    }
   };
   if current.as_ref().map(|f|&f.fingerprint)!=m.fingerprint.as_ref(){return Err(HkError::Conflict("A destination changed; refresh before resolving".into()));}
  }
  r.skills.iter_mut().find(|s|s.id==skill_id).unwrap().identity_confirmed=true;
  for m in &skill.members{if m.id==source.id||m.excluded||paused(r,m)||(r.preferences.policy=="like"&&provider_name(r,m)!=provider_name(r,source)){continue;}
   if m.error.is_some() || unavailable.contains(&m.id){continue;}
   let env=r.environments.iter().find(|e|e.id==m.environment).unwrap();let provider=env.providers.iter().find(|p|p.id==m.provider).unwrap();
   let write=if matches!(env.connection,super::transport::Connection::Local){replace_folder(&local_installation_path(provider,&skill.name,&m.path)?,m.fingerprint.as_deref(),snapshot.as_ref(),&self.root.join("history"))}else{super::transport::run(&env.connection,serde_json::json!({"action":"write","root":provider.root,"name":skill.name,"expected":m.fingerprint,"folder":snapshot})).map(|_|())};
   // Persist completed destinations even if a later transport fails.
   if let Err(error)=write{
    r.skills.iter_mut().find(|s|s.id==skill_id).unwrap().members.iter_mut().find(|v|v.id==m.id).unwrap().error=Some(error.to_string());
    failure=Some(error);continue;
   }

   let updated=r.skills.iter_mut().find(|s|s.id==skill_id).unwrap();
   if m.fingerprint!=source.fingerprint{updated.revisions.push(Revision{path:Some(m.path.clone()),number:updated.revisions.last().map(|r|r.number+1).unwrap_or(1),environment:m.environment.clone(),captured_at:chrono::Utc::now().to_rfc3339(),snapshot:snapshot.clone()});}
   let target=updated.members.iter_mut().find(|v|v.id==m.id).unwrap();target.fingerprint=source.fingerprint.clone();target.previous=source.fingerprint.clone();target.provenance=source.provenance.clone();
  }
  if failure.is_none() && skill.members.iter().all(|m|m.excluded || (r.preferences.policy=="like"&&provider_name(r,m)!=provider_name(r,source)) || (!paused(r,m) && m.error.is_none())) {
 let updated=r.skills.iter_mut().find(|s|s.id==skill_id).unwrap();
 updated.members.iter_mut().find(|m|m.id==member_id).unwrap().previous=source.fingerprint.clone();
 }
  Ok(failure)
 })?;
        if let Some(error) = failure {
            Err(error)
        } else {
            Ok(())
        }
    }
    pub fn edit(
        &self,
        skill_id: &str,
        member_id: &str,
        file: &str,
        expected: &str,
        content: &str,
    ) -> Result<(), HkError> {
        self.transaction(|r|{
  let s=r.skills.iter().find(|s|s.id==skill_id).cloned().ok_or_else(||HkError::NotFound("Skill".into()))?;
  let m=s.members.iter().find(|m|m.id==member_id).ok_or_else(||HkError::NotFound("Installation".into()))?;
  let e=r.environments.iter().find(|e|e.id==m.environment).ok_or_else(||HkError::NotFound("Environment".into()))?;
  let p=e.providers.iter().find(|p|p.id==m.provider).ok_or_else(||HkError::NotFound("Provider".into()))?;
  let old=read_installation(e,p,&s.name,&m.path)?.ok_or_else(||HkError::NotFound("Skill folder".into()))?;
  if old.fingerprint!=expected{return Err(HkError::Conflict("Folder changed since comparison opened".into()));}
  if !old.files.contains_key(file)||content.len()>4*1024*1024||content.contains('\0'){return Err(HkError::Validation("Choose an existing text file below 4 MiB".into()));}
  let mut files=old.files.clone();files.insert(file.into(),content.as_bytes().to_vec());let next=Folder::with_metadata(files,old.modes.clone(),old.directories.clone());
  if matches!(e.connection,super::transport::Connection::Local){replace_folder(&local_installation_path(p,&s.name,&m.path)?,Some(expected),Some(&next),&self.root.join("history"))?;}else{super::transport::run(&e.connection,serde_json::json!({"action":"write","root":p.root,"name":s.name,"expected":expected,"folder":next}))?;}
  let updated=r.skills.iter_mut().find(|s|s.id==skill_id).unwrap();
  updated.revisions.push(Revision{path:Some(m.path.clone()),number:updated.revisions.last().map(|r|r.number+1).unwrap_or(1),environment:m.environment.clone(),captured_at:chrono::Utc::now().to_rfc3339(),snapshot:Some(next.clone())});
  updated.members.iter_mut().find(|m|m.id==member_id).unwrap().fingerprint=Some(next.fingerprint);
  Ok(())
 })
    }

    pub fn revision_action(
        &self,
        skill_id: &str,
        member_id: &str,
        number: usize,
        variant: Option<&str>,
    ) -> Result<(), HkError> {
        self.transaction(|r|{
  let skill=r.skills.iter().find(|s|s.id==skill_id).cloned().ok_or_else(||HkError::NotFound("Skill".into()))?;
  let member=skill.members.iter().find(|m|m.id==member_id).ok_or_else(||HkError::NotFound("Installation".into()))?;
  let revision=skill.revisions.iter().find(|v|v.number==number).ok_or_else(||HkError::NotFound("Revision".into()))?;
  let env=r.environments.iter().find(|e|e.id==member.environment).ok_or_else(||HkError::NotFound("Environment".into()))?;
  let provider=env.providers.iter().find(|p|p.id==member.provider).ok_or_else(||HkError::NotFound("Provider".into()))?;
  let name=variant.unwrap_or(&skill.name);
  if name.is_empty()||name.contains(['/', '\\', ':'])||name=="."||name==".."{return Err(HkError::Validation("Choose a unique folder name".into()));}
  if variant.is_some()&&r.skills.iter().any(|s|s.name==name&&s.scope==skill.scope){return Err(HkError::Conflict("Variant name already exists".into()));}
  let path=if variant.is_some(){installation_path(env,provider,name)}else{member.path.clone()};
  let expected=if variant.is_some(){None}else{member.fingerprint.as_deref()};
  if matches!(env.connection,super::transport::Connection::Local){replace_folder(&local_installation_path(provider,name,&path)?,expected,revision.snapshot.as_ref(),&self.root.join("history"))?;}else{super::transport::run(&env.connection,serde_json::json!({"action":"write","root":provider.root,"name":name,"expected":expected,"folder":revision.snapshot}))?;}
  let mut next=revision.clone();next.captured_at=chrono::Utc::now().to_rfc3339();next.environment=member.environment.clone();next.path=Some(path.clone());let fingerprint=next.snapshot.as_ref().map(|s|s.fingerprint.clone());
  if variant.is_some(){next.number=1;let mut installation=member.clone();installation.id=uuid::Uuid::new_v4().to_string();installation.path=path;installation.fingerprint=fingerprint;installation.previous=None;installation.excluded=false;installation.error=None;r.skills.push(Skill{identity_confirmed:true,id:uuid::Uuid::new_v4().to_string(),name:name.into(),scope:skill.scope.clone(),original:Some(skill.id.clone()),stopped:false,members:vec![installation],revisions:vec![next]});}
  else{let updated=r.skills.iter_mut().find(|s|s.id==skill_id).unwrap();next.number=updated.revisions.last().map(|r|r.number+1).unwrap_or(1);updated.revisions.push(next);updated.members.iter_mut().find(|m|m.id==member_id).unwrap().fingerprint=fingerprint;}
  Ok(())
 })
    }
}
fn storage_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let Ok(kind) = entry.file_type() else {
                return 0;
            };
            if kind.is_symlink() {
                0
            } else if kind.is_dir() {
                storage_bytes(&entry.path())
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}
fn provider_name<'a>(r: &'a Registry, m: &Member) -> &'a str {
    r.environments
        .iter()
        .find(|e| e.id == m.environment)
        .and_then(|e| e.providers.iter().find(|p| p.id == m.provider))
        .map(|p| p.name.as_str())
        .unwrap_or("")
}
fn paused(r: &Registry, m: &Member) -> bool {
    r.environments
        .iter()
        .find(|e| e.id == m.environment)
        .is_none_or(|e| {
            e.paused
                || e.providers
                    .iter()
                    .find(|p| p.id == m.provider)
                    .is_none_or(|p| p.paused || p.disabled)
        })
}
pub fn decision(r: &Registry, s: &Skill) -> Decision {
    if s.stopped || r.preferences.policy == "none" {
        return Decision::Off;
    }
    if !s.identity_confirmed
        && s.members
            .iter()
            .filter(|m| m.fingerprint.is_some() && !m.excluded)
            .count()
            > 1
    {
        return Decision::IdentityDecision;
    }
    let observations = s
        .members
        .iter()
        .map(|m| Observation {
            id: m.id.clone(),
            available: m.error.is_none(),
            paused: paused(r, m),
            excluded: m.excluded,
            current: m.fingerprint.clone(),
            previous: m.previous.clone(),
        })
        .collect::<Vec<_>>();
    let partitions: Vec<Vec<Observation>> = if r.preferences.policy == "like" {
        let names = observations
            .iter()
            .filter_map(|o| {
                s.members
                    .iter()
                    .find(|m| m.id == o.id)
                    .map(|m| provider_name(r, m).to_string())
            })
            .collect::<std::collections::BTreeSet<_>>();
        names
            .into_iter()
            .map(|name| {
                observations
                    .iter()
                    .filter(|o| {
                        s.members
                            .iter()
                            .any(|m| m.id == o.id && provider_name(r, m) == name)
                    })
                    .cloned()
                    .collect()
            })
            .collect()
    } else {
        vec![observations]
    };
    let mut result = Decision::Synced;
    for partition in partitions {
        let host_member = s
            .members
            .iter()
            .find(|m| {
                Some(&m.environment) == r.preferences.host.as_ref()
                    && partition.iter().any(|o| o.id == m.id)
            })
            .map(|m| m.id.as_str());
        if r.preferences.direction == "host" && host_member.is_none() {
            return Decision::Conflict;
        }
        let current = reconcile(
            &partition,
            r.preferences.removal == "ask",
            if r.preferences.direction == "host" {
                host_member
            } else {
                None
            },
        );
        if matches!(current, Decision::Conflict | Decision::RemovalDecision) {
            return current;
        }
        if !matches!(current, Decision::Synced | Decision::Off) {
            result = current;
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Coordinator) {
        let t = tempfile::tempdir().unwrap();
        let c = Coordinator::new(t.path().join("state"));
        c.transaction(|r| {
            for id in ["a", "b"] {
                let root = t.path().join(id);
                fs::create_dir_all(root.join("review")).unwrap();
                fs::write(root.join("review/SKILL.md"), "same").unwrap();
                r.environments.push(Environment {
                    connection: super::super::transport::Connection::Local,
                    id: id.into(),
                    machine: id.into(),
                    name: id.into(),
                    os: "linux".into(),
                    paused: false,
                    providers: vec![Provider {
                        disabled: false,
                        last_error: None,
                        last_scanned: None,
                        scope: "global".into(),
                        id: "codex:0".into(),
                        name: "codex".into(),
                        root: root.to_string_lossy().into_owned(),
                        paused: false,
                    }],
                });
            }
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        c.transaction(|r| {
            r.skills[0].identity_confirmed = true;
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        (t, c)
    }
    #[test]
    fn mapped_provider_starts_paused_and_refresh_preserves_resume() {
        let (t, c) = setup();
        let root = t.path().join("mapped");
        fs::create_dir(&root).unwrap();
        c.map_provider("a", "claude", root.to_str().unwrap(), "global")
            .unwrap();
        let v = c.overview().unwrap();
        let p = v.registry.environments[0]
            .providers
            .iter()
            .find(|p| p.root == root.to_string_lossy())
            .unwrap();
        assert!(p.paused);
        let id = p.id.clone();
        c.transaction(|r| {
            r.environments[0]
                .providers
                .iter_mut()
                .find(|p| p.id == id)
                .unwrap()
                .paused = false;
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        assert!(
            !c.overview().unwrap().registry.environments[0]
                .providers
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .paused
        );
    }
    #[test]
    fn provider_disappearing_after_scan_does_not_block_later_healthy_target() {
        let (t, c) = setup();
        c.transaction(|r| {
            let mut third = r.environments[1].clone();
            third.id = "c".into();
            third.providers[0].root = t.path().join("c").to_string_lossy().into_owned();
            fs::create_dir_all(t.path().join("c/review")).unwrap();
            fs::write(t.path().join("c/review/SKILL.md"), "same").unwrap();
            r.environments.push(third);
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        fs::write(t.path().join("a/review/SKILL.md"), "updated").unwrap();
        c.refresh().unwrap();
        let v = c.overview().unwrap();
        let s = &v.registry.skills[0];
        let a = s.members.iter().find(|m| m.environment == "a").unwrap();
        fs::remove_dir_all(t.path().join("b")).unwrap();
        assert!(c.keep(&s.id, &a.id, v.registry.generation).is_err());
        assert_eq!(
            fs::read_to_string(t.path().join("c/review/SKILL.md")).unwrap(),
            "updated"
        );
        assert!(
            c.overview().unwrap().registry.skills[0]
                .members
                .iter()
                .find(|m| m.environment == "b")
                .unwrap()
                .error
                .is_some()
        );
        assert!(!t.path().join("b").exists());
    }
    #[test]
    fn inaccessible_provider_does_not_block_healthy_sync() {
        let (t, c) = setup();
        c.transaction(|r| {
            let mut third = r.environments[1].clone();
            third.id = "broken".into();
            third.providers[0].root = t.path().join("missing").to_string_lossy().into_owned();
            r.environments.push(third);
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        fs::write(t.path().join("a/review/SKILL.md"), "healthy change").unwrap();
        c.refresh().unwrap();
        let v = c.overview().unwrap();
        let s = &v.registry.skills[0];
        let a = s.members.iter().find(|m| m.environment == "a").unwrap();
        c.keep(&s.id, &a.id, v.registry.generation).unwrap();
        assert_eq!(
            fs::read_to_string(t.path().join("b/review/SKILL.md")).unwrap(),
            "healthy change"
        );
        assert!(!t.path().join("missing").exists());
        assert!(
            c.overview()
                .unwrap()
                .registry
                .environments
                .iter()
                .find(|e| e.id == "broken")
                .unwrap()
                .providers[0]
                .last_error
                .is_some()
        );
    }
    #[test]
    fn creation_never_recreates_a_previously_scanned_directory() {
        let (t, c) = setup();
        fs::remove_dir_all(t.path().join("b")).unwrap();
        assert!(
            c.create_provider_directory("b", "codex:0", t.path().join("b").to_str().unwrap())
                .is_err()
        );
        assert!(!t.path().join("b").exists());
        c.transaction(|r| {
            let mut p = r.environments[1].providers[0].clone();
            p.id = "new".into();
            p.root = t.path().join("new").to_string_lossy().into_owned();
            p.last_scanned = None;
            r.environments[1].providers.push(p);
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        c.create_provider_directory("b", "new", t.path().join("new").to_str().unwrap())
            .unwrap();
        assert!(t.path().join("new").is_dir());
    }
    #[test]
    fn inventory_read_does_not_invalidate_resolution() {
        let (t, c) = setup();
        fs::write(t.path().join("a/review/SKILL.md"), "changed").unwrap();
        c.refresh().unwrap();
        let view = c.overview().unwrap();
        assert_eq!(
            view.registry.generation,
            c.overview().unwrap().registry.generation
        );
        let s = &view.registry.skills[0];
        let a = s.members.iter().find(|m| m.environment == "a").unwrap();
        c.keep(&s.id, &a.id, view.registry.generation).unwrap();
        assert_eq!(
            fs::read_to_string(t.path().join("b/review/SKILL.md")).unwrap(),
            "changed"
        );
    }
    #[test]
    fn changed_target_after_preview_is_not_overwritten() {
        let (t, c) = setup();
        fs::write(t.path().join("a/review/SKILL.md"), "a edit").unwrap();
        c.refresh().unwrap();
        let v = c.overview().unwrap();
        let s = &v.registry.skills[0];
        fs::write(t.path().join("b/review/SKILL.md"), "b edit").unwrap();
        assert!(
            c.keep(&s.id, &s.members[0].id, v.registry.generation)
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(t.path().join("b/review/SKILL.md")).unwrap(),
            "b edit"
        );
    }
    #[test]
    fn stale_third_environment_prevents_any_write() {
        let (t, c) = setup();
        c.transaction(|r| {
            let mut third = r.environments[1].clone();
            third.id = "c".into();
            third.providers[0].root = t.path().join("c").to_string_lossy().into_owned();
            fs::create_dir_all(t.path().join("c/review")).unwrap();
            fs::write(t.path().join("c/review/SKILL.md"), "same").unwrap();
            r.environments.push(third);
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        fs::write(t.path().join("a/review/SKILL.md"), "source edit").unwrap();
        c.refresh().unwrap();
        let v = c.overview().unwrap();
        let s = &v.registry.skills[0];
        let source = s.members.iter().find(|m| m.environment == "a").unwrap();
        fs::write(t.path().join("c/review/SKILL.md"), "concurrent edit").unwrap();
        assert!(c.keep(&s.id, &source.id, v.registry.generation).is_err());
        assert_eq!(
            fs::read_to_string(t.path().join("b/review/SKILL.md")).unwrap(),
            "same"
        );
    }
    #[test]
    fn host_mode_without_a_host_copy_never_pushes_remote_changes() {
        let (t, c) = setup();
        c.transaction(|r| {
            r.preferences.direction = "host".into();
            r.preferences.host = Some("not-a-member".into());
            Ok(())
        })
        .unwrap();
        fs::write(t.path().join("a/review/SKILL.md"), "remote edit").unwrap();
        c.refresh().unwrap();
        assert_eq!(
            c.overview().unwrap().statuses[0].decision,
            Decision::Conflict
        );
    }
    #[test]
    fn changing_provider_path_keeps_old_files_and_requires_scan() {
        let (t, c) = setup();
        let next = t.path().join("new-root");
        fs::create_dir(&next).unwrap();
        let old = t.path().join("b").to_string_lossy().into_owned();
        c.update_provider_path("b", "codex:0", &old, next.to_str().unwrap())
            .unwrap();
        let v = c.overview().unwrap();
        let m = v.registry.skills[0]
            .members
            .iter()
            .find(|m| m.environment == "b")
            .unwrap();
        assert!(m.error.is_some());
        assert!(m.previous.is_none());
        assert!(t.path().join("b/review/SKILL.md").is_file());
        assert!(
            c.update_provider_path("b", "codex:0", &old, next.to_str().unwrap())
                .is_err()
        );
    }
    #[test]
    fn remote_member_actions_preserve_content_and_reject_stale_changes() {
        let (t, c) = setup();
        let v = c.overview().unwrap();
        let m = &v.registry.skills[0].members[1];
        c.extension_action(&m.id, "toggle", m.fingerprint.as_deref(), Some(false))
            .unwrap();
        assert_eq!(
            fs::read_to_string(t.path().join("b/review/SKILL.md.disabled")).unwrap(),
            "same"
        );
        assert!(!t.path().join("b/review/SKILL.md").exists());
        assert!(
            c.extension_action(&m.id, "delete", m.fingerprint.as_deref(), None)
                .is_err()
        );
        assert!(t.path().join("b/review/SKILL.md.disabled").exists());
        let content = c.extension_action(&m.id, "content", None, None).unwrap();
        assert_eq!(content["content"], "same");
    }
    #[test]
    fn different_known_sources_do_not_merge_or_enroll_over_each_other() {
        let (t, c) = setup();
        for id in ["a", "b"] {
            let git = t.path().join(id).join("review/.git");
            fs::create_dir(&git).unwrap();
            fs::write(
                git.join("config"),
                format!("[remote \"origin\"]\nurl = https://example.com/{id}/repo.git\n"),
            )
            .unwrap();
        }
        c.transaction(|r| {
            r.skills.clear();
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        let view = c.overview().unwrap();
        assert_eq!(view.registry.skills.len(), 2);
        assert!(view.registry.skills.iter().all(|s| s.members.len() == 1));
    }
    #[test]
    fn restored_member_path_cannot_escape_its_provider_root() {
        let (t, c) = setup();
        let outside = t.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("SKILL.md"), "private").unwrap();
        let mut registry = c.overview().unwrap().registry;
        registry.skills[0].members[0].path = outside.to_string_lossy().into_owned();
        assert!(super::super::backup::restore(&c, registry).is_err());
        c.transaction(|r| {
            r.skills[0].members[0].path = outside.to_string_lossy().into_owned();
            Ok(())
        })
        .unwrap();
        c.refresh().unwrap();
        let view = c.overview().unwrap();
        assert!(view.registry.skills[0].members[0].error.is_some());
        assert_eq!(
            fs::read_to_string(outside.join("SKILL.md")).unwrap(),
            "private"
        );
    }
    #[test]
    fn stopped_skill_never_writes() {
        let (t, c) = setup();
        c.transaction(|r| {
            r.skills[0].stopped = true;
            Ok(())
        })
        .unwrap();
        let v = c.overview().unwrap();
        assert_eq!(v.statuses[0].decision, Decision::Off);
        let s = &v.registry.skills[0];
        assert!(
            c.keep(&s.id, &s.members[0].id, v.registry.generation)
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(t.path().join("b/review/SKILL.md")).unwrap(),
            "same"
        );
    }
}

pub(super) fn installation_path(env: &Environment, provider: &Provider, name: &str) -> String {
    if matches!(env.connection, super::transport::Connection::Local) {
        Path::new(&provider.root)
            .join(name)
            .to_string_lossy()
            .into_owned()
    } else {
        format!("{}/{}", provider.root.trim_end_matches(['/', '\\']), name)
    }
}
pub(super) fn list_provider(
    env: &Environment,
    provider: &Provider,
) -> Result<Vec<String>, HkError> {
    if !matches!(env.connection, super::transport::Connection::Local) {
        return Ok(serde_json::from_value(super::transport::run(
            &env.connection,
            serde_json::json!({"action":"list","root":provider.root}),
        )?)?);
    }
    let mut names = vec![];
    for entry in fs::read_dir(&provider.root)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with(".hk-") {
            continue;
        }
        let path = entry.path();
        if path.join("SKILL.md").is_file() || path.join("SKILL.md.disabled").is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}
pub(super) fn read_installation(
    env: &Environment,
    provider: &Provider,
    name: &str,
    path: &str,
) -> Result<Option<Folder>, HkError> {
    if matches!(env.connection, super::transport::Connection::Local) {
        read_folder(&local_installation_path(provider, name, path)?)
    } else {
        Ok(serde_json::from_value(super::transport::run(
            &env.connection,
            serde_json::json!({"action":"read","root":provider.root,"name":name}),
        )?)?)
    }
}

fn provider_origins(
    env: &Environment,
    provider: &Provider,
) -> Result<std::collections::HashMap<String, String>, HkError> {
    if !matches!(env.connection, super::transport::Connection::Local) {
        return Ok(serde_json::from_value(super::transport::run(
            &env.connection,
            serde_json::json!({"action":"origins","root":provider.root}),
        )?)?);
    }
    let mut origins = std::collections::HashMap::new();
    for entry in crate::scanner::scan_skill_dir(Path::new(&provider.root), &provider.name, false) {
        if let (Some(path), Some(url)) = (entry.source_path, entry.source.url) {
            if let Some(name) = Path::new(&path).parent().and_then(Path::file_name) {
                origins.insert(name.to_string_lossy().into_owned(), url);
            }
        }
    }
    Ok(origins)
}

fn local_installation_path(
    provider: &Provider,
    name: &str,
    path: &str,
) -> Result<PathBuf, HkError> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
        return Err(HkError::Validation("Invalid skill name".into()));
    }
    let root = Path::new(&provider.root);
    if !root.is_absolute() || Path::new(path) != root.join(name) {
        return Err(HkError::PathNotAllowed(path.into()));
    }
    let canonical = fs::canonicalize(root)?;
    if !canonical.is_dir() {
        return Err(HkError::Validation("Provider root is unavailable".into()));
    }
    Ok(canonical.join(name))
}
