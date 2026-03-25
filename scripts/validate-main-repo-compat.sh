#!/usr/bin/env bash
# validate-main-repo-compat.sh — Cross-workspace compatibility validation.
#
# Verifies that nexus-move can serve as a drop-in replacement for the
# main Nexus repo's Move VM integration points.  Run from nexus-move/.
#
# Usage:
#   ./scripts/validate-main-repo-compat.sh [MAIN_REPO_DIR]
#
# If MAIN_REPO_DIR is omitted, defaults to ../Nexus_Devnet_0.1.12_Pre.

set -euo pipefail

NEXUS_MOVE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MAIN_REPO="${1:-$(cd "$NEXUS_MOVE_DIR/.." && pwd)/Nexus_Devnet_0.1.12_Pre}"

echo "nexus-move dir: $NEXUS_MOVE_DIR"
echo "main repo dir:  $MAIN_REPO"
echo ""

PASS=0
FAIL=0
SKIP=0

check() {
    local desc="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        echo "  ✓ $desc"
        PASS=$((PASS + 1))
    else
        echo "  ✗ $desc"
        FAIL=$((FAIL + 1))
    fi
}

skip() {
    echo "  – $1 (skipped)"
    SKIP=$((SKIP + 1))
}

# ── 1. nexus-move workspace health ────────────────────────────────────

echo "=== 1. nexus-move workspace health ==="
cd "$NEXUS_MOVE_DIR"

NEXUS_PKGS="-p nexus-move-types -p nexus-move-bytecode -p nexus-move-stdlib -p nexus-move-runtime -p nexus-move-package"

check "cargo check (default features)" cargo check --workspace
check "cargo check (vm-backend)" cargo check -p nexus-move-runtime --features vm-backend
check "default tests pass" cargo test $NEXUS_PKGS
check "vm-backend tests pass" cargo test --features vm-backend $NEXUS_PKGS
check "cross-repo compat tests pass" cargo test -p nexus-move-runtime --features vm-backend --test cross_repo_compat

echo ""

# ── 2. Vendor path availability ───────────────────────────────────────

echo "=== 2. Vendor crate paths ==="

VENDOR_CRATES=(
    "move-core-types"
    "move-binary-format"
    "move-bytecode-verifier"
    "move-vm-runtime"
    "move-vm-types"
    "move-vm-metrics"
    "move-borrow-graph"
    "move-bytecode-spec"
)

for crate in "${VENDOR_CRATES[@]}"; do
    if [ -f "$NEXUS_MOVE_DIR/vendor/$crate/Cargo.toml" ]; then
        check "vendor/$crate exists" true
    else
        check "vendor/$crate exists" false
    fi
done

echo ""

# ── 3. Main repo structure check ─────────────────────────────────────

echo "=== 3. Main repo integration files ==="

if [ ! -d "$MAIN_REPO" ]; then
    skip "main repo not found at $MAIN_REPO"
else
    MAIN_FILES=(
        "crates/nexus-execution/src/move_adapter/aptos_vm.rs"
        "crates/nexus-execution/src/move_adapter/stdlib.rs"
        "crates/nexus-execution/src/move_adapter/mod.rs"
        "tools/nexus-wallet/src/move_tooling/commands/build.rs"
    )

    for f in "${MAIN_FILES[@]}"; do
        if [ -f "$MAIN_REPO/$f" ]; then
            check "$f present" true
        else
            skip "$f not found"
        fi
    done

    # Check that main repo counter.mv matches nexus-move's
    MAIN_COUNTER="$MAIN_REPO/contracts/examples/counter/nexus-artifact/bytecode/counter.mv"
    NM_COUNTER="$NEXUS_MOVE_DIR/examples/counter/nexus-artifact/bytecode/counter.mv"

    if [ -f "$MAIN_COUNTER" ] && [ -f "$NM_COUNTER" ]; then
        # Both exist — check magic bytes match (first 4 bytes)
        MAIN_MAGIC=$(xxd -l 4 -p "$MAIN_COUNTER")
        NM_MAGIC=$(xxd -l 4 -p "$NM_COUNTER")
        if [ "$MAIN_MAGIC" = "$NM_MAGIC" ]; then
            check "counter.mv magic bytes match" true
        else
            check "counter.mv magic bytes match ($MAIN_MAGIC vs $NM_MAGIC)" false
        fi
    else
        skip "counter.mv comparison (one or both not found)"
    fi
fi

echo ""

# ── 4. Feature compatibility ─────────────────────────────────────────

echo "=== 4. Feature flag compatibility ==="

check "stdlib vm-backend compiles" cargo check -p nexus-move-stdlib --features vm-backend
check "native-compile compiles" cargo check --features native-compile

echo ""

# ── Summary ──────────────────────────────────────────────────────────

echo "=== Summary ==="
echo "  Passed:  $PASS"
echo "  Failed:  $FAIL"
echo "  Skipped: $SKIP"

if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo "⚠  Some checks failed. Review output above."
    exit 1
fi

echo ""
echo "All checks passed."
