# nexus-move

Move smart-contract subsystem for [Nexus](https://github.com/vootaa-labs/nexus-node). Provides runtime execution, bytecode verification, stdlib, and package tooling as a standalone dependency.

**Version**: 0.1.1 · **Rust**: 1.85.0 · **License**: Apache-2.0

## Quick Start (Consumer)

In your workspace `Cargo.toml`:

```toml
[dependencies]
nexus-move-runtime  = { git = "https://github.com/vootaa-labs/nexus-move", tag = "v0.1.1" }
nexus-move-types    = { git = "https://github.com/vootaa-labs/nexus-move", tag = "v0.1.1" }
nexus-move-bytecode = { git = "https://github.com/vootaa-labs/nexus-move", tag = "v0.1.1" }
nexus-move-package  = { git = "https://github.com/vootaa-labs/nexus-move", tag = "v0.1.1" }
```

Enable the real Move VM backend:

```toml
nexus-move-runtime = { git = "...", tag = "v0.1.1", features = ["vm-backend"] }
```

> **Rule**: Consumer crates depend only on the five `nexus-move-*` facade crates. Never depend on `vendor/` crates directly.

## Crates

| Crate | Role |
|---|---|
| `nexus-move-types` | Shared public types (`VmOutput`, `FunctionCall`, `UpgradePolicy`, …) |
| `nexus-move-bytecode` | Bytecode policy, structural verification, publish preflight |
| `nexus-move-runtime` | Execution facade, VM backends, gas metering, state bridge, upstream re-exports |
| `nexus-move-stdlib` | Embedded `0x1` framework modules (11 modules) and native function registry |
| `nexus-move-package` | Package frontend, artifact generation, compile backend selection |

## Feature Flags

| Flag | Scope | Effect |
|---|---|---|
| `vm-backend` | runtime, stdlib | Enables real Move VM execution, native functions, and `upstream` re-export module |
| `verified-compile` | package | Bytecode deserialization + verification during package builds |
| `native-compile` | package | Compilation via vendored `move-compiler-v2` |
| `bootstrap-vendor` | package | Subprocess bootstrap backend for compatibility checks |

## upstream Re-export Module

When `vm-backend` is enabled, `nexus_move_runtime::upstream` re-exports 40+ types from four vendored crates (`move-core-types`, `move-binary-format`, `move-vm-runtime`, `move-vm-types`), mirroring upstream module paths:

```rust
use nexus_move_runtime::upstream::move_core_types::account_address::AccountAddress;
use nexus_move_runtime::upstream::move_vm_runtime::move_vm::MoveVM;
```

This is the **only** sanctioned import path for upstream Move types in consumer code.

## Layout

```text
nexus-move/
  crates/           5 first-party facade crates
  vendor/           20 frozen upstream Move crates (workspace members)
  stdlib/           Move source for framework modules
  examples/         Example packages with committed artifacts
  scripts/          Validation and audit scripts
  docs/             Architecture, development, and release docs
```

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — crate topology, VM backends, gas model, stdlib, vendor layer
- [docs/FACADE_MAPPING.md](docs/FACADE_MAPPING.md) — public API surface per facade crate
- [docs/DEPENDENCY_FREEZE.md](docs/DEPENDENCY_FREEZE.md) — vendored crate inventory and freeze policy
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — build, test, and CI commands
- [docs/RELEASE.md](docs/RELEASE.md) — versioning and release checklist
