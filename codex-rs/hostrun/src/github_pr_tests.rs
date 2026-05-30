use serde_json::json;

use super::HostrunSession;

#[test]
fn github_pr_view_defaults_to_common_json_fields() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.github.prView({ repo: 'Globalcomix/gc', pr: 789 });")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.gh");
    assert_eq!(
        approval.summary,
        "Run gh pr view 789 --repo Globalcomix/gc --json number,title,url,headRefName,baseRefName,state,mergeable,reviewDecision,statusCheckRollup (stdout text)"
    );
    assert_eq!(
        approval.args,
        json!({
            "program": "gh",
            "args": [
                "pr",
                "view",
                "789",
                "--repo",
                "Globalcomix/gc",
                "--json",
                "number,title,url,headRefName,baseRefName,state,mergeable,reviewDecision,statusCheckRollup"
            ],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn github_pr_view_accepts_field_overrides() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.github.prView({ pr: 789, fields: ['headRefName', 'baseRefName'] });")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "gh",
            "args": ["pr", "view", "789", "--json", "headRefName,baseRefName"],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn github_create_pr_uses_body_file_stdin() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "tools.github.createPR({
              repo: 'Globalcomix/gc',
              base: 'master',
              head: 'ad-hostrun-pr-helper',
              title: 'Add Hostrun PR helper',
              bodyLines: [
                'Adds a safe PR helper.',
                '',
                'Verification:',
                '- cargo test -p codex-hostrun github_create_pr_uses_body_file_stdin'
              ],
              labels: ['automation', 'hostrun'],
              draft: true
            });",
        )
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.gh");
    assert_eq!(
        approval.summary,
        "Run gh pr create --repo Globalcomix/gc --base master --head ad-hostrun-pr-helper --title Add Hostrun PR helper --body-file - --draft --label automation --label hostrun (stdin text)"
    );
    assert_eq!(
        approval.args,
        json!({
            "program": "gh",
            "args": [
                "pr",
                "create",
                "--repo",
                "Globalcomix/gc",
                "--base",
                "master",
                "--head",
                "ad-hostrun-pr-helper",
                "--title",
                "Add Hostrun PR helper",
                "--body-file",
                "-",
                "--draft",
                "--label",
                "automation",
                "--label",
                "hostrun"
            ],
            "stdin": {
                "type": "text",
                "text": "Adds a safe PR helper.\n\nVerification:\n- cargo test -p codex-hostrun github_create_pr_uses_body_file_stdin"
            }
        })
    );
}

#[test]
fn github_create_pr_rejects_literal_escaped_newlines() {
    let session = HostrunSession::new().expect("session");

    session
        .eval(
            "tools.github.createPR({
              title: 'Bad body',
              body: 'line one\\\\nline two'
            });",
        )
        .expect_err("literal escaped newlines should fail");
}
