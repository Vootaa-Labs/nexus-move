# Dependency Freeze

## Upstream Source

All vendored crates originate from **aptos-core** at commit `d3a8cac631df`. Workspace-level `{ workspace = true }` entries have been resolved against the nexus-move workspace. Dev-dependencies are stripped.

## Vendored Crate Inventory

### Batch 1 — Runtime (8 crates)

| Crate | Purpose |
|---|---|
| `move-borrow-graph` | Borrow-graph data structure (no deps) |
| `move-bytecode-spec` | Proc-macro for bytecode spec attributes |
| `move-core-types` | `AccountAddress`, `TypeTag`, `ModuleId`, `Identifier` |
| `move-binary-format` | `CompiledModule`, bytecode (de)serialization |
| `move-bytecode-verifier` | Semantic bytecode verification |
| `move-vm-types` | Runtime values, resolver traits, gas trait |
| `move-vm-metrics` | Prometheus metrics for VM |
| `move-vm-runtime` | Module loader, interpreter, execution engine |

### Batch 2 — Compiler Leaf (4 crates)

| Crate | Purpose |
|---|---|
| `move-symbol-pool` | Static string interning |
| `move-command-line-common` | Path/address/value parsing utilities |
| `move-ir-types` | IR AST, source locations |
| `move-bytecode-source-map` | Compiled module ↔ source location mapping |

### Batch 3 — Compiler Core (8 crates)

| Crate | Purpose |
|---|---|
| `move-bytecode-utils` | Bytecode analysis helpers |
| `abstract-domain-derive` | Derive macro for abstract interpretation |
| `legacy-move-compiler` | Legacy Move compiler (v1) |
| `move-coverage` | Code coverage instrumentation |
| `move-disassembler` | Bytecode disassembly |
| `move-model` | Move source model for compiler-v2 |
| `move-stackless-bytecode` | Stackless bytecode IR for analysis |
| `move-compiler-v2` | Move compiler v2 |

## External Dependencies

~55 external crate versions are pinned in the root `Cargo.toml` to ensure reproducible builds. `Cargo.lock` is committed.

## Freeze Policy

- Vendor crates are **not updated** for routine work. All application logic stays in the 5 first-party `nexus-move-*` crates.
- Updates require: a security fix, a needed upstream capability within scope, or a deliberate narrowing effort.
- Any vendor update must be documented in release notes with the new upstream commit and rationale.

## Out of Scope

`move-prover`, `move-docgen`, `move-cli`, `move-bytecode-viewer`, and upstream testing infrastructure are excluded from the freeze boundary.
