#!/usr/bin/env bash
# scripts/vendor-crate.sh — Copy an upstream Move crate into vendor/ and
# strip dev-dependencies from its Cargo.toml.
#
# Usage:
#   ./scripts/vendor-crate.sh <upstream-dir> <vendor-name>
#
# Example:
#   ./scripts/vendor-crate.sh \
#     ../Nexus_Devnet_0.1.12_Pre/vendor-src/aptos-core/third_party/move/move-core/types \
#     move-core-types
#
# The script copies only src/ (and any nested subdirectories) and Cargo.toml.
# It then strips [dev-dependencies] from the manifest so that vendored crates
# can compile without pulling in compiler-chain or test-only dependencies.

set -euo pipefail

UPSTREAM_DIR="${1:?Usage: vendor-crate.sh <upstream-dir> <vendor-name>}"
VENDOR_NAME="${2:?Usage: vendor-crate.sh <upstream-dir> <vendor-name>}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(dirname "$SCRIPT_DIR")"
TARGET_DIR="${WORKSPACE_ROOT}/vendor/${VENDOR_NAME}"

if [[ ! -d "$UPSTREAM_DIR/src" ]]; then
    echo "ERROR: $UPSTREAM_DIR/src not found" >&2
    exit 1
fi

echo "Vendoring $VENDOR_NAME from $UPSTREAM_DIR"

# Create target and copy source tree
mkdir -p "$TARGET_DIR"
rm -rf "$TARGET_DIR/src"
cp -R "$UPSTREAM_DIR/src" "$TARGET_DIR/src"

# Copy Cargo.toml and strip dev-dependencies
if [[ -f "$UPSTREAM_DIR/Cargo.toml" ]]; then
    # Use awk to remove [dev-dependencies] section
    awk '
        /^\[dev-dependencies\]/ { skip=1; next }
        /^\[/ && skip { skip=0 }
        !skip { print }
    ' "$UPSTREAM_DIR/Cargo.toml" > "$TARGET_DIR/Cargo.toml"
    echo "  Wrote Cargo.toml (dev-dependencies stripped)"
else
    echo "  WARNING: No Cargo.toml found in $UPSTREAM_DIR"
fi

RS_COUNT=$(find "$TARGET_DIR" -name '*.rs' | wc -l | tr -d ' ')
echo "  Copied $RS_COUNT .rs files to vendor/$VENDOR_NAME"
