#!/usr/bin/env bash
# Compile nursery Move modules (guid.move, event.move) into .mv bytecode
# and place them in the framework directory for embedding.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FRAMEWORK_DIR="$REPO_ROOT/crates/nexus-move-stdlib/src/framework"
NURSERY_DIR="$REPO_ROOT/stdlib/nursery/sources"

echo "=== Compiling nursery modules ==="
echo "  Sources:  $NURSERY_DIR"
echo "  Target:   $FRAMEWORK_DIR"

# Use the native compile integration test to generate bytecode
cd "$REPO_ROOT"
cargo test -p nexus-move-package --features native-compile \
    -- native_backend::tests::generate_nursery_bytecode --exact --ignored 2>&1

echo ""
echo "Checking output..."
for mod in guid event; do
    if [[ -f "$FRAMEWORK_DIR/$mod.mv" ]]; then
        sz=$(wc -c < "$FRAMEWORK_DIR/$mod.mv" | tr -d ' ')
        echo "  ✅ $mod.mv  ($sz bytes)"
    else
        echo "  ❌ $mod.mv  MISSING"
        exit 1
    fi
done
echo ""
echo "=== Done ==="
