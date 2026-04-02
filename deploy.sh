#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_root="$repo_root/codex-rs"
install_root="${CODEX_INSTALL_ROOT:-$HOME/.cargo}"
bin_dir="$install_root/bin"
codex_bin_path="$bin_dir/codex"
mcp_bin_path="$bin_dir/codex-mcp-server"

cd "$workspace_root"

cargo install \
  --path ./cli \
  --bin codex \
  --root "$install_root" \
  --locked \
  --force

cargo install \
  --path ./mcp-server \
  --bin codex-mcp-server \
  --root "$install_root" \
  --locked \
  --force

echo "Deployed codex to $codex_bin_path"
"$codex_bin_path" --version
echo "Deployed codex-mcp-server to $mcp_bin_path"

cat <<EOF

Use the installed Codex binary for MCP with either of these config forms:

[mcp_servers.codex]
command = "$codex_bin_path"
args = ["mcp-server"]

[mcp_servers.codex-standalone]
command = "$mcp_bin_path"
EOF
