use std::env;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadStartContributor;
use codex_extension_api::ToolBundle;
use codex_extension_api::ToolContributor;

use crate::HostrunSessionStore;
use crate::HostrunToolConfig;
use crate::hostrun_tool_bundle;
use crate::tool_bundle::hostrun_tool_bundle_with_sessions;

pub const HOSTRUN_RUNNER_ENV: &str = "CODEX_HOSTRUN_RUNNER";
const HOSTRUN_JS_PACKAGE: &str = "@openai/codex-hostrun-js";
const HOSTRUN_INSTRUCTIONS: &str = "\
Hostrun is available through the `hostrun_eval` tool.

Hostrun evaluates synchronous JavaScript in a persistent QuickJS session:
- Do not use `await`. Hostrun helpers return values directly in this runtime.
- `ctx` persists across later `hostrun_eval` calls and across later assistant turns in the same Codex thread. Store scratch results there when later work should reuse them instead of recomputing.
- `console.log`, `console.info`, `console.warn`, `console.error`, and `console.debug` are captured in the tool result.
- Arrays have `.containing(needle)` plus non-mutating helpers such as `.notContaining()`, `.startsWith()`, `.endsWith()`, `.matching()`, `.glob()`, `.unique()`, `.sorted()`, `.reversed()`, `.groupBy()`, `.countBy()`, `.uniqueBy()`, and `.sortBy()`.
- Strings expose shell-style helpers such as `.lines(start, end)`, `.head()`, `.tail()`, `.splitWords()`, `.splitColumn()`, `.cut(separator, fields)`, `.json()`, `.jsonl()`, `.yaml()`, `.toml()`, `.csv()`, `.tsv()`, `.lineCount()`, `.wordCount()`, `.byteCount()`, `.bytes()`, `.byteArray()`, and `.chars()`.
- `path.*` and `date.*` provide small readable helpers for path transforms and UTC date parse/format/humanize workflows.
- `host.cwd()` returns the current Hostrun session cwd, and `host.cd(path)` changes it persistently for later Hostrun calls in the same Codex thread. Relative `fs.*` paths, `cli.*` commands, `run.*` commands, `rg.*`/`fd.*` helpers, stdin files, and output redirects resolve against this cwd.
- `run.<program>(...args)` executes a host command without stdout/stderr capture by default, e.g. `run.dmidecode()` or `run.git('status', '--short')`. `run` is not a shell parser: use `run.dmidecode('-t', 'system')`, not `run('dmidecode -t system')`, and never use `await run(...)`.
- `cli.<program>(...args)` creates a lazy host command builder for workflows that need output capture, stdin, redirects, spawn, piping, or a one-command cwd. Use `.in(path).run()` to execute one command in another directory. Use explicit stream selectors for capture: `cli.ls().text().trim()`, `cli.rclone('lsf', remote).lines()`, or `cli.sh('-c', 'echo err >&2').stderr.text()`. Use `.stdout.capture().stderr.capture().run()` when a probe needs stdout, stderr, and exit status together. Config-only helpers such as `.stdout.toFile(path)`, `.stdout.tee(path)`, `.stderr.toStdout()`, and `.combined.toFile(path)` return the builder so another terminal call can execute it. `.run()` remains available as the low-level builder execution method when a command builder has no terminal output selector.
- `.spawn()` starts a command and returns a managed process handle with `id`, `pid`, `stdout`, `stderr`, `.wait()`, and `.kill()`. Store it in `ctx` if a later Hostrun call should wait or kill it.
- Stream piping is explicit: `const source = cli.rclone('cat', remote); cli.cat().stdin(source.stdout).stdout.text()` returns downstream output plus `commands` status entries for the upstream and downstream commands.
- `fs.write(path, content)`, `fs.read(path)`, `fs.open(path, options)`, `fs.glob(pattern, options)`, `fs.exists(path)`, and `fs.remove(path)` request approval-gated host file operations. `fs.open` parses JSON, JSONL, YAML, TOML, CSV, and TSV from the filename extension unless `options.format` is supplied.
- `sqlite.query(database, sql)` and `kubectl.get(resource, options)` build lazy command wrappers for common JSON-output inspection workflows.
- `rclone.deletefile(target)` requests an approval-gated `rclone deletefile`; `rclone.lsf(target, { recursive: true })` builds a lazy `rclone lsf` command.
- `fd.find(pattern, options)`, `fd.files(root, options)`, and `fd.dirs(root, options)` build lazy `fdfind` commands.
- `rg.search(pattern, paths, options)`, `rg.files(pattern, paths, options)`, and `rg.matches(pattern, paths, options)` build lazy ripgrep commands.
- `http.get/post/put/patch/delete/head(url, options)` and `http.request(method, url, options)` build approval-gated HTTP requests. Use `.json()`, `.text()`, `.bytes()`, `.save(path)`, or `.run()` to choose response handling.
- Prefer Hostrun over shell loops for HTTP polling, retries, and response parsing. Example:
  `for (let i = 0; i < 30; i++) { const html = http.get(url, { headers: { 'User-Agent': 'Mozilla/5.0' }, tls: { acceptInvalidCerts: true } }).text(); const tag = html.match(/<script type=\"module\" src=\"[^\"]*bundle[^\"]*\"/)?.[0] ?? ''; if (tag.includes('globalcomix-frontend.nyc3.cdn')) { tag; break; } run.sleep('2'); }`
- `tools.sudo(commandBuilder)` wraps a `cli.*` command builder with `authsudo` for privileged commands. Example: `tools.sudo(cli.dmidecode('-t', 'system')).run()`. Its `.run()` captures stdout and stderr by default unless the wrapped builder already configured streams. `cli.sudo(...)` and `run.sudo(...)` still invoke the `sudo` binary literally.
- `tools.github.createPR(options)` creates GitHub pull requests through `gh pr create` with the PR body sent via `--body-file -` stdin. Prefer `bodyLines: [...]` or a template literal `body` so Markdown newlines are real newlines; literal `\\n` sequences are rejected by default. Common options: `repo`, `base`, `head`, `title`, `body`, `bodyLines`, `draft`, `labels`, `reviewers`, `assignees`, `projects`, and `milestone`.
- `tools.git.commit(options)` creates commits through `git commit --file -` with the commit message sent via stdin. Prefer `subject` plus `bodyLines: [...]` or a template literal `body`; literal `\\n` sequences are rejected by default. Common options: `cwd`, `subject`/`message`, `body`, `bodyLines`, `paths`/`files`, `includeStaged`, `all`, `amend`, `noEdit`, `allowEmpty`, `noVerify`, and `signoff`. Listed `paths`/`files` that exist are added before committing. `includeStaged` defaults to false, so unrelated staged files are excluded unless explicitly requested.

Return a final expression value when useful.";

#[derive(Clone, Debug)]
pub struct HostrunToolContributor {
    runner: PathBuf,
}

impl HostrunToolContributor {
    pub fn new(runner: impl AsRef<Path>) -> Self {
        Self {
            runner: runner.as_ref().to_path_buf(),
        }
    }
}

impl ToolContributor for HostrunToolContributor {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<ToolBundle> {
        vec![hostrun_tool_bundle(HostrunToolConfig::new(&self.runner))]
    }
}

pub fn install<C>(registry: &mut ExtensionRegistryBuilder<C>, runner: impl AsRef<Path>) {
    registry.tool_contributor(Arc::new(HostrunToolContributor::new(runner)));
}

pub fn install_from_env<C>(registry: &mut ExtensionRegistryBuilder<C>) {
    let Some(runner) = env::var_os(HOSTRUN_RUNNER_ENV) else {
        return;
    };
    install(registry, PathBuf::from(runner));
}

pub fn install_managed<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
) -> Result<PathBuf, HostrunRunnerLifecycleError> {
    let runner = if let Some(runner) = env::var_os(HOSTRUN_RUNNER_ENV) {
        PathBuf::from(runner)
    } else {
        HostrunRunnerLifecycle::managed_package().ensure_runner()?
    };
    install(registry, &runner);
    Ok(runner)
}

pub fn install_feature_gated<C>(registry: &mut ExtensionRegistryBuilder<C>, enabled: fn(&C) -> bool)
where
    C: Send + Sync + 'static,
{
    let contributor = Arc::new(HostrunFeatureGatedContributor { enabled });
    registry.thread_start_contributor(contributor.clone());
    registry.prompt_contributor(contributor.clone());
    registry.tool_contributor(contributor);
}

struct HostrunFeatureGatedContributor<C> {
    enabled: fn(&C) -> bool,
}

impl<C> ThreadStartContributor<C> for HostrunFeatureGatedContributor<C>
where
    C: 'static,
{
    fn contribute(&self, config: &C, _session_store: &ExtensionData, thread_store: &ExtensionData) {
        if !(self.enabled)(config) {
            thread_store.insert(HostrunFeatureState::disabled());
            return;
        }

        thread_store.insert(HostrunFeatureState::enabled());
    }
}

impl<C> ToolContributor for HostrunFeatureGatedContributor<C>
where
    C: Send + Sync + 'static,
{
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<ToolBundle> {
        if !hostrun_enabled(thread_store) {
            return Vec::new();
        }

        let sessions = thread_store
            .get_or_init(|| std::sync::Mutex::new(HostrunSessionStore::new_auto_approve()));
        vec![hostrun_tool_bundle_with_sessions(sessions)]
    }
}

impl<C> ContextContributor for HostrunFeatureGatedContributor<C>
where
    C: Send + Sync + 'static,
{
    fn contribute(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<PromptFragment> {
        if !hostrun_enabled(thread_store) {
            return Vec::new();
        }

        vec![PromptFragment::developer_capability(HOSTRUN_INSTRUCTIONS)]
    }
}

fn hostrun_enabled(thread_store: &ExtensionData) -> bool {
    thread_store
        .get::<HostrunFeatureState>()
        .is_some_and(|state| state.enabled)
}

#[derive(Clone, Debug)]
struct HostrunFeatureState {
    enabled: bool,
}

impl HostrunFeatureState {
    fn disabled() -> Self {
        Self { enabled: false }
    }

    fn enabled() -> Self {
        Self { enabled: true }
    }
}

#[derive(Clone, Debug)]
pub struct HostrunRunnerLifecycle {
    workspace_root: PathBuf,
    package_dir: PathBuf,
}

impl HostrunRunnerLifecycle {
    pub fn new(workspace_root: impl AsRef<Path>, package_dir: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            package_dir: package_dir.as_ref().to_path_buf(),
        }
    }

    pub fn managed_package() -> Self {
        let hostrun_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = hostrun_crate
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| hostrun_crate.clone());
        Self::new(workspace_root, hostrun_crate.join("js"))
    }

    pub fn runner_path(&self) -> PathBuf {
        self.package_dir.join("dist").join("cli.js")
    }

    pub fn ensure_runner(&self) -> Result<PathBuf, HostrunRunnerLifecycleError> {
        let runner = self.runner_path();
        if runner.is_file() {
            return Ok(runner);
        }

        self.build_runner()?;
        if runner.is_file() {
            return Ok(runner);
        }

        Err(HostrunRunnerLifecycleError::RunnerMissingAfterBuild { path: runner })
    }

    fn build_runner(&self) -> Result<(), HostrunRunnerLifecycleError> {
        let output = Command::new("npx")
            .args(["pnpm", "--filter", HOSTRUN_JS_PACKAGE, "build"])
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|source| HostrunRunnerLifecycleError::BuildSpawnFailed { source })?;

        if output.status.success() {
            return Ok(());
        }

        Err(HostrunRunnerLifecycleError::BuildFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

#[derive(Debug)]
pub enum HostrunRunnerLifecycleError {
    BuildSpawnFailed { source: std::io::Error },
    BuildFailed { status: String, stderr: String },
    RunnerMissingAfterBuild { path: PathBuf },
}

impl fmt::Display for HostrunRunnerLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildSpawnFailed { source } => {
                write!(formatter, "failed to start Hostrun JS build: {source}")
            }
            Self::BuildFailed { status, stderr } => {
                write!(formatter, "Hostrun JS build failed with {status}: {stderr}")
            }
            Self::RunnerMissingAfterBuild { path } => {
                write!(
                    formatter,
                    "Hostrun JS build finished but runner is missing at {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for HostrunRunnerLifecycleError {}

#[cfg(test)]
mod tests {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ToolContributor;
    use codex_tool_api::ToolCall;
    use serde_json::json;
    use tempfile::TempDir;

    use super::HostrunRunnerLifecycle;
    use super::HostrunToolContributor;
    use super::install;
    use super::install_feature_gated;

    #[test]
    fn contributor_returns_hostrun_eval_bundle() {
        let contributor = HostrunToolContributor::new("/tmp/hostrun-runner");

        let tools = contributor.tools(&ExtensionData::new(), &ExtensionData::new());

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_name(), "hostrun_eval");
    }

    #[test]
    fn install_adds_hostrun_tool_contributor() {
        let mut builder = ExtensionRegistryBuilder::<()>::new();
        install(&mut builder, "/tmp/hostrun-runner");
        let registry = builder.build();

        let tools =
            registry.tool_contributors()[0].tools(&ExtensionData::new(), &ExtensionData::new());

        assert_eq!(tools[0].tool_name(), "hostrun_eval");
    }

    #[test]
    fn managed_lifecycle_uses_existing_built_runner() {
        let temp_dir = TempDir::new().expect("temp dir");
        let workspace_root = temp_dir.path().join("repo");
        let package_dir = workspace_root.join("codex-rs").join("hostrun").join("js");
        let runner = package_dir.join("dist").join("cli.js");
        std::fs::create_dir_all(runner.parent().expect("runner parent")).expect("create dist");
        std::fs::write(&runner, "#!/usr/bin/env node\n").expect("write runner");
        let lifecycle = HostrunRunnerLifecycle::new(&workspace_root, &package_dir);

        assert_eq!(lifecycle.ensure_runner().expect("runner exists"), runner);
    }

    #[test]
    fn feature_gated_install_hides_hostrun_when_disabled() {
        let mut builder = ExtensionRegistryBuilder::<bool>::new();
        install_feature_gated(&mut builder, |enabled| *enabled);
        let registry = builder.build();
        let session_store = ExtensionData::new();
        let thread_store = ExtensionData::new();

        registry.thread_start_contributors()[0].contribute(&false, &session_store, &thread_store);
        let tools = registry.tool_contributors()[0].tools(&session_store, &thread_store);

        assert!(tools.is_empty());
        let fragments =
            registry.context_contributors()[0].contribute(&session_store, &thread_store);
        assert!(fragments.is_empty());
    }

    #[test]
    fn feature_gated_install_contributes_tool_and_instructions_when_enabled() {
        let mut builder = ExtensionRegistryBuilder::<bool>::new();
        install_feature_gated(&mut builder, |enabled| *enabled);
        let registry = builder.build();
        let session_store = ExtensionData::new();
        let thread_store = ExtensionData::new();

        registry.thread_start_contributors()[0].contribute(&true, &session_store, &thread_store);
        let tools = registry.tool_contributors()[0].tools(&session_store, &thread_store);
        let fragments =
            registry.context_contributors()[0].contribute(&session_store, &thread_store);

        assert_eq!(tools[0].tool_name(), "hostrun_eval");
        assert_eq!(fragments.len(), 1);
        assert!(fragments[0].text().contains("ctx"));
        assert!(fragments[0].text().contains("across later assistant turns"));
        assert!(fragments[0].text().contains("run.dmidecode()"));
        assert!(fragments[0].text().contains("fs.read(path)"));
        assert!(fragments[0].text().contains("rclone.lsf(target"));
        assert!(fragments[0].text().contains("fd.find(pattern"));
        assert!(fragments[0].text().contains("rg.search(pattern"));
        assert!(fragments[0].text().contains("http.get/post"));
        assert!(
            fragments[0]
                .text()
                .contains("Prefer Hostrun over shell loops")
        );
        assert!(fragments[0].text().contains("acceptInvalidCerts"));
        assert!(!fragments[0].text().contains("tools.fs.write"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn feature_gated_tools_reuse_hostrun_session_store_across_tool_assembly() {
        let mut builder = ExtensionRegistryBuilder::<bool>::new();
        install_feature_gated(&mut builder, |enabled| *enabled);
        let registry = builder.build();
        let session_store = ExtensionData::new();
        let thread_store = ExtensionData::new();

        registry.thread_start_contributors()[0].contribute(&true, &session_store, &thread_store);
        let first_tools = registry.tool_contributors()[0].tools(&session_store, &thread_store);
        let first = first_tools[0]
            .executor()
            .execute(tool_call(
                "ctx.hostrun_probe = 'working'; ctx.hostrun_probe;",
            ))
            .await
            .expect("first eval");
        let second_tools = registry.tool_contributors()[0].tools(&session_store, &thread_store);
        let second = second_tools[0]
            .executor()
            .execute(tool_call("ctx.hostrun_probe;"))
            .await
            .expect("second eval");

        assert_eq!(first["value"], json!("working"));
        assert_eq!(second["value"], json!("working"));
    }

    fn tool_call(code: &str) -> ToolCall {
        ToolCall {
            call_id: "call-hostrun".to_string(),
            arguments: json!({ "code": code }).to_string(),
        }
    }
}
