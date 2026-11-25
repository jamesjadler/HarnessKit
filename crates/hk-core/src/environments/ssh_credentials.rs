//! Password SSH. Registry contains only an opaque credential reference and pinned host key.
use super::transport::Connection;
use crate::HkError;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    time::Duration,
};

fn error(message: &str) -> HkError {
    HkError::Network(message.into())
}
fn vault_error() -> HkError {
    error(
        "Secure credential storage is unavailable or locked. Unlock the system keyring; in WSL enable Windows PowerShell interoperability. No plaintext password was saved. SSH key / agent authentication is also available.",
    )
}
#[derive(Serialize, Deserialize)]
struct Secret {
    host: String,
    user: String,
    port: u16,
    host_key: String,
    password: String,
}
fn entry(id: &str) -> Result<keyring::Entry, HkError> {
    uuid::Uuid::parse_str(id).map_err(|_| error("Invalid credential reference"))?;
    keyring::Entry::new("HarnessKit SSH", id).map_err(|_| vault_error())
}
fn is_wsl() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WSL_DISTRO_NAME").is_some()
            || std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .is_ok_and(|s| s.to_lowercase().contains("microsoft")))
}
fn blob_path(id: &str) -> Result<PathBuf, HkError> {
    uuid::Uuid::parse_str(id).map_err(|_| error("Invalid credential reference"))?;
    Ok(dirs::home_dir()
        .ok_or_else(vault_error)?
        .join(".harnesskit/ssh-credentials")
        .join(format!("{id}.dpapi")))
}
// Only encrypted DPAPI blobs touch disk. Cleartext travels over anonymous pipes, never argv/env.
fn dpapi(input: &[u8], protect: bool) -> Result<Vec<u8>, HkError> {
    let script = if protect {
        "$ErrorActionPreference='Stop'; [Console]::InputEncoding=[Text.UTF8Encoding]::new($false); Add-Type -AssemblyName System.Security; $inputBytes=[Text.Encoding]::UTF8.GetBytes([Console]::In.ReadToEnd()); $blob=[Security.Cryptography.ProtectedData]::Protect($inputBytes,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser); [Console]::Out.Write([Convert]::ToBase64String($blob))"
    } else {
        "$ErrorActionPreference='Stop'; [Console]::InputEncoding=[Text.UTF8Encoding]::new($false); Add-Type -AssemblyName System.Security; $blob=[Convert]::FromBase64String([Console]::In.ReadToEnd()); $plain=[Security.Cryptography.ProtectedData]::Unprotect($blob,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser); [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); [Console]::Out.Write([Text.Encoding]::UTF8.GetString($plain))"
    };
    let mut child = crate::command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| vault_error())?;
    child
        .stdin
        .take()
        .ok_or_else(vault_error)?
        .write_all(input)
        .map_err(|_| vault_error())?;
    let mut stdout = child.stdout.take().ok_or_else(vault_error)?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = std::time::Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(vault_error());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = child.wait().map_err(|_| vault_error())?;
    let output = reader
        .join()
        .map_err(|_| vault_error())?
        .map_err(|_| vault_error())?;
    if !status.success() {
        return Err(vault_error());
    }
    Ok(output)
}
fn store(id: &str, secret: &Secret) -> Result<(), HkError> {
    let text = serde_json::to_string(secret)?;
    if is_wsl() {
        let blob = dpapi(text.as_bytes(), true)?;
        let path = blob_path(id)?;
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&blob)?;
        file.as_file().sync_all()?;
        file.persist_noclobber(path).map_err(|_| vault_error())?;
        Ok(())
    } else {
        entry(id)?.set_password(&text).map_err(|_| vault_error())
    }
}
fn load(id: &str) -> Result<Secret, HkError> {
    let text = if is_wsl() {
        dpapi(
            &std::fs::read(blob_path(id)?).map_err(|_| vault_error())?,
            false,
        )?
    } else {
        entry(id)?
            .get_password()
            .map_err(|_| vault_error())?
            .into_bytes()
    };
    serde_json::from_slice(&text).map_err(|_| vault_error())
}
pub fn forget(id: &str) {
    if is_wsl() {
        if let Ok(p) = blob_path(id) {
            let _ = std::fs::remove_file(p);
        }
    } else if let Ok(e) = entry(id) {
        let _ = e.delete_credential();
    }
}
fn handshake(host: &str, port: u16) -> Result<(ssh2::Session, String), HkError> {
    if host.is_empty()
        || host.len() > 253
        || host.starts_with('-')
        || host
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | '\0'))
        || port == 0
    {
        return Err(error(
            "Enter a hostname or IP address and a port from 1 to 65535. Password connections do not use SSH aliases.",
        ));
    }
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| error("Cannot resolve SSH hostname"))?;
    let stream=addresses.take(4).find_map(|a|TcpStream::connect_timeout(&a,Duration::from_secs(5)).ok()).ok_or_else(||error("Connection refused or timed out. Enable Remote Login / SSH on the target and check its address."))?;
    let mut session = ssh2::Session::new().map_err(|_| error("Cannot initialize SSH"))?;
    session.set_timeout(15000);
    session.set_tcp_stream(stream);
    session
        .handshake()
        .map_err(|_| error("SSH handshake failed"))?;
    let (key, _) = session
        .host_key()
        .ok_or_else(|| error("Server did not provide an SSH host key"))?;
    let fingerprint = format!("SHA256:{}", STANDARD_NO_PAD.encode(Sha256::digest(key)));
    Ok((session, fingerprint))
}
fn verify_key(actual: &str, expected: &str) -> Result<(), HkError> {
    if expected.is_empty() {
        return Err(error(&format!("Confirm SSH host fingerprint: {actual}")));
    }
    if actual != expected {
        return Err(error(
            "SSH host key changed. Connection blocked; verify the target's identity before reconnecting.",
        ));
    }
    Ok(())
}
fn authenticate(session: &ssh2::Session, user: &str, password: &str) -> Result<(), HkError> {
    if user.trim().is_empty() || password.is_empty() || password.len() > 4096 {
        return Err(error(
            "SSH username and password are required (password maximum 4096 bytes).",
        ));
    }
    session.userauth_password(user,password).map_err(|_|error("SSH authentication failed. Check the username/password and that password login is allowed on the target."))
}
fn request(session: &ssh2::Session, input: Value) -> Result<Value, HkError> {
    let script = include_str!("remote.py");
    let mut channel = session
        .channel_session()
        .map_err(|_| error("Cannot open SSH channel"))?;
    channel
        .handle_extended_data(ssh2::ExtendedData::Merge)
        .map_err(|_| error("Cannot configure SSH channel"))?;
    channel
        .exec(&format!("python3 -c '{}'", script.replace('\'', "'\\''")))
        .map_err(|_| error("Cannot run remote helper; Python 3 is required."))?;
    channel.write_all(&serde_json::to_vec(&input)?)?;
    channel.send_eof().map_err(|_| error("SSH input failed"))?;
    let mut output = Vec::new();
    (&mut channel)
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut output)?;
    if output.len() > 64 * 1024 * 1024 {
        return Err(error("SSH response exceeds size limit"));
    }
    channel
        .wait_close()
        .map_err(|_| error("SSH channel did not close"))?;
    if channel.exit_status().unwrap_or(1) != 0 {
        return Err(error(
            "Remote helper failed. Ensure Python 3 is installed and accessible over SSH.",
        ));
    }
    let response: Value = serde_json::from_slice(&output)
        .map_err(|_| error("Invalid remote response; check Python 3 and shell startup output."))?;
    if let Some(e) = response.get("error") {
        return Err(HkError::Conflict(e.to_string()));
    }
    Ok(response["result"].clone())
}
pub fn connect(
    host: String,
    user: String,
    port: u16,
    password: String,
    host_key: String,
) -> Result<Connection, HkError> {
    // Verify identity before sending credentials, and verify remote helper before saving them.
    let (session, actual) = handshake(&host, port)?;
    verify_key(&actual, &host_key)?;
    authenticate(&session, &user, &password)?;
    request(&session, serde_json::json!({"action":"probe"}))?;
    let id = uuid::Uuid::new_v4().to_string();
    store(
        &id,
        &Secret {
            host: host.clone(),
            user: user.clone(),
            port,
            host_key: actual.clone(),
            password,
        },
    )?;
    Ok(Connection::SshPassword {
        host,
        user,
        port,
        credential: id,
        host_key: actual,
    })
}
pub fn run(
    host: &str,
    user: &str,
    port: u16,
    credential: &str,
    host_key: &str,
    input: Value,
) -> Result<Value, HkError> {
    let secret = load(credential)?;
    if secret.host != host
        || secret.user != user
        || secret.port != port
        || secret.host_key != host_key
    {
        return Err(error("Saved credential does not match this SSH connection"));
    }
    let (session, actual) = handshake(host, port)?;
    verify_key(&actual, host_key)?;
    authenticate(&session, user, &secret.password)?;
    request(&session, input)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_key_requires_consent_and_rejects_changes() {
        assert!(
            verify_key("SHA256:abc", "")
                .unwrap_err()
                .to_string()
                .contains("Confirm SSH host fingerprint")
        );
        assert!(verify_key("SHA256:abc", "SHA256:other").is_err());
        assert!(verify_key("SHA256:abc", "SHA256:abc").is_ok());
    }
    #[test]
    fn registry_serialization_contains_reference_not_password() {
        let c = Connection::SshPassword {
            host: "mac".into(),
            user: "demonlord".into(),
            port: 22,
            credential: uuid::Uuid::new_v4().to_string(),
            host_key: "SHA256:key".into(),
        };
        let v = serde_json::to_value(c).unwrap();
        assert!(v.get("password").is_none());
        assert!(v.get("credential").is_some());
    }
    #[test]
    fn rejects_invalid_credential_paths() {
        assert!(blob_path("../../secret").is_err());
    }
}
