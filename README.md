# nexus-move

`nexus-move` is the standalone Move subsystem for Nexus.

It packages the Nexus Move runtime facade, bytecode and verifier facade, frozen stdlib snapshot, package frontend, and the audited vendored Move crates required to build and execute Move contracts with offline-compatible workflows.

## What This Repository Provides

- `nexus-move-runtime`: publish, call, query, gas, event, and storage bridge APIs
- `nexus-move-bytecode`: structural verification and bundle preflight policy
- `nexus-move-stdlib`: embedded framework modules and native function registry
- `nexus-move-package`: package inspection, artifact generation, verified compile, and native compile backends
- `vendor/`: frozen upstream Move crates pinned for reproducible Nexus integration

## Scope

This repository will own:

- the Nexus Move runtime facade
- the Nexus bytecode and verifier facade
- the frozen stdlib snapshot and native function registry
- the Nexus package frontend and artifact semantics
- the audited subset of upstream Move crates required for build and execution

This repository will not own:

- consensus orchestration
- RPC server assembly
- node lifecycle and devnet orchestration
- Nexus storage backends
- top-level transaction routing outside the Move boundary

## Workspace Layout

```text
nexus-move/
  crates/
    nexus-move-types/
    nexus-move-bytecode/
    nexus-move-runtime/
    nexus-move-stdlib/
    nexus-move-package/
  docs/
  scripts/
  examples/
  stdlib/
  vendor/
```

## Current Capabilities

- real Move VM backend behind the `vm-backend` feature
- offline-compatible package builds using precompiled, verified, or native compile backends
- embedded framework modules at `0x1` with native function registration
- compatibility-preserving artifact output under `nexus-artifact/`
- upgrade-policy enforcement and ABI hashing for compatible upgrades
- example package artifacts for smoke testing and regression coverage

## Crates

- `nexus-move-types`: shared public types used by the other facade crates
- `nexus-move-bytecode`: bytecode policy, verification surface, and publish preflight helpers
- `nexus-move-runtime`: execution facade, VM backends, gas metering, state bridge, and publish/query APIs
- `nexus-move-stdlib`: embedded stdlib modules and native registrations
- `nexus-move-package`: package frontend, artifact orchestration, and compile backend selection

## Feature Flags

- `vm-backend`: enables the real vendored Move VM backend in `nexus-move-runtime` and native registrations in `nexus-move-stdlib`
- `verified-compile`: enables bytecode deserialization and verification during package builds
- `native-compile`: enables compilation through vendored `move-compiler-v2`
- `bootstrap-vendor`: enables the temporary subprocess bootstrap backend for compatibility checks

## Development Commands

- `make fmt`
- `make fmt-check`
- `make clippy`
- `make test`
- `make test-vm-backend`
- `make test-cross-repo`
- `make smoke-offline-build`
- `make validate-compat`

## Validation Entry Points

### `cargo test` driven checks

- `make test` runs the default unit and integration tests for the first-party `nexus-move-*` crates.
- `make test-vm-backend` runs the runtime and stdlib tests that require the real Move VM backend.
- `make test-cross-repo` runs `crates/nexus-move-runtime/tests/cross_repo_compat.rs`, which checks storage keys, metadata encoding, stdlib compatibility, and other in-repo cross-version invariants.
- `make check-native-compile` and `make test-native-compile` validate the vendored compiler-v2 path.

### Script-driven checks

- `make smoke-offline-build` wraps `scripts/check-offline-build.sh`. This script combines `cargo test` runs for the package backends with filesystem assertions on the prebuilt example artifact, so it validates the offline build path rather than only Rust test logic.
- `make validate-compat` wraps `scripts/validate-main-repo-compat.sh`. This script orchestrates a broader compatibility sweep: workspace `cargo check` and `cargo test` under multiple feature sets, vendor path presence checks, and optional comparisons against the adjacent main Nexus repository layout and `counter.mv` artifact.

## Tooling Notes

- Vendored upstream crates remain workspace members so the dependency graph stays explicit, but the default validation surface focuses on the first-party `nexus-move-*` crates.
- Workspace settings in `.vscode/settings.json` restrict rust-analyzer checks to the first-party crates to avoid diagnostics from stripped vendored test targets.
- `Cargo.lock` is committed to make the frozen dependency surface reproducible in CI and for downstream consumers.

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): repository boundary, crate responsibilities, runtime and stdlib architecture
- [docs/DEPENDENCY_FREEZE.md](docs/DEPENDENCY_FREEZE.md): frozen upstream dependency policy and exclusions
- [docs/FACADE_MAPPING.md](docs/FACADE_MAPPING.md): public facade and integration surface mapping
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md): local development workflow, rust-analyzer behavior, and validation commands
- [docs/RELEASE.md](docs/RELEASE.md): repository publishing and release checklist

## Repository Status

This repository is intended to be published and maintained as the canonical GitHub home for the Nexus Move subsystem. The codebase, lockfile, CI workflow, vendored dependencies, and example artifacts are kept in-repo so the default developer path and the public repository state stay aligned.
