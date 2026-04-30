use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use crate::GitToolingError;
use crate::operations::ensure_git_repository;
use crate::operations::resolve_repository_root;
use crate::operations::run_git_for_status;

const WORKTREE_BASE_REF: &str = "origin/master";

pub fn create_or_reuse_codex_worktree(
    base_dir: &Path,
    name: &str,
) -> Result<PathBuf, GitToolingError> {
    ensure_git_repository(base_dir)?;
    validate_worktree_name(base_dir, name)?;

    let worktree_path = codex_worktree_path(base_dir, name)?;
    if worktree_path.exists() {
        return canonicalize_or_original(worktree_path);
    }

    add_worktree(base_dir, name, &worktree_path)?;
    canonicalize_or_original(worktree_path)
}

fn codex_worktree_path(base_dir: &Path, name: &str) -> Result<PathBuf, GitToolingError> {
    let repo_root = resolve_repository_root(base_dir)?;
    let repo_name =
        repo_root
            .file_name()
            .ok_or_else(|| GitToolingError::RepositoryRootHasNoName {
                path: repo_root.clone(),
            })?;
    let worktree_dir_name = format!("{}-{name}", repo_name.to_string_lossy());
    Ok(repo_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(worktree_dir_name))
}

fn add_worktree(base_dir: &Path, name: &str, worktree_path: &Path) -> Result<(), GitToolingError> {
    if branch_exists(base_dir, name)? {
        run_git_for_status(
            base_dir,
            [
                OsString::from("worktree"),
                OsString::from("add"),
                worktree_path.as_os_str().to_os_string(),
                OsString::from(name),
            ],
            /*env*/ None,
        )?;
    } else {
        run_git_for_status(
            base_dir,
            [
                OsString::from("worktree"),
                OsString::from("add"),
                OsString::from("-b"),
                OsString::from(name),
                worktree_path.as_os_str().to_os_string(),
                OsString::from(WORKTREE_BASE_REF),
            ],
            /*env*/ None,
        )?;
    }

    Ok(())
}

fn validate_worktree_name(base_dir: &Path, name: &str) -> Result<(), GitToolingError> {
    if name.trim().is_empty() {
        return Err(GitToolingError::InvalidWorktreeName {
            name: name.to_string(),
        });
    }

    match run_git_for_status(
        base_dir,
        [
            OsString::from("check-ref-format"),
            OsString::from("--branch"),
            OsString::from(name),
        ],
        /*env*/ None,
    ) {
        Ok(()) => Ok(()),
        Err(GitToolingError::GitCommand { .. }) => Err(GitToolingError::InvalidWorktreeName {
            name: name.to_string(),
        }),
        Err(err) => Err(err),
    }
}

fn branch_exists(base_dir: &Path, name: &str) -> Result<bool, GitToolingError> {
    match run_git_for_status(
        base_dir,
        [
            OsString::from("show-ref"),
            OsString::from("--verify"),
            OsString::from("--quiet"),
            OsString::from(format!("refs/heads/{name}")),
        ],
        /*env*/ None,
    ) {
        Ok(()) => Ok(true),
        Err(GitToolingError::GitCommand { .. }) => Ok(false),
        Err(err) => Err(err),
    }
}

fn canonicalize_or_original(path: PathBuf) -> Result<PathBuf, GitToolingError> {
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(path),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn creates_sibling_worktree_from_origin_master() {
        let fixture = git_repo_with_origin_master();

        let worktree_path =
            create_or_reuse_codex_worktree(&fixture.repo, "feature-a").expect("create worktree");

        assert_eq!(
            worktree_path,
            fixture
                .temp
                .path()
                .join("repo-feature-a")
                .canonicalize()
                .unwrap()
        );
        assert_eq!(
            git_stdout(&worktree_path, ["branch", "--show-current"]),
            "feature-a"
        );
        assert_eq!(
            git_stdout(&worktree_path, ["rev-parse", "HEAD"]),
            fixture.head
        );
    }

    #[test]
    fn reuses_existing_worktree_directory() {
        let fixture = git_repo_with_origin_master();
        let worktree_path =
            create_or_reuse_codex_worktree(&fixture.repo, "feature-a").expect("create worktree");

        let reused =
            create_or_reuse_codex_worktree(&fixture.repo, "feature-a").expect("reuse worktree");

        assert_eq!(reused, worktree_path);
    }

    #[test]
    fn rejects_invalid_worktree_name() {
        let fixture = git_repo_with_origin_master();

        let err = create_or_reuse_codex_worktree(&fixture.repo, "not valid")
            .expect_err("invalid branch name should fail");

        assert!(matches!(err, GitToolingError::InvalidWorktreeName { .. }));
    }

    struct GitFixture {
        temp: TempDir,
        repo: PathBuf,
        head: String,
    }

    fn git_repo_with_origin_master() -> GitFixture {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        std::fs::create_dir(&repo).expect("create repo dir");
        git(&repo, ["init", "-b", "master"]);
        git(&repo, ["config", "user.name", "Codex Test"]);
        git(&repo, ["config", "user.email", "codex@example.test"]);
        std::fs::write(repo.join("README.md"), "hello\n").expect("write readme");
        git(&repo, ["add", "README.md"]);
        git(&repo, ["commit", "-m", "initial"]);
        git(&repo, ["remote", "add", "origin", "."]);
        let head = git_stdout(&repo, ["rev-parse", "HEAD"]);
        git(&repo, ["update-ref", "refs/remotes/origin/master", &head]);

        GitFixture { temp, repo, head }
    }

    fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("utf8 stdout")
            .trim()
            .to_string()
    }
}
