use super::{
    folders::{Folder, replace_folder},
    registry::{Coordinator, installation_path, read_installation},
    transport::{self, Connection},
};
use crate::HkError;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct InstallJob {
    pub id: String,
    pub environment: String,
    pub provider: String,
    pub name: String,
    pub source: String,
    pub revision: Option<String>,
    pub snapshot: Folder,
    pub status: String,
    pub error: Option<String>,
}
pub fn enqueue(
    c: &Coordinator,
    environment: &str,
    provider: &str,
    name: &str,
    source: &str,
    revision: Option<String>,
    snapshot: Folder,
) -> Result<(), HkError> {
    c.transaction(|r| {
        if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
            return Err(HkError::Validation("Invalid skill folder name".into()));
        }
        if !r
            .environments
            .iter()
            .any(|e| e.id == environment && e.providers.iter().any(|p| p.id == provider))
        {
            return Err(HkError::NotFound("Install destination".into()));
        }
        if r.jobs.iter().any(|j| {
            j.environment == environment
                && j.provider == provider
                && j.name == name
                && j.status == "pending"
        }) {
            return Err(HkError::Conflict("This install is already queued".into()));
        }
        r.jobs.push(InstallJob {
            id: uuid::Uuid::new_v4().to_string(),
            environment: environment.into(),
            provider: provider.into(),
            name: name.into(),
            source: source.into(),
            revision,
            snapshot,
            status: "pending".into(),
            error: None,
        });
        Ok(())
    })
}
pub fn prepare(
    c: &Coordinator,
    environment: &str,
    provider: &str,
    source: &str,
    skill_id: &str,
) -> Result<(), HkError> {
    let url = crate::marketplace::git_url_for_source(source);
    let temp = tempfile::tempdir()?;
    let repo = temp.path().join("repo");
    crate::manager::shallow_clone(&url, &repo)?;
    let target = temp.path().join("installed");
    std::fs::create_dir(&target)?;
    let installed = crate::manager::install_from_clone(&repo, &target, Some(skill_id), &url)?;
    let snapshot = super::folders::read_folder(&target.join(&installed.name))?
        .ok_or_else(|| HkError::NotFound("Prepared skill".into()))?;
    enqueue(
        c,
        environment,
        provider,
        &installed.name,
        &url,
        installed.revision,
        snapshot,
    )
}
pub fn deliver(c: &Coordinator) -> Result<(), HkError> {
    c.transaction(|r|{
 for index in 0..r.jobs.len(){let job=r.jobs[index].clone();if job.status!="pending"{continue;}
  let Some(env)=r.environments.iter().find(|e|e.id==job.environment) else{continue};
  let Some(provider)=env.providers.iter().find(|p|p.id==job.provider) else{continue};
  if env.paused||provider.paused{continue;}
  let path=installation_path(env,provider,&job.name);
  let apply=||->Result<(),HkError>{
   if matches!(env.connection,Connection::Local)&&!std::fs::metadata(&provider.root)?.is_dir(){return Err(HkError::Validation("Provider root is unavailable".into()));}
   let current=read_installation(env,provider,&job.name,&path)?;
   if let Some(current)=current{
    if current.fingerprint==job.snapshot.fingerprint{return Ok(());}
    return Err(HkError::Conflict("Destination already exists; queued install will not overwrite it".into()));
   }
   if matches!(env.connection,Connection::Local){replace_folder(std::path::Path::new(&path),None,Some(&job.snapshot),&c.root.join("history"))?;}else{transport::run(&env.connection,serde_json::json!({"action":"write","root":provider.root,"name":job.name,"expected":null,"folder":job.snapshot}))?;}
   Ok(())
  };
  match apply(){Ok(())=>{r.jobs[index].status="complete".into();r.jobs[index].error=None;},Err(e)=>r.jobs[index].error=Some(e.to_string())}
 }
 Ok(())
})
}
#[cfg(test)]
mod tests {
    use super::super::registry::{Environment, Provider};
    use super::*;
    #[test]
    fn offline_install_waits_and_never_overwrites_a_new_destination() {
        let temp = tempfile::tempdir().unwrap();
        let c = Coordinator::new(temp.path().join("state"));
        let root = temp.path().join("offline");
        c.transaction(|r| {
            r.environments.push(Environment {
                connection: Connection::Local,
                id: "remote".into(),
                machine: "Mac".into(),
                name: "macOS".into(),
                os: "darwin".into(),
                paused: false,
                providers: vec![Provider {
                    disabled: false,
                    last_error: None,
                    last_scanned: None,
                    scope: "global".into(),
                    id: "codex".into(),
                    name: "codex".into(),
                    root: root.to_string_lossy().into_owned(),
                    paused: false,
                }],
            });
            Ok(())
        })
        .unwrap();
        let snapshot = Folder::new(std::collections::BTreeMap::from([(
            "SKILL.md".into(),
            b"install".to_vec(),
        )]));
        enqueue(&c, "remote", "codex", "example", "repo", None, snapshot).unwrap();
        deliver(&c).unwrap();
        assert!(!root.exists());
        assert_eq!(c.overview().unwrap().registry.jobs[0].status, "pending");
        std::fs::create_dir_all(root.join("example")).unwrap();
        std::fs::write(root.join("example/SKILL.md"), "independent").unwrap();
        deliver(&c).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("example/SKILL.md")).unwrap(),
            "independent"
        );
        assert!(c.overview().unwrap().registry.jobs[0].error.is_some());
    }
}
