# Facade Mapping

Consumer crates import exclusively from the 5 `nexus-move-*` facade crates. This document lists the public surface per crate.

## nexus-move-types

Shared types with no vendor dependencies.

| Export | Description |
|---|---|
| `VmOutput`, `VmResult`, `VmStatus`, `VmError` | Execution result types |
| `FunctionCall`, `ScriptExecution`, `ModulePublish` | Transaction intent types |
| `QueryRequest`, `QueryResult` | View query types |
| `PublishOutcome`, `StateChange` | Execution effect types |
| `UpgradePolicy` | Module upgrade policy enum |

## nexus-move-bytecode

| Export | Description |
|---|---|
| `BytecodePolicy` | Configuration for verification strictness |
| `verify_publish_bundle` | Bundle-level publish preflight |
| `VerificationError` | Verification finding type |

## nexus-move-runtime

### Core API (always available)

| Export | Description |
|---|---|
| `VmConfig` | VM configuration |
| `MoveExecutor`, `MoveVm`, `PlanningMoveVm` | Executor and VM backend trait/impls |
| `GasSchedule`, `SimpleGasMeter`, `GasMeter`, `GasExhausted` | Gas metering |
| `NexusStateView`, `StateReader` | State access traits |
| `ExecuteSession`, `SessionKind`, `MoveGasSummary` | Session management |
| `derive_contract_address`, `publish_verified_modules` | Publishing helpers |
| `resource_key`, `ResourceStore`, `WriteSet` | Resource storage |
| `MODULE_CODE_KEY`, `MODULE_CODE_HASH_KEY`, `MODULE_METADATA_KEY` | Storage key constants |
| `BALANCE_KEY`, `MODULE_COUNT_KEY`, `MODULE_DEPLOYER_KEY` | Storage key constants |
| `RuntimeBootstrap` | Runtime initialization config |

### vm-backend Feature

| Export | Description |
|---|---|
| `RealMoveVm` | Full Move VM execution backend |
| `abi_is_compatible`, `compute_module_abi_hash`, `compute_package_abi_hash` | ABI compatibility |
| `NexusGasMeter` (via `move_gas_meter`) | Full upstream `GasMeter` implementation |

### upstream Re-export Module (vm-backend)

`nexus_move_runtime::upstream::*` — sole import path for upstream Move types:

```text
upstream::move_core_types::
  account_address::AccountAddress
  effects::{ChangeSet, Op}
  gas_algebra::InternalGas
  identifier::{IdentStr, Identifier}
  language_storage::{ModuleId, StructTag, TypeTag}
  metadata::Metadata
  value::MoveTypeLayout
  vm_status::StatusCode

upstream::move_binary_format::
  CompiledModule
  access::ModuleAccess
  deserializer::DeserializerConfig
  errors::{Location, PartialVMError, PartialVMResult, VMResult}
  file_format_common::{IDENTIFIER_SIZE_MAX, VERSION_MAX}

upstream::move_vm_runtime::
  {AsUnsyncModuleStorage, ModuleStorage, RuntimeEnvironment, WithRuntimeEnvironment}
  data_cache::TransactionDataCache
  module_traversal::{TraversalContext, TraversalStorage}
  move_vm::MoveVM
  native_extensions::NativeContextExtensions
  native_functions::NativeFunction

upstream::move_vm_types::
  code::ModuleBytesStorage
  gas::UnmeteredGasMeter
  loaded_data::runtime_types::Type
  natives::function::NativeResult
  resolver::ResourceResolver
  values::{Value, values_impl::SignerRef}
  pop_arg  (macro)
```

## nexus-move-stdlib

| Export | Description |
|---|---|
| `framework_address_bytes` | `0x1` address as bytes |
| `get_framework_module` | Retrieve embedded module bytecode by name |
| `natives::native_functions` | Native function registry (requires `vm-backend`) |

## nexus-move-package

| Export | Description |
|---|---|
| `BuildOptions`, `CompiledPackage` | Build configuration and output |
| `PackageMetadata`, `ArtifactManifest` | Package metadata and manifest |
| `UpgradePolicy` | Re-exported upgrade policy |
| `build_package` / `orchestrate_build` | Build entry points |
| `inspect_move_toml` | Lightweight manifest inspection |
| `CompileBackend` | Backend trait for compile pipeline |
