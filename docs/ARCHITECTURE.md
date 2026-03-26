# Architecture

## Crate Topology

```text
nexus-move-package ──► nexus-move-runtime ──► nexus-move-bytecode ──► nexus-move-types
                       nexus-move-stdlib  ──► nexus-move-types
                       (vm-backend)       ──► vendor/{move-vm-runtime, move-core-types, ...}
```

- **nexus-move-types**: zero-vendor-dependency type surface (`VmOutput`, `FunctionCall`, `UpgradePolicy`, etc.)
- **nexus-move-bytecode**: `BytecodePolicy`, `verify_publish_bundle`, structural verification
- **nexus-move-runtime**: execution facade with two VM backends, gas metering, state bridge, upstream re-exports
- **nexus-move-stdlib**: 11 framework modules at `0x1` (embedded bytecode, `include_bytes!`), 9 native functions
- **nexus-move-package**: `BuildOptions` → `CompiledPackage` pipeline, multiple compile backends

## VM Backends

`nexus-move-runtime` exposes the `MoveVm` trait with two implementations:

| Backend | Feature Gate | Capability |
|---|---|---|
| `PlanningMoveVm` | (default) | Structural verification, storage-key write-sets. For offline/planning use. |
| `RealMoveVm` | `vm-backend` | Full bytecode execution via vendored `move-vm-runtime`. Publish, call, query, events, upgrade policy. |

`MoveExecutor::with_vm()` selects the backend. Consumer code programs against the `MoveVm` trait.

## Gas Model

Two tiers:

1. **NexusGasMeter** (`vm-backend` only) — implements upstream `GasMeter` trait (~30 methods). Per-instruction tracking with configurable flat costs (instruction=1, call=10, global_op=5, load_resource_per_byte=2). Returns `OUT_OF_GAS` at limit.
2. **SimpleGasMeter / GasSchedule** — Nexus-level I/O cost estimates (transfer_base, publish_base, per-byte). Final `gas_used = max(vm_consumed, io_estimate)`.

## Stdlib

11 modules at framework address `0x1`: `ascii`, `bcs`, `bit_vector`, `error`, `fixed_point32`, `hash`, `option`, `signer`, `string`, `type_name`, `vector`.

9 native functions (requires `vm-backend`): `signer::borrow_address`, `bcs::to_bytes`, `hash::sha2_256`, `hash::sha3_256`, `type_name::get`, `string::internal_check_utf8`, `string::internal_sub_string`, `string::internal_index_of`, `string::internal_is_char_boundary`. Vector operations are VM bytecodes, not natives.

Bytecodes sourced from `aptos-node-v1.30.4` MoveStdlib build.

## Upgrade Policy

On publish, `PackageMetadata` (name, deployer, code hash, policy, version) is stored as BCS under `MODULE_METADATA_KEY`. Subsequent deploys enforce:

- **Immutable** (default): rejected (abort 20)
- **GovernanceOnly**: rejected (abort 21)
- **Compatible**: allowed (ABI compat check available via `abi_is_compatible`)

## Vendor Layer

`vendor/` contains 20 upstream Move crates from `aptos-core` at commit `d3a8cac631df`, split into three batches:

- **Batch 1 (runtime, 8 crates)**: `move-borrow-graph`, `move-bytecode-spec`, `move-core-types`, `move-binary-format`, `move-bytecode-verifier`, `move-vm-types`, `move-vm-metrics`, `move-vm-runtime`
- **Batch 2 (compiler leaf, 4 crates)**: `move-symbol-pool`, `move-command-line-common`, `move-ir-types`, `move-bytecode-source-map`
- **Batch 3 (compiler core, 8 crates)**: `move-bytecode-utils`, `abstract-domain-derive`, `legacy-move-compiler`, `move-coverage`, `move-disassembler`, `move-model`, `move-stackless-bytecode`, `move-compiler-v2`

All are workspace members with dev-dependencies stripped. ~55 external dependency versions are pinned in the root `Cargo.toml`.

## upstream Re-export Module

`nexus_move_runtime::upstream` (requires `vm-backend`) re-exports 40+ types from 4 vendor crates, mirroring their original module paths. This is the **sole** sanctioned import path for upstream types in consumer code. See [FACADE_MAPPING.md](FACADE_MAPPING.md) for the full listing.

## Package Frontend

`nexus-move-package` supports four compile backends:

1. **Stub** — deterministic testing (unit tests)
2. **Bootstrap** (`bootstrap-vendor`) — subprocess delegation to `nexus-wallet move build`
3. **Verified** (`verified-compile`) — any backend + bytecode verification via vendored verifier
4. **Native** (`native-compile`) — direct compilation via vendored `move-compiler-v2`

Pipeline: `BuildOptions` → `CompileBackend` → `CompiledPackage` → `PackageMetadata` + `ArtifactManifest`.
