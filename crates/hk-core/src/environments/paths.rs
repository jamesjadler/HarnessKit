//! Resolve Windows/WSL aliases only in the local machine context.
use std::path::PathBuf;
#[derive(Clone, Copy)]
pub enum Runtime {
    Windows,
    Wsl,
    Other,
}
pub fn spelling(path: &str, runtime: Runtime) -> String {
    if matches!(runtime, Runtime::Other) {
        return path.into();
    }
    let p = path.replace('\\', "/");
    if p.len() >= 3 && p.as_bytes()[0].is_ascii_alphabetic() && &p[1..3] == ":/" {
        let drive = p[..1].to_ascii_uppercase();
        return match runtime {
            Runtime::Wsl => format!("/mnt/{}/{}", drive.to_ascii_lowercase(), &p[3..]),
            _ => format!("{drive}:/{}", &p[3..]),
        };
    }
    if matches!(runtime, Runtime::Windows) {
        if let Some(rest) = p.strip_prefix("/mnt/") {
            if rest.len() >= 1
                && rest.as_bytes()[0].is_ascii_alphabetic()
                && (rest.len() == 1 || rest.as_bytes()[1] == b'/')
            {
                return format!(
                    "{}:/{}",
                    rest[..1].to_ascii_uppercase(),
                    rest.get(2..).unwrap_or("")
                );
            }
        }
        if let Some(rest) = p.strip_prefix("//wsl$/") {
            return format!("//wsl.localhost/{rest}");
        }
    }
    path.into()
}
pub fn local(path: &str) -> PathBuf {
    let runtime = if cfg!(windows) {
        Runtime::Windows
    } else if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        Runtime::Wsl
    } else {
        Runtime::Other
    };
    if matches!(runtime, Runtime::Wsl) && path.len() > 2 && path.as_bytes()[1] == b':' {
        if let Some(translated) = super::windows_host::translate(path) {
            return PathBuf::from(translated);
        }
    }
    PathBuf::from(spelling(path, runtime))
}
pub fn for_connection(path: &str, connection: &super::transport::Connection) -> String {
    match connection {
        super::transport::Connection::Local => local(path).to_string_lossy().into_owned(),
        super::transport::Connection::Wsl { .. } => spelling(path, Runtime::Wsl),
        super::transport::Connection::SshPassword { .. }
        | super::transport::Connection::Ssh { .. }
        | super::transport::Connection::Docker { .. } => path.into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drive_aliases_resolve_in_both_directions() {
        assert_eq!(
            spelling(r"C:\Users\example-user\Code", Runtime::Wsl),
            "/mnt/c/Users/example-user/Code"
        );
        assert_eq!(
            spelling("/mnt/d/My Files/skills", Runtime::Windows),
            "D:/My Files/skills"
        );
        assert_eq!(
            spelling(r"c:\Users\demon", Runtime::Windows),
            "C:/Users/demon"
        );
    }
    #[test]
    fn linux_and_remote_paths_are_not_conflated() {
        assert_eq!(
            spelling("/home/demon/skills", Runtime::Windows),
            "/home/demon/skills"
        );
        assert_eq!(spelling("/mnt/c/skills", Runtime::Other), "/mnt/c/skills");
        assert_eq!(spelling("C:relative", Runtime::Wsl), "C:relative");
        assert_eq!(
            spelling("/mnt/cache/skills", Runtime::Windows),
            "/mnt/cache/skills"
        );
    }
    #[test]
    fn wsl_unc_spellings_share_an_identity_on_windows() {
        assert_eq!(
            spelling(r"\\wsl$\Ubuntu\home\demon", Runtime::Windows),
            "//wsl.localhost/Ubuntu/home/demon"
        );
    }
}
