# Facade Mapping

This document maps the main Nexus Move integration concepts to the public `nexus-move` surface that is intended to remain stable for downstream callers.

## Runtime Mapping

- `move_adapter::VmConfig` -> `nexus_move_runtime::VmConfig`
- `move_adapter::NexusStateView` -> `nexus_move_runtime::NexusStateView`
- `move_adapter::MoveVm` -> `nexus_move_runtime::MoveVm`
- `move_adapter::MoveExecutor` -> `nexus_move_runtime::MoveExecutor`
- `move_adapter::VmOutput` -> `nexus_move_runtime::VmOutput`
- `move_adapter::query::QueryResult` -> `nexus_move_runtime::QueryResult`
- publisher storage keys -> `nexus_move_runtime::{MODULE_CODE_KEY, MODULE_CODE_HASH_KEY, MODULE_METADATA_KEY}`

Runtime behavior currently includes:

- planning backend and real VM backend selection
- publish, entry-function call, script execution, and query support
- gas metering and event capture
- upgrade policy enforcement and ABI hashing

## Bytecode Mapping

- local structural verification policy -> `nexus_move_bytecode::BytecodePolicy`
- bundle-level preflight checks -> `nexus_move_bytecode::verify_publish_bundle`
- verification findings -> `nexus_move_bytecode::VerificationError`

## Stdlib Mapping

- framework address helpers -> `nexus_move_stdlib::framework_address_bytes`
- embedded framework modules -> `nexus_move_stdlib::get_framework_module`
- native registry -> `nexus_move_stdlib::natives::native_functions`

The repository embeds the current Nexus framework module set under `0x1` and serves those bytes directly through the runtime storage bridge.

## Package Mapping

- package build entry point -> `nexus_move_package::build::build_package`
- `move_adapter::package::UpgradePolicy` -> `nexus_move_package::UpgradePolicy`
- `move_adapter::package::PackageMetadata` -> `nexus_move_package::PackageMetadata`
- wallet build defaults -> `nexus_move_package::BuildOptions`
- lightweight manifest inspection -> `nexus_move_package::inspect_move_toml`

Compile backend modes exposed by `nexus-move-package`:

- precompiled artifact loading
- verified compile wrapper
- vendored native compile via `move-compiler-v2`
- bootstrap subprocess backend for compatibility testing

## Public Boundary Guidance

Downstream callers should prefer the five first-party crates over direct imports from `vendor/`. The vendored crates are intentionally present for freeze control and local compilation, not as the primary public API of the repository.
