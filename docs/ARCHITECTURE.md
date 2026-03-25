# Architecture

## Goal

`nexus-move` isolates the Move-specific runtime, bytecode, stdlib, and package concerns from the rest of the Nexus system. The extraction target is a repository that can be versioned, audited, built, and distributed independently while preserving current Nexus external behavior.

## Repository Boundary

Owned here:

- Move runtime facade and VM integration
- bytecode decoding and verification facade
- stdlib snapshot and native registration
- package frontend and artifact generation support
- frozen upstream Move crate subset

Owned by the main Nexus repository:

- transaction orchestration and executor scheduling
- node startup and feature gating
- RPC service surface
- storage engine implementations
- consensus, network, and devnet assets

## Initial Crates

- `nexus-move-types`: minimal type surface for callers and adapters
- `nexus-move-bytecode`: bytecode access, verification entrypoints, and structural policy
- `nexus-move-runtime`: runtime facade and state bridge glue
- `nexus-move-stdlib`: stdlib snapshot handling and native function registration
- `nexus-move-package`: package parsing, dependency policy, artifact compatibility

## Current Runtime Module Shape

`nexus-move-runtime` is now split into submodules that roughly mirror the main-repo Move adapter boundary:

- `config`
- `types`
- `state`
- `gas`
- `resources`
- `session`
- `publisher`
- `executor`

This split is intentional so later extraction work can move logic from the main Nexus repository into matching files instead of repeatedly reshaping one large module.

## Compatibility Contract

The extraction must preserve:

- `nexus-wallet move build`
- `nexus-wallet move deploy`
- `nexus-wallet move call`
- `nexus-wallet move query`
- `nexus-artifact/` layout
- `PackageMetadata` and upgrade policy encoding
- publish, call, and query RPC semantics

## Migration Principle

Do not rewrite the Move VM, bytecode format, verifier, or compiler. Freeze and narrow the upstream crates instead. Move productization and distribution concerns into Nexus-owned layers.

## Vendor Layer

`vendor/` contains 12 upstream Move crates from `aptos-core` at commit `d3a8cac631df`, normalized to resolve `{ workspace = true }` entries against the nexus-move workspace:

```text
vendor/
├── Batch 1: Runtime Chain (8 crates)
│   ├── move-borrow-graph       (no dependencies)
│   ├── move-bytecode-spec      (proc-macro: once_cell, quote, syn)
│   ├── move-core-types         (core types: AccountAddress, TypeTag, etc.)
│   ├── move-binary-format      (CompiledModule, deserialization)
│   ├── move-bytecode-verifier  (semantic bytecode verification)
│   ├── move-vm-types           (runtime values, resolver traits)
│   ├── move-vm-metrics         (prometheus metrics)
│   └── move-vm-runtime         (module loader, interpreter, execution engine)
│
└── Batch 2: Compiler Leaf (4 crates)
    ├── move-symbol-pool        (static string interning)
    ├── move-command-line-common (path/address/value parsing)
    ├── move-ir-types           (IR AST, locations)
    └── move-bytecode-source-map (compiled module ↔ source location mapping)
```

All 12 are workspace members with dev-dependencies stripped. The nexus-move workspace Cargo.toml provides version pinning for ~55 external dependencies they use.

## VM Backend Integration

`nexus-move-runtime` provides two `MoveVm` implementations:

1. **`PlanningMoveVm`** (default) — stub that only handles structural verification and storage-key write-sets. Used for planning and offline development.
2. **`RealMoveVm`** (feature `vm-backend`) — wraps the vendored `move-vm-runtime` for real bytecode execution. Uses `NexusMoveStorage` bridge to serve framework modules from `nexus-move-stdlib` and user modules from state. All 9 native functions are registered. Tested with full counter and token contract lifecycles.

The `MoveExecutor` wrapper selects the backend via `MoveExecutor::with_vm()`.

## Gas Metering

Two-tier gas model:

1. **NexusGasMeter** (`move_gas_meter.rs`, feature `vm-backend`) — implements the full upstream `move_vm_types::gas::GasMeter` trait (~30 methods) with configurable flat costs per instruction category. Wired into the Move VM interpreter for per-instruction gas tracking:
   - `instruction_base` = 1 (Add, Sub, Mul, etc.)
   - `call_base` = 10 (function calls)
   - `global_op_base` = 5 (borrow_global, exists, move_from, move_to)
   - `load_resource_per_byte` = 2
   - `native_call_base` = 10
   - `pack_unpack_base` = 3
   - `ref_op_base` = 2 / `vec_op_base` = 3
   - Returns `OUT_OF_GAS` error when limit is reached

2. **SimpleGasMeter / GasSchedule** (`gas.rs`) — Nexus-level I/O cost estimates (transfer_base, call_base, publish_base, per-byte costs). Used as a lower bound: `gas_used = max(interpreter_consumed, io_estimate)`.

`execute_function` and `query_view` construct a `NexusGasMeter::with_limit(gas_limit)` and pass it to the Move VM. After execution, `gas_meter.consumed()` provides the actual instruction count.

## Upgrade Policy

`RealMoveVm::publish_modules` enforces upgrade policy:

1. On first deploy, `PackageMetadata` (name, deployer, code hash, upgrade policy, version) is stored as BCS under `MODULE_METADATA_KEY` ("package_metadata").
2. On subsequent deploys to the same address:
   - **Immutable** (default): rejected with abort code 20
   - **GovernanceOnly**: rejected with abort code 21
   - **Compatible**: allowed (ABI compatibility checking not yet implemented)
3. Legacy modules without stored metadata are treated as Immutable.

`ModulePublish` includes an optional `upgrade_policy: Option<UpgradePolicy>` field.

## Stdlib

`nexus-move-stdlib` embeds compiled bytecode for 11 framework modules at address `0x1`:

```text
ascii, bcs, bit_vector, error, fixed_point32, hash,
option, signer, string, type_name, vector
```

All bytecodes are from aptos-node-v1.30.4 MoveStdlib build artifacts (`include_bytes!`).

With the `vm-backend` feature, the stdlib provides native function registration for all 9 move-stdlib natives:

- **signer**: `borrow_address`
- **bcs**: `to_bytes` (BCS serialization via `ValueSerDeContext`)
- **hash**: `sha2_256`, `sha3_256` (crypto digests via `sha2`/`sha3` crates)
- **type_name**: `get` (type tag → canonical string → TypeName struct)
- **string**: `internal_check_utf8`, `internal_sub_string`, `internal_index_of`, `internal_is_char_boundary`

Vector operations (`length`, `borrow`, `push_back`, `pop_back`, `destroy_empty`, `swap`) are VM bytecodes — they are handled by the interpreter, not as native functions. All native implementations use safe Rust only (no `unsafe` blocks).

## Package Frontend

`nexus-move-package` provides:

- `BuildOptions` / `BuildPlan` / `CompileBackend` trait
- `CompiledPackage` / `PackageMetadata` / `ArtifactManifest`
- `orchestrate_build` — end-to-end compile → metadata → manifest pipeline

Three compile backend modes:

1. **Stub** — unit tests provide a `StubBackend` for deterministic orchestration testing
2. **Bootstrap** (feature `bootstrap-vendor`) — shells out to main-repo `nexus-wallet move build`; collects `.mv` artifacts from the build directory
3. **Verified** (feature `verified-compile`) — wraps any backend with native bytecode verification and module metadata extraction using vendored `move-binary-format` + `move-bytecode-verifier`; extracts module name, address, dependencies, friends, and hash

The verified backend can compose with any delegate: `VerifiedCompileBackend::new(BootstrapBackend::new())` gives compile + verify.

## Cross-Repo Compatibility

Integration tests in `crates/nexus-move-runtime/tests/cross_repo_compat.rs` verify wire-format alignment with the main Nexus repository:

- Storage key constants (`b"code"`, `b"code_hash"`, `b"balance"`) must match
- `PackageMetadata` BCS encoding must round-trip identically
- BLAKE3 hashing for contract addresses and module hashes
- Bytecode magic bytes `0xa11ceb0b`
- Gas schedule default values
- Framework address `0x1` and required stdlib modules
