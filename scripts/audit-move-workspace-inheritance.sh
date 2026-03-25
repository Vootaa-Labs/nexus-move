#!/usr/bin/env sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../../Nexus_Devnet_0.1.12_Pre/vendor-src/aptos-core/third_party/move" && pwd)"

if [ ! -d "$ROOT" ]; then
  echo "expected Move vendor tree not found: $ROOT" >&2
  exit 1
fi

TARGETS="
move-core/types/Cargo.toml
move-binary-format/Cargo.toml
move-bytecode-verifier/Cargo.toml
move-vm/types/Cargo.toml
move-vm/runtime/Cargo.toml
tools/move-package/Cargo.toml
move-compiler-v2/Cargo.toml
move-command-line-common/Cargo.toml
move-model/Cargo.toml
"

search_manifest() {
  file="$1"
  pattern='workspace *= *true|workspace\.package|workspace\.dependencies'

  if command -v rg >/dev/null 2>&1; then
    rg -n "$pattern" "$file" || true
  else
    grep -nE "$pattern" "$file" || true
  fi
}

echo "# Workspace inheritance audit"
echo "# Root: $ROOT"

for rel in $TARGETS; do
  file="$ROOT/$rel"
  if [ ! -f "$file" ]; then
    echo
    echo "## Missing: $rel"
    continue
  fi

  echo
  echo "## $rel"
  search_manifest "$file"
done
