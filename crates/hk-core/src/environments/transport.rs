use crate::HkError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::Stdio,
    time::{Duration, Instant},
};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Connection {
    #[default]
    Local,
    Ssh {
        host: String,
        user: String,
        port: u16,
    },
    Docker {
        container: String,
        #[serde(default)]
        context: Option<String>,
    },
    Wsl {
        distribution: String,
    },
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_:@".contains(c))
}
pub fn wsl_running(distribution: &str) -> Result<bool, HkError> {
    let running = crate::command("wsl.exe")
        .args(["--list", "--running", "--quiet"])
        .output()?;
    if !running.status.success() {
        return Err(HkError::CommandFailed(
            String::from_utf8_lossy(&running.stderr).into_owned(),
        ));
    }
    Ok(decode_wsl(&running.stdout)
        .lines()
        .any(|n| n.trim() == distribution))
}
fn ssh_args(host: &str, user: &str, port: u16) -> Result<Vec<String>, HkError> {
    if !token(host) || (!user.is_empty() && !token(user)) {
        return Err(HkError::Validation("Invalid SSH address or user".into()));
    }
    let mut args: Vec<String> = [
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        "-o",
        "StrictHostKeyChecking=yes",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    // Zero means no override: honor the user's SSH alias/configuration.
    if port != 0 {
        args.extend(["-p".into(), port.to_string()]);
    }
    args.push(if user.is_empty() {
        host.into()
    } else {
        format!("{user}@{host}")
    });
    Ok(args)
}
// Docker commands never invoke a shell or accept option-shaped identifiers.
fn docker_args(context: Option<&str>, container: &str) -> Result<Vec<String>, HkError> {
    let valid = |s: &str| {
        !s.is_empty()
            && !s.starts_with('-')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
    };
    if !valid(container) || context.is_some_and(|c| !valid(c)) {
        return Err(HkError::Validation(
            "Invalid Docker container or context".into(),
        ));
    }
    Ok(context
        .map(|c| vec!["--context".into(), c.into()])
        .unwrap_or_default())
}
fn bounded_output(
    cmd: &mut std::process::Command,
    timeout: Duration,
) -> Result<std::process::Output, HkError> {
    use std::io::{Read, Seek};
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(HkError::Network("Docker metadata command timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if stdout.metadata()?.len() > 8 * 1024 * 1024 || stderr.metadata()?.len() > 65536 {
        return Err(HkError::Validation(
            "Docker metadata output exceeds limit".into(),
        ));
    }
    stdout.rewind()?;
    stderr.rewind()?;
    let mut out = Vec::new();
    let mut err = Vec::new();
    stdout.read_to_end(&mut out)?;
    stderr.read_to_end(&mut err)?;
    Ok(std::process::Output {
        status,
        stdout: out,
        stderr: err,
    })
}
fn docker_inspect(context: Option<&str>, container: &str) -> Result<Value, HkError> {
    let out = bounded_output(
        crate::command("docker")
            .args(docker_args(context, container)?)
            .args(["container", "inspect", container]),
        Duration::from_secs(5),
    )?;
    if !out.status.success() {
        return Err(HkError::CommandFailed(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    let items: Vec<Value> = serde_json::from_slice(&out.stdout)?;
    items
        .into_iter()
        .next()
        .ok_or_else(|| HkError::Validation("Container was not found".into()))
}
pub fn normalize_connection(connection: Connection) -> Result<Connection, HkError> {
    if let Connection::Docker { container, context } = connection {
        let inspect = docker_inspect(context.as_deref(), &container)?;
        let id = inspect["Id"]
            .as_str()
            .ok_or_else(|| HkError::Validation("Docker container identity unavailable".into()))?;
        docker_args(context.as_deref(), id)?;
        Ok(Connection::Docker {
            container: id.into(),
            context,
        })
    } else {
        Ok(connection)
    }
}
fn docker_access(inspect: &Value) -> Result<Vec<String>, HkError> {
    if inspect["State"]["Running"] != json!(true) {
        return Err(HkError::Validation("Container stopped — start it outside HarnessKit, then reconnect. HarnessKit never starts containers.".into()));
    }
    docker_mounts(inspect)
}
fn docker_mounts(inspect: &Value) -> Result<Vec<String>, HkError> {
    let mounts = inspect["Mounts"].as_array().ok_or_else(|| {
        HkError::Validation("Docker mount information unavailable; access blocked".into())
    })?;
    mounts
        .iter()
        .filter(|m| m["Type"] != "tmpfs")
        .map(|m| {
            m["Destination"]
                .as_str()
                .filter(|p| p.starts_with('/'))
                .map(str::to_owned)
                .ok_or_else(|| {
                    HkError::Validation("Invalid Docker mount destination; access blocked".into())
                })
        })
        .collect()
}
fn docker_candidates(input: &str, context: Option<&str>) -> Vec<Candidate> {
    input
        .lines()
        .filter_map(|line| {
            let value: Value = serde_json::from_str(line).ok()?;
            let container = value["ID"].as_str()?;
            docker_args(context, container).ok()?;
            let name = value["Names"].as_str().unwrap_or(container);
            let labels = value["Labels"].as_str().unwrap_or("");
            let devcontainer = labels.split(',').any(|l| {
                l.starts_with("devcontainer.local_folder=")
                    || l.starts_with("devcontainer.config_file=")
                    || l.starts_with("vsch.local.folder=")
            });
            Some(Candidate {
                name: if devcontainer {
                    format!("{name} · Dev Container")
                } else {
                    name.into()
                },
                connection: Connection::Docker {
                    container: container.into(),
                    context: context.map(str::to_owned),
                },
                status: if value["State"] == "running" {
                    "Running · access not checked"
                } else {
                    "Stopped · start outside HarnessKit"
                }
                .into(),
            })
        })
        .collect()
}
pub fn run(connection: &Connection, mut request: Value) -> Result<Value, HkError> {
    let script = include_str!("remote.py");
    let mut cmd = match connection {
        Connection::Local => {
            return Err(HkError::Validation(
                "Local transport uses native filesystem operations".into(),
            ));
        }
        Connection::Ssh { host, user, port } => {
            let mut cmd = crate::command("ssh");
            cmd.args(ssh_args(host, user, *port)?);
            // All shell text is fixed; user paths and payloads travel only over stdin.
            cmd.arg(format!("python3 -c '{}'", script.replace('\'', "'\\''")));
            cmd
        }
        Connection::Docker { container, context } => {
            let inspect = docker_inspect(context.as_deref(), container)?;
            request["_blocked_mounts"] = json!(docker_access(&inspect)?);
            let id = inspect["Id"].as_str().ok_or_else(|| {
                HkError::Validation("Docker container identity unavailable".into())
            })?;
            let mut cmd = crate::command("docker");
            cmd.args(docker_args(context.as_deref(), id)?);
            cmd.args(["exec", "-i", id, "python3", "-c", script]);
            cmd
        }
        Connection::Wsl { distribution } => {
            if distribution.is_empty() || distribution.contains(['\0', '\n', '\r']) {
                return Err(HkError::Validation("Invalid WSL distribution".into()));
            }
            if !wsl_running(distribution)? {
                return Err(HkError::Validation(
                    "WSL stopped — sync unavailable. Start the distribution explicitly.".into(),
                ));
            }
            let mut cmd = crate::command("wsl.exe");
            cmd.args([
                "--distribution",
                distribution,
                "--exec",
                "python3",
                "-c",
                script,
            ]);
            cmd
        }
    };
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let input = serde_json::to_vec(&request)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| HkError::Internal("Missing transport stdin".into()))?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut bytes = vec![];
        stdout
            .take(512 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let errors = std::thread::spawn(move || {
        use std::io::Read;
        let mut bytes = vec![];
        stderr.take(65536).read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(HkError::Network("Transport timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    writer
        .join()
        .map_err(|_| HkError::Internal("Transport writer failed".into()))??;
    let out = reader
        .join()
        .map_err(|_| HkError::Internal("Transport reader failed".into()))??;
    let err = errors
        .join()
        .map_err(|_| HkError::Internal("Transport error reader failed".into()))??;
    let response: Value = serde_json::from_slice(&out)
        .map_err(|_| HkError::Network(String::from_utf8_lossy(&err).into_owned()))?;
    if let Some(error) = response.get("error") {
        return Err(HkError::Conflict(error.to_string()));
    }
    Ok(response["result"].clone())
}
pub fn decode_wsl(bytes: &[u8]) -> String {
    if bytes.contains(&0) {
        String::from_utf16_lossy(
            &bytes
                .chunks_exact(2)
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}
#[derive(Serialize)]
pub struct Candidate {
    pub name: String,
    pub connection: Connection,
    pub status: String,
}
pub fn discover() -> Vec<Candidate> {
    let mut found = vec![];
    // Use only the existing runtime/context. Never switch context or start anything.
    let context = bounded_output(
        crate::command("docker").args(["context", "show"]),
        Duration::from_secs(5),
    )
    .ok()
    .filter(|o| o.status.success() && std::env::var_os("DOCKER_HOST").is_none())
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    if let Ok(args) = docker_args(context.as_deref(), "discovery") {
        if let Ok(out) = bounded_output(
            crate::command("docker").args(args).args([
                "ps",
                "--all",
                "--no-trunc",
                "--format",
                "{{json .}}",
            ]),
            Duration::from_secs(5),
        ) {
            if out.status.success() {
                found.extend(docker_candidates(
                    &String::from_utf8_lossy(&out.stdout),
                    context.as_deref(),
                ));
            }
        }
    }
    if let Ok(out) = crate::command("wsl.exe")
        .args(["--list", "--quiet"])
        .output()
    {
        if out.status.success() {
            for name in decode_wsl(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                found.push(Candidate {
                    name: name.into(),
                    connection: Connection::Wsl {
                        distribution: name.into(),
                    },
                    status: "Access not checked".into(),
                });
            }
        }
    }
    let tailscale =
        if std::path::Path::new("/mnt/c/Program Files/Tailscale/tailscale.exe").is_file() {
            "/mnt/c/Program Files/Tailscale/tailscale.exe"
        } else {
            "tailscale"
        };
    if let Ok(out) = crate::command(tailscale)
        .args(["status", "--json"])
        .output()
    {
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) {
                if let Some(peers) = v["Peer"].as_object() {
                    for peer in peers.values() {
                        if let Some(host) = peer["DNSName"]
                            .as_str()
                            .or_else(|| peer["HostName"].as_str())
                        {
                            found.push(Candidate {
                                name: host.into(),
                                connection: Connection::Ssh {
                                    host: host.trim_end_matches('.').into(),
                                    user: String::new(),
                                    port: 22,
                                },
                                status: if peer["Online"] == json!(true) {
                                    "Tailscale online · SSH not checked"
                                } else {
                                    "Offline"
                                }
                                .into(),
                            });
                        }
                    }
                }
            }
        }
    }
    found
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_command_is_bounded_and_captures_output() {
        let out = bounded_output(
            crate::command("python3").args(["-c", "print('ok')"]),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout, b"ok\n");
        let start = Instant::now();
        assert!(
            bounded_output(
                crate::command("python3").args(["-c", "import time; time.sleep(10)"]),
                Duration::from_millis(50)
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn docker_arguments_reject_flags_and_shell_fragments() {
        assert!(docker_args(Some("desktop-linux"), "my-container").is_ok());
        for bad in ["", "-x", "a;touch /tmp/a", "a/b", "a\n"] {
            assert!(docker_args(None, bad).is_err());
            assert!(docker_args(Some(bad), "ok").is_err());
        }
    }
    #[test]
    fn docker_mount_checks_fail_closed() {
        assert!(docker_access(&json!({"State":{"Running":false},"Mounts":[]})).is_err());
        assert!(docker_access(&json!({"State":{"Running":true}})).is_err());
        assert!(docker_access(&json!({"State":{"Running":true},"Mounts":[{"Type":"bind","Destination":"relative"}]})).is_err());
        assert_eq!(docker_access(&json!({"State":{"Running":true},"Mounts":[{"Type":"bind","Destination":"/home/user"},{"Type":"volume","Destination":"/work"},{"Type":"tmpfs","Destination":"/tmp"}]})).unwrap(), vec!["/home/user", "/work"]);
    }
    #[test]
    fn docker_candidates_include_stopped_and_devcontainers() {
        let input = "{\"ID\":\"abc123\",\"Names\":\"workspace\",\"State\":\"running\",\"Labels\":\"devcontainer.local_folder=/work\"}\n{\"ID\":\"def456\",\"Names\":\"old\",\"State\":\"exited\",\"Labels\":\"\"}";
        let found = docker_candidates(input, Some("desktop-linux"));
        assert_eq!(found.len(), 2);
        assert!(found[0].name.contains("Dev Container"));
        assert!(found[1].status.contains("Stopped"));
        assert_eq!(
            serde_json::to_value(&found[0].connection).unwrap()["context"],
            "desktop-linux"
        );
    }
    #[test]
    fn ssh_alias_reuses_config_unless_explicitly_overridden() {
        let args = ssh_args("macbook", "", 0).unwrap();
        assert_eq!(args.last().unwrap(), "macbook");
        assert!(!args.contains(&"-p".to_string()));
        assert!(args.contains(&"BatchMode=yes".to_string()));
        assert!(args.contains(&"StrictHostKeyChecking=yes".to_string()));
        let args = ssh_args("macbook", "demonlord", 2222).unwrap();
        assert!(args.ends_with(&["-p".into(), "2222".into(), "demonlord@macbook".into()]));
        assert!(ssh_args("-oProxyCommand=evil", "", 0).is_err());
        assert!(ssh_args("macbook", "user; touch /tmp/evil", 22).is_err());
    }
    #[test]
    fn ssh_options_cannot_be_injected() {
        assert!(!token("-oProxyCommand=evil"));
        assert!(!token("host;touch /tmp/a"));
        assert!(token("my-host.example.ts.net"));
    }
    #[test]
    fn parses_wsl_utf16() {
        let bytes = "Ubuntu\r\nDebian\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(decode_wsl(&bytes).lines().any(|l| l == "Debian"));
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[test]
    fn remote_text_save_preserves_backup_and_rejects_stale_content() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".codex/skills");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.parent().unwrap().join("config.toml");
        std::fs::write(&path, "model = 'a'\n").unwrap();
        let request = |request: Value| {
            let mut child = crate::command("python3")
                .env("HOME", temp.path())
                .args(["-c", include_str!("remote.py")])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            serde_json::to_writer(child.stdin.take().unwrap(), &request).unwrap();
            let out = child.wait_with_output().unwrap();
            serde_json::from_slice::<Value>(&out.stdout).unwrap()
        };
        let initial = request(json!({"action":"text_read","root":root,"file":"config.toml"}));
        let revision = initial["result"]["revision"].as_str().unwrap();
        let saved = request(
            json!({"action":"text_save","root":root,"file":"config.toml","expected":revision,"content":"model = 'b'\n"}),
        );
        assert!(saved.get("error").is_none());
        assert_eq!(
            std::fs::read_to_string(
                temp.path()
                    .join(".harnesskit/comparison-history")
                    .join(revision)
            )
            .unwrap(),
            "model = 'a'\n"
        );
        let stale = request(
            json!({"action":"text_save","root":root,"file":"config.toml","expected":revision,"content":"stale"}),
        );
        assert!(stale.get("error").is_some());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "model = 'b'\n");
    }
    #[test]
    fn listing_excludes_interrupted_disabled_backups() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["real", ".hk-old-interrupted"] {
            let dir = temp.path().join(name);
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(dir.join("SKILL.md.disabled"), "test").unwrap();
        }
        let mut child = crate::command("python3")
            .args(["-c", include_str!("remote.py")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(
            child.stdin.take().unwrap(),
            &json!({"action":"list","root":temp.path()}),
        )
        .unwrap();
        let out = child.wait_with_output().unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["result"], json!(["real"]));
    }
    #[test]
    fn probe_reads_custom_wsl_paths_and_shared_skills_without_changing_settings() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        std::fs::create_dir_all(home.join(".harnesskit")).unwrap();
        std::fs::create_dir_all(home.join(".agents/skills")).unwrap();
        std::fs::create_dir_all(home.join("custom/skills")).unwrap();
        let store = crate::store::Store::open(&home.join(".harnesskit/metadata.db")).unwrap();
        store
            .set_agent_path("codex", Some(home.join("custom").to_str().unwrap()))
            .unwrap();
        let mut child = crate::command("python3")
            .args(["-c", include_str!("remote.py")])
            .env("HOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(child.stdin.take().unwrap(), &json!({"action":"probe"})).unwrap();
        let out = child.wait_with_output().unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        let roots = v["result"]["providers"].as_array().unwrap();
        assert!(roots.contains(&json!(["codex", home.join("custom/skills")])));
        assert!(roots.contains(&json!(["codex", home.join(".agents/skills")])));
    }
    #[test]
    fn helper_matches_native_snapshots_and_preserves_permissions() {
        use std::{collections::BTreeMap, fs};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let folder = crate::environments::folders::Folder::new(BTreeMap::from([
            ("SKILL.md".into(), b"hello\n".to_vec()),
            ("scripts/check.sh".into(), b"#!/bin/sh\n".to_vec()),
        ]));
        crate::environments::folders::replace_folder(
            &root.join("skill"),
            None,
            Some(&folder),
            &root.join("history"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                root.join("skill/scripts/check.sh"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        fs::create_dir(root.join("skill/empty")).unwrap();
        let native = crate::environments::folders::read_folder(&root.join("skill"))
            .unwrap()
            .unwrap();
        let mut child = crate::command("python3")
            .args(["-c", include_str!("remote.py")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(
            child.stdin.take().unwrap(),
            &json!({"action":"read","root":root,"name":"skill"}),
        )
        .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        let remote: crate::environments::folders::Folder =
            serde_json::from_value(value["result"].clone()).unwrap();
        assert_eq!(native, remote);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    #[test]
    #[ignore = "Requires an explicitly provided temporary directory on a live SSH target"]
    fn ssh_round_trip_and_stale_write() {
        use std::collections::BTreeMap;
        let connection = Connection::Ssh {
            host: std::env::var("HK_TEST_SSH_HOST").unwrap(),
            user: std::env::var("HK_TEST_SSH_USER").unwrap(),
            port: 22,
        };
        let root = std::env::var("HK_TEST_SSH_ROOT").unwrap();
        assert!(root.contains("harnesskit-transport-test-"));
        let f = crate::environments::folders::Folder::new(BTreeMap::from([
            ("SKILL.md".into(), b"# Test\n".to_vec()),
            ("assets/test.bin".into(), vec![0, 255, 1]),
        ]));
        run(
            &connection,
            json!({"action":"write","root":root,"name":"probe","expected":null,"folder":f}),
        )
        .unwrap();
        let read: crate::environments::folders::Folder = serde_json::from_value(
            run(
                &connection,
                json!({"action":"read","root":root,"name":"probe"}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(read, f);
        assert!(run(&connection,json!({"action":"write","root":root,"name":"probe","expected":"stale","folder":null})).is_err());
        assert_eq!(
            run(&connection, json!({"action":"list","root":root})).unwrap(),
            json!(["probe"])
        );
        run(&connection,json!({"action":"write","root":root,"name":"probe","expected":f.fingerprint,"folder":null})).unwrap();
        assert_eq!(
            run(&connection, json!({"action":"list","root":root})).unwrap(),
            json!([])
        );
    }
}
