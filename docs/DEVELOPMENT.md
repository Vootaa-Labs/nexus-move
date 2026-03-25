# Development

## Toolchain

- Rust toolchain: `1.85.0` from `rust-toolchain.toml`
- Required components: `rustfmt`, `clippy`, `rust-src`, `llvm-tools-preview`
- Primary entry points: `Makefile` targets and the scripts under `scripts/`

## Workspace Model

The workspace contains two categories of crates:

- first-party `nexus-move-*` crates, which define the supported public surface and the default validation targets
- vendored upstream Move crates under `vendor/`, which are pinned to a reviewed upstream snapshot and kept as workspace members so dependency edges remain explicit

The vendored crates intentionally do not carry their full upstream `dev-dependencies` surface. That keeps the repository boundary narrow and avoids pulling unrelated upstream test infrastructure into the default Nexus workflow.

## rust-analyzer

The workspace includes `.vscode/settings.json` that limits rust-analyzer checks to the first-party crates:

- `nexus-move-types`
- `nexus-move-bytecode`
- `nexus-move-runtime`
- `nexus-move-stdlib`
- `nexus-move-package`

This reduces diagnostics from vendored test and proptest targets whose upstream `dev-dependencies` are intentionally not part of the repository contract.

## Recommended Commands

Fast local loop:

- `make check`
- `make clippy`
- `make test`

Feature-gated validation:

- `make test-vm-backend`
- `make test-cross-repo`
- `make check-verified-compile`
- `make check-native-compile`

Artifact and compatibility checks:

- `make smoke-offline-build`
- `make validate-compat`

## Example Artifacts

`examples/counter/` contains committed `nexus-artifact/` output. Those files are not incidental build leftovers; they are used by the offline build smoke test to verify artifact layout and bytecode loading behavior.

## When To Touch vendor/

Only update vendored crates when one of these is true:

- a compatibility or security issue requires a reviewed freeze refresh
- a first-party crate needs an upstream Move capability that is in scope for this repository boundary
- dependency narrowing work is being done deliberately and documented in `docs/DEPENDENCY_FREEZE.md`

Routine application work should stay inside the five first-party crates.