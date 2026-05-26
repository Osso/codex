use serde_json::json;

use super::HostrunSession;

#[test]
fn path_helpers_cover_join_basename_dirname_and_parse() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            ({
              joined: path.join("/tmp", "hostrun", "probe.txt"),
              relativeJoin: path.join("tmp", "hostrun", "", "probe.txt"),
              basename: path.basename("/tmp/hostrun/probe.txt"),
              dirname: path.dirname("/tmp/hostrun/probe.txt"),
              rootDirname: path.dirname("/probe.txt"),
              relativeDirname: path.dirname("probe.txt"),
              parsed: path.parse("/tmp/hostrun/probe.txt"),
              noExtension: path.parse("README")
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "joined": "/tmp/hostrun/probe.txt",
            "relativeJoin": "tmp/hostrun/probe.txt",
            "basename": "probe.txt",
            "dirname": "/tmp/hostrun",
            "rootDirname": "/",
            "relativeDirname": ".",
            "parsed": {
                "root": "/",
                "dir": "/tmp/hostrun",
                "base": "probe.txt",
                "name": "probe",
                "ext": ".txt"
            },
            "noExtension": {
                "root": "",
                "dir": ".",
                "base": "README",
                "name": "README",
                "ext": ""
            }
        }))
    );
}
