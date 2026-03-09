use dirs::config_dir;
use dirs::home_dir;
use std::path::PathBuf;

/// Returns the path to the Codex configuration directory, which can be
/// specified by the `CODEX_HOME` environment variable. If not set, defaults to
/// the preferred XDG config directory and falls back to `~/.codex`.
///
/// - If `CODEX_HOME` is set, the value must exist and be a directory. The
///   value will be canonicalized and this function will Err otherwise.
/// - If `CODEX_HOME` is not set, this function does not verify that the
///   directory exists.
pub fn find_codex_home() -> std::io::Result<PathBuf> {
    let codex_home_env = std::env::var("CODEX_HOME")
        .ok()
        .filter(|val| !val.is_empty());
    find_codex_home_from_env(codex_home_env.as_deref())
}

fn find_codex_home_from_env(codex_home_env: Option<&str>) -> std::io::Result<PathBuf> {
    // Honor the `CODEX_HOME` environment variable when it is set to allow users
    // (and tests) to override the default location.
    match codex_home_env {
        Some(val) => {
            let path = PathBuf::from(val);
            let metadata = std::fs::metadata(&path).map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("CODEX_HOME points to {val:?}, but that path does not exist"),
                ),
                _ => std::io::Error::new(
                    err.kind(),
                    format!("failed to read CODEX_HOME {val:?}: {err}"),
                ),
            })?;

            if !metadata.is_dir() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("CODEX_HOME points to {val:?}, but that path is not a directory"),
                ))
            } else {
                path.canonicalize().map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!("failed to canonicalize CODEX_HOME {val:?}: {err}"),
                    )
                })
            }
        }
        None => {
            let xdg_codex = config_dir().map(|mut path| {
                path.push("codex");
                path
            });
            let legacy_codex = home_dir().map(|mut path| {
                path.push(".codex");
                path
            });

            match (xdg_codex, legacy_codex) {
                (Some(xdg_codex), Some(legacy_codex)) => {
                    if xdg_codex.exists() {
                        Ok(xdg_codex)
                    } else if legacy_codex.exists() {
                        Ok(legacy_codex)
                    } else {
                        Ok(xdg_codex)
                    }
                }
                (Some(xdg_codex), None) => Ok(xdg_codex),
                (None, Some(legacy_codex)) => Ok(legacy_codex),
                (None, None) => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find home directory",
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::find_codex_home_from_env;
    use dirs::config_dir;
    use dirs::home_dir;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    #[test]
    fn find_codex_home_env_missing_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let missing = temp_home.path().join("missing-codex-home");
        let missing_str = missing
            .to_str()
            .expect("missing codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(missing_str)).expect_err("missing CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(
            err.to_string().contains("CODEX_HOME"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_file_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let file_path = temp_home.path().join("codex-home.txt");
        fs::write(&file_path, "not a directory").expect("write temp file");
        let file_str = file_path
            .to_str()
            .expect("file codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(file_str)).expect_err("file CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("not a directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_valid_directory_canonicalizes() {
        let temp_home = TempDir::new().expect("temp home");
        let temp_str = temp_home
            .path()
            .to_str()
            .expect("temp codex home path should be valid utf-8");

        let resolved = find_codex_home_from_env(Some(temp_str)).expect("valid CODEX_HOME");
        let expected = temp_home
            .path()
            .canonicalize()
            .expect("canonicalize temp home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_uses_default_home_dir() {
        let resolved = find_codex_home_from_env(None).expect("default CODEX_HOME");
        let mut expected = config_dir().expect("config dir");
        expected.push("codex");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_prefers_existing_legacy_dir_when_xdg_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let home = tmp.path().join("home");
        let config = tmp.path().join("config");
        let legacy = home.join(".codex");
        fs::create_dir_all(&legacy).expect("create legacy codex dir");
        fs::create_dir_all(&config).expect("create config dir");

        let home_before = std::env::var_os("HOME");
        let xdg_before = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", &config);

        let resolved = find_codex_home_from_env(None).expect("resolve codex home");

        if let Some(value) = home_before {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(value) = xdg_before {
            std::env::set_var("XDG_CONFIG_HOME", value);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        assert_eq!(resolved, legacy);
    }
}
