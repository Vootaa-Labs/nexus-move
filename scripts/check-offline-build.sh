#!/usr/bin/env bash
# scripts/check-offline-build.sh — Smoke test for offline package builds.
#
# Validates that:
# 1. The precompiled backend loads .mv files from the example counter package
# 2. Building metadata + manifest from pre-compiled bytecode succeeds
# 3. The verified-compile backend can verify the loaded bytecode
#
# This proves the offline build pipeline works end-to-end without a network
# connection or external compiler toolchain.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Offline Build Smoke Test ==="
echo ""

# Step 1: Run default package tests (includes precompiled backend)
echo "[1/3] Testing precompiled backend..."
cd "$WORKSPACE_ROOT"
cargo test -p nexus-move-package -- --quiet 2>&1
echo "  PASS: precompiled backend tests"

# Step 2: Run verified-compile tests (verifies bytecode with vendored verifier)
echo "[2/3] Testing verified compile backend..."
cargo test -p nexus-move-package --features verified-compile -- --quiet 2>&1
echo "  PASS: verified compile backend tests"

# Step 3: Verify the example counter package has valid structure
echo "[3/3] Checking example counter package..."
COUNTER_DIR="$WORKSPACE_ROOT/examples/counter"

if [ ! -f "$COUNTER_DIR/Move.toml" ]; then
    echo "  FAIL: examples/counter/Move.toml missing"
    exit 1
fi

if [ ! -f "$COUNTER_DIR/nexus-artifact/bytecode/counter.mv" ]; then
    echo "  FAIL: examples/counter/nexus-artifact/bytecode/counter.mv missing"
    exit 1
fi

BYTECODE_SIZE=$(wc -c < "$COUNTER_DIR/nexus-artifact/bytecode/counter.mv" | tr -d ' ')
if [ "$BYTECODE_SIZE" -eq 0 ]; then
    echo "  FAIL: counter.mv is empty"
    exit 1
fi

echo "  PASS: counter package structure valid (counter.mv: ${BYTECODE_SIZE} bytes)"
echo ""
echo "=== All offline build checks passed ==="

