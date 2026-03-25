# Dependency Freeze

## Runtime Set

Planned frozen runtime crates:

- `move-core-types`
- `move-binary-format`
- `move-bytecode-verifier`
- `move-vm-types`
- `move-vm-runtime`

## Compiler Set

Planned frozen compiler crates:

- `move-package`
- `move-compiler-v2`
- `legacy-move-compiler`
- `move-model`
- `move-command-line-common`

## Tracking Fields

Each frozen upstream crate should eventually record:

- source repository and commit
- retained path within the upstream tree
- reason it is required
- risk if updated
- future replacement or narrowing target

## Known Non-Goals For Initial Freeze

The current main workspace also exposes many additional Move crates, including `move-prover`, `move-docgen`, `move-cli`, `move-bytecode-viewer`, `move-coverage`, and testing infrastructure. These remain out of scope for the first `nexus-move` extraction boundary unless a later phase proves they are needed.
