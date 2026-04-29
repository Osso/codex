#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_root="$repo_root/codex-rs"
install_root="${CODEX_INSTALL_ROOT:-$HOME/.cargo}"
bin_dir="$install_root/bin"
codex_bin_path="$bin_dir/codex"

cd "$workspace_root"

cargo install \
  --path ./cli \
  --bin codex \
  --root "$install_root" \
  --locked \
  --force

echo "Deployed codex to $codex_bin_path"
"$codex_bin_path" --version
