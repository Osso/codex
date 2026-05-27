use serde_json::json;

use super::HostrunSession;

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
              paths: [
                'codex-rs/hostrun/src/bootstrap.js',
                'codex-rs/hostrun/src/git_commit_tests.rs'
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
        "Run git -C /repo commit --file - --no-verify -- codex-rs/hostrun/src/bootstrap.js codex-rs/hostrun/src/git_commit_tests.rs (stdin text)"
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
                "--no-verify",
                "--",
                "codex-rs/hostrun/src/bootstrap.js",
                "codex-rs/hostrun/src/git_commit_tests.rs"
            ],
            "stdin": {
                "type": "text",
                "text": "Add Hostrun git commit helper\n\nCommit body line one.\n\nVerification:\n- cargo test -p codex-hostrun git_commit_uses_file_stdin_for_message"
            }
        })
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
