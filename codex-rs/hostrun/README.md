# Hostrun

Hostrun is a persistent QuickJS runtime for readable host-side automation in Codex and Claude Code.

It is meant to replace ad hoc shell snippets when JavaScript control flow, structured parsing, persistent scratch state, or approval-readable host capabilities are clearer than Bash.

Codex-specific integration lives in `codex-hostrun-adapter`, not this crate.
That adapter maps Hostrun output into Codex's native exec/progress display. MCP
clients, including Claude Code, see Hostrun through MCP logging/progress
notifications instead.

## Claude Code MCP

Build the MCP server binary:

```sh
cargo build -p codex-hostrun --bin hostrun-mcp
```

Add it to Claude Code as a local stdio MCP server:

```sh
claude mcp add --scope user hostrun -- /path/to/hostrun-mcp
```

For a local development checkout, point Claude at Cargo:

```sh
claude mcp add --scope user hostrun -- cargo run -p codex-hostrun --bin hostrun-mcp
```

Then verify it:

```sh
claude mcp list
```

Inside Claude Code, `/mcp` shows the server and its tool. Claude Code's MCP
documentation describes local stdio servers as commands after the `--`
separator and notes that user-scoped servers are available across projects:
https://code.claude.com/docs/en/mcp

The standalone MCP server defaults to pending approval for host operations.
Calls such as filesystem writes, command execution, HTTP requests, and remote
mutations return structured `needs_approval` results instead of executing
automatically.

## Standalone Repository Extraction

This crate has a local standalone package proof in `/home/osso/Repos/hostrun`:

- Runtime and MCP code live here, without `codex-extension-api` or
  `codex-tool-api` dependencies.
- Codex-specific tool contribution and native exec/progress display mapping live
  in `codex-hostrun-adapter`.
- `hostrun-mcp` is the standalone stdio MCP binary. `codex-hostrun-mcp` remains
  as a compatibility alias while Hostrun still lives in the Codex workspace.

Remaining lift-out work:

- Publish or otherwise finalize the standalone Hostrun repository location.
- Keep `codex-hostrun-adapter` in Codex and point it at the standalone Hostrun
  crate or repository.
- Preserve focused verification: standalone `cargo test`, `cargo build --bin
  hostrun-mcp`, and Codex adapter progress-display tests.

## Runtime

Hostrun evaluates synchronous JavaScript. Do not use `await`.

Each session keeps `ctx` alive across later `hostrun_eval` calls, so expensive or useful intermediate results can be stored and reused:

```js
ctx.files = rg.files('Hostrun', ['codex-rs/hostrun']).lines();
ctx.files.length;
```

Console calls are captured in the result:

```js
console.log('checking publish bundle');
'done';
```

## Working Directory

Use `host.cd(path)` to change the persistent session cwd. Relative `fs`, `cli`, `run`, `rg`, `fd`, stdin files, and output redirects resolve against it.

```js
host.cd('/syncthing/Sync/Projects/globalcomix/gc');
host.cwd();
```

Prefer `host.cd()` over repeating `-C` or `cd ... && ...` in shell snippets.

## Commands

`run.<program>(...args)` executes a command without stdout/stderr capture by default:

```js
run.git('status', '--short');
```

`cli.<program>(...args)` builds a command when output capture, stdin, redirects, spawning, or piping is needed:

```js
cli.git('status', '--short').stdout.text();
cli.ls('-la').lines();
cli.echo('hello').in('/tmp').run();
cli.sh('-c', 'printf out; printf err >&2')
  .stdout.capture()
  .stderr.capture()
  .run();
```

`run` is not a shell parser:

```js
// Wrong
run('git status --short');

// Right
run.git('status', '--short');
```

There is no `.complete()` command-builder method. Use explicit stream selectors or `.stdout.capture().stderr.capture().run()`.

Never use `Bash(...)` for ad hoc command composition, pipes, loops, command substitution, parsing, retries, or multi-line workflows when Hostrun is available. Capture command output and use JavaScript for filtering/counting/sorting:

```js
const secret = kubectl.get('secret', {
  name: 'ipg-import',
  namespace: 'ops'
}).json();
const decode = (value) => cli.base64('-d').stdin.text(value).text().trim();
const remote = `:s3,provider=DigitalOcean,access_key_id=${decode(secret.data.DO_SPACES_ACCESS_KEY)},secret_access_key=${decode(secret.data.DO_SPACES_SECRET_KEY)},endpoint=nyc3.digitaloceanspaces.com:globalcomix-publisher-uploads`;
const listing = cli.rclone('lsf', `${remote}/bookwire/content/`)
  .lines()
  .filter((line) => !line.includes('cached'));

({
  feedFiles: listing.filter((line) => /\.(xml|onix)$/i.test(line)),
  total: listing.length,
  recent: cli.rclone('lsl', `${remote}/bookwire/content/`)
    .lines()
    .filter((line) => !line.includes('cached'))
    .sort()
    .slice(-3)
});
```

Use `.in(path)` for a one-command cwd without mutating the persistent Hostrun session cwd:

```js
cli.git('status', '--short').in('/repo').run();
cli.git('status', '--short').in('/repo').stdout.text();
```

## Privileged Commands

Use `tools.sudo(commandBuilder)` for privileged commands. It wraps a `cli.*` command builder with `authsudo`.

```js
tools.sudo(cli.dmidecode('-t', 'system')).run();
```

`tools.sudo(...).run()` captures stdout and stderr by default unless the wrapped builder already configured streams.

```js
tools.sudo(cli.ls('/root')).run();
tools.sudo(cli.dmidecode('-t', 'system').stdout.capture()).run();
```

`cli.sudo(...)` and `run.sudo(...)` invoke the `sudo` binary literally. They do not use `authsudo`.

## SSH Commands

`tools.ssh(options)` wraps OpenSSH for remote `cli.*` command builders. `.run(command)` captures stdout and stderr by default:

```js
const router = tools.ssh({
  host: 'router',
  user: 'root',
  password: 'none',
  passwordMode: 'plain'
});

router.run(cli.hostname());
router.cli(cli.cat('/etc/os-release')).text();
```

For Windows remotes, use `tools.powershell(script, options)` to build a PowerShell command with `-EncodedCommand` so paths with spaces do not require nested shell escaping:

```js
const desktop = tools.ssh({ host: 'desktop' });
desktop.run(tools.powershell("Test-Path 'C:\\World of Warcraft\\_retail_\\Interface\\AddOns'"));
```

Password auth is only enabled when `passwordMode: 'plain'` is explicit. That mode uses `sshpass -e` and redacts `SSHPASS` from approval metadata; it is meant for intentionally non-secret passwords such as `none`.

## Files

Filesystem helpers are approval-gated:

```js
fs.read('Cargo.toml');
fs.write('notes.txt', 'hello\n');
fs.exists('src/bootstrap.js');
fs.glob('src/**/*_tests.rs');
fs.open('config.toml');
```

`fs.open()` parses JSON, JSONL, YAML, TOML, CSV, and TSV from the filename extension unless an explicit format is passed.

## File Editing Helpers

Use `tools.file.replace()` for small exact edits. It requires exactly one match by default, so ambiguous replacements fail before writing:

```js
tools.file.replace('README.md', { from: 'old text', to: 'new text' });
tools.file.replace('README.md', 'old text', 'new text');
tools.file.replace('README.md', { from: 'old', to: 'new', all: true });
tools.file.replace('README.md', { from: 'old', to: 'new', occurrence: 2 });
```

Use `tools.file.patch()` for unified diffs. Pass a full diff with file headers, or pass the path separately when the patch only contains hunks:

```js
tools.file.patch(`--- a/notes.txt
+++ b/notes.txt
@@ -1,2 +1,2 @@
 alpha
-old
+new
`);

tools.file.patch('notes.txt', `@@ -1,2 +1,2 @@
 alpha
-old
+new
`);
```

## HTTP

Use `http.get/post/put/patch/delete/head(url, options)` or `http.request(method, url, options)`.

`.text()`, `.json()`, and `.bytes()` return the response body directly. `.run()` returns the structured response with status, headers, byte count, and body metadata.

```js
http.get('https://example.com/api', {
  headers: { Accept: 'application/json' },
  retries: 2
}).json();

http.get('https://example.com/').text().slice(0, 120);
http.get('https://example.com/').run().status;
```

Use `http.session(options)` when multiple requests share a base URL, headers, TLS options, or cookies:

```js
ctx.unifi = http.session({
  baseUrl: `https://${cfg.host}`,
  headers: { 'X-API-Key': cfg.api_key },
  tls: { acceptInvalidCerts: true }
});

ctx.unifi.get('/proxy/network/api/s/default/rest/portforward').json();
ctx.unifi.cookies;
```

HTTP sessions store cookies from `Set-Cookie` response headers and send them on later requests through the session. The cookie jar is a visible object at `.cookies`.

Prefer Hostrun over shell loops for HTTP polling, retries, and response parsing:

```js
const url = 'https://publish.globalcomix.com/';

for (let i = 0; i < 30; i++) {
  const html = http.get(url, {
    headers: { 'User-Agent': 'Mozilla/5.0' },
    tls: { acceptInvalidCerts: true }
  }).text();

  const tag = html.match(/<script type="module" src="[^"]*bundle[^"]*"/)?.[0] ?? '';
  if (tag.includes('globalcomix-frontend.nyc3.cdn')) {
    tag;
    break;
  }

  run.sleep('2');
}
```

## Git Helpers

`tools.git.status({ cwd })` returns `git status --short --branch` text:

```js
tools.git.status({ cwd: '/syncthing/Sync/Projects/globalcomix/gc' });
```

`tools.git.commit(options)` creates commits with the message sent through `git commit --file -`.

```js
tools.git.commit({
  cwd: '/syncthing/Sync/Projects/globalcomix/gc',
  subject: 'Wire publish frontend version',
  bodyLines: [
    'Updates publish default template to use the deployed frontend bundle.',
    '',
    'Verification:',
    '- checked publish page script tag'
  ],
  files: [
    'apps/publish/content/publishdefault.php'
  ]
});
```

Listed `files` or `paths` that exist are added before committing. Missing listed files are ignored.

By default `includeStaged` is false, so unrelated staged files are excluded. Set `includeStaged: true` when already-staged changes should be included:

```js
tools.git.commit({
  cwd,
  subject: 'Commit selected and staged files',
  files: ['src/bootstrap.js'],
  includeStaged: true
});
```

Literal `\n` sequences in commit messages are rejected by default. Use `bodyLines` or a template literal for multiline text.

## GitHub PR Helper

`tools.github.prView({ repo, pr, fields })` returns parsed JSON from `gh pr view --json ...`:

```js
tools.github.prView({
  repo: 'Globalcomix/gc',
  pr: 789,
  fields: ['headRefName', 'baseRefName', 'state', 'mergeable']
});
```

`tools.github.runView({ repo, run, fields })` returns parsed JSON from `gh run view --json ...`:

```js
tools.github.runView({
  repo: 'Globalcomix/gc',
  run: 26680417560,
  fields: ['status', 'conclusion', 'jobs', 'url', 'headSha']
});
```

`tools.github.createPR(options)` creates pull requests through `gh pr create` and sends the body through stdin:

```js
tools.github.createPR({
  repo: 'openai/codex',
  base: 'main',
  head: 'hostrun-readme',
  title: 'Document Hostrun usage',
  bodyLines: [
    'Adds operational Hostrun examples.',
    '',
    'Tested with `cargo test -p codex-hostrun`.'
  ]
});
```

## Browser Helper

`tools.browser` wraps `browser-cli` for Chrome DevTools Protocol automation. Chrome must be available with remote debugging; `browser-cli` can auto-start it in the usual local setup.

Browser helpers return command builders, so actions use `.run()` and reads use `.text()`:

```js
tools.browser.open('https://example.com').run();
tools.browser.get('title').text();
tools.browser.text('main');
tools.browser.click('button[type=submit]').run();
tools.browser.fill('input[name=email]', 'user@example.com').run();
tools.browser.press('Enter').run();
```

Snapshots, screenshots, JavaScript eval, waits, and tabs are also exposed:

```js
tools.browser.snapshot({ mini: true, interactive: true }).text();
tools.browser.screenshot('/tmp/page.jpg', { full: true }).run();
tools.browser.eval('document.title').text();
tools.browser.exceptions({ reload: true }).json();
tools.browser.console({ reload: true, waitMs: 3000 }).json();
tools.browser.wait('main').run();
tools.browser.tabs.list().text();
tools.browser.tabs.switch(0).run();
```

Use `tools.browser.command(...args)` for browser-cli subcommands that do not yet have a typed helper:

```js
tools.browser.command('get', 'attr', 'a.primary', 'href').text();
```

## Search And Data Helpers

Ripgrep and fd helpers build lazy commands:

```js
rg('tools.sudo', ['codex-rs/hostrun/src']).lines();
rg.files('Hostrun', ['codex-rs/hostrun']).lines();
fd.files('codex-rs/hostrun/src').lines();
```

Strings and arrays include small data-shaping helpers:

```js
cli.git('status', '--short').stdout.text().lines();
ctx.files.containing('hostrun').sorted();
```

## MCP Server

The crate builds the Claude Code MCP server binary:

```bash
cargo build --release -p codex-hostrun --bin hostrun-mcp
```

Claude Code can register the built binary as the `hostrun` MCP server. The server exposes the `hostrun_eval` tool and contributes the runtime instructions above.

## Tests

Run Hostrun tests from `codex-rs`:

```bash
cargo test -p codex-hostrun
```

After Rust changes:

```bash
just fmt
just fix -p codex-hostrun
```
