use std::fs;
use std::path::Path;

use rquickjs::Context;
use serde::Deserialize;

pub(super) fn tmp_resources(context: &Context) -> Vec<HostrunTmpResource> {
    context
        .with(|ctx| {
            ctx.eval::<String, _>("JSON.stringify(globalThis.__hostrun_tmpResources ?? [])")
        })
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub(super) fn remove_tmp_resource(path: &str) {
    let path = Path::new(path);
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    let _ = if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
}

#[derive(Debug, Deserialize)]
pub(super) struct HostrunTmpResource {
    pub(super) path: String,
}
