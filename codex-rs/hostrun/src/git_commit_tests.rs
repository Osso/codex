use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::json;

use super::HostrunSession;

#[test]
fn git_status_defaults_to_short_branch_output() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.git.status({ cwd: '/repo' });")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.git");
    assert_eq!(
        approval.summary,
        "Run git -C /repo status --short --branch (stdout text)"
    );
    assert_eq!(
        approval.args,
        json!({
            "program": "git",
            "args": ["-C", "/repo", "status", "--short", "--branch"],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn git_commit_uses_file_stdin_for_message() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "tools.git.commit({
              cwd: '/repo',
              subject: 'Add Hostrun git commit helper',
              bodyLines: [
                'Commit body line one.',
                '',
                'Verification:',
                '- cargo test -p codex-hostrun git_commit_uses_file_stdin_for_message'
              ],
              noVerify: true
            });",
        )
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.git");
    assert_eq!(
        approval.summary,
        "Run git -C /repo commit --file - --no-verify (stdin text)"
    );
    assert_eq!(
        approval.args,
        json!({
            "program": "git",
            "args": [
                "-C",
                "/repo",
                "commit",
                "--file",
                "-",
                "--no-verify"
            ],
            "stdin": {
                "type": "text",
                "text": "Add Hostrun git commit helper\n\nCommit body line one.\n\nVerification:\n- cargo test -p codex-hostrun git_commit_uses_file_stdin_for_message"
            }
        })
    );
}

#[test]
fn git_commit_adds_existing_files_and_excludes_unrelated_staged_by_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    init_repo(dir.path());
    fs::write(dir.path().join("tracked.txt"), "base\n").expect("write tracked");
    git(dir.path(), ["add", "tracked.txt"]);
    git(dir.path(), ["commit", "-m", "Initial commit"]);

    fs::write(dir.path().join("tracked.txt"), "updated\n").expect("update tracked");
    fs::write(dir.path().join("new.txt"), "new\n").expect("write new");
    fs::write(dir.path().join("staged.txt"), "staged\n").expect("write staged");
    git(dir.path(), ["add", "staged.txt"]);

    let session = HostrunSession::new_auto_approve().expect("session");
    let code = format!(
        "tools.git.commit({{
          cwd: {},
          subject: 'Commit selected files',
          files: ['tracked.txt', 'new.txt', 'missing.txt']
        }});",
        serde_json::to_string(dir.path()).expect("cwd JSON")
    );

    let result = session.eval(&code).expect("commit");

    assert_eq!(result.result_type, "completed");
    assert_eq!(
        git_lines(
            dir.path(),
            ["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
        ),
        vec!["new.txt".to_string(), "tracked.txt".to_string()]
    );
    assert_eq!(
        git_lines(dir.path(), ["diff", "--cached", "--name-only"]),
        vec!["staged.txt".to_string()]
    );
}

#[test]
fn git_commit_can_include_existing_staged_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    init_repo(dir.path());
    fs::write(dir.path().join("tracked.txt"), "base\n").expect("write tracked");
    git(dir.path(), ["add", "tracked.txt"]);
    git(dir.path(), ["commit", "-m", "Initial commit"]);

    fs::write(dir.path().join("new.txt"), "new\n").expect("write new");
    fs::write(dir.path().join("staged.txt"), "staged\n").expect("write staged");
    git(dir.path(), ["add", "staged.txt"]);

    let session = HostrunSession::new_auto_approve().expect("session");
    let code = format!(
        "tools.git.commit({{
          cwd: {},
          subject: 'Commit selected and staged files',
          files: ['new.txt'],
          includeStaged: true
        }});",
        serde_json::to_string(dir.path()).expect("cwd JSON")
    );

    let result = session.eval(&code).expect("commit");

    assert_eq!(result.result_type, "completed");
    assert_eq!(
        git_lines(
            dir.path(),
            ["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
        ),
        vec!["new.txt".to_string(), "staged.txt".to_string()]
    );
    assert_eq!(
        git_lines(dir.path(), ["diff", "--cached", "--name-only"]),
        Vec::<String>::new()
    );
}

#[test]
fn git_commit_requires_subject_or_message() {
    let session = HostrunSession::new().expect("session");

    session
        .eval("tools.git.commit({ bodyLines: ['missing subject'] });")
        .expect_err("missing subject should fail");
}

#[test]
fn git_commit_rejects_literal_escaped_newlines() {
    let session = HostrunSession::new().expect("session");

    session
        .eval(
            "tools.git.commit({
              subject: 'Bad commit',
              body: 'line one\\\\nline two'
            });",
        )
        .expect_err("literal escaped newlines should fail");
}

fn init_repo(path: &Path) {
    git(path, ["init"]);
    git(path, ["config", "user.name", "Hostrun Test"]);
    git(path, ["config", "user.email", "hostrun@example.com"]);
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git failed: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_lines<const N: usize>(cwd: &Path, args: [&str; N]) -> Vec<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git failed: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}
