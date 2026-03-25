//! Real Move VM backend backed by `move-vm-runtime`.
//!
//! Feature-gated behind `vm-backend`.  When active, [`RealMoveVm`] provides
//! real bytecode execution through the upstream Move interpreter, replacing
//! the planning stubs in [`super::executor::PlanningMoveVm`].

use std::collections::BTreeMap;

use bytes::Bytes;
use move_binary_format::access::ModuleAccess;
use move_binary_format::deserializer::DeserializerConfig;
use move_binary_format::errors::{Location, PartialVMError, PartialVMResult, VMResult};
use move_binary_format::file_format_common::{IDENTIFIER_SIZE_MAX, VERSION_MAX};
use move_binary_format::CompiledModule;
use move_core_types::account_address::AccountAddress as MoveAddress;
use move_core_types::identifier::{IdentStr, Identifier};
use move_core_types::language_storage::{ModuleId, StructTag, TypeTag};
use move_core_types::metadata::Metadata;
use move_core_types::value::MoveTypeLayout;
use move_core_types::vm_status::StatusCode;
use move_vm_runtime::data_cache::TransactionDataCache;
use move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage};
use move_vm_runtime::move_vm::MoveVM;
use move_vm_runtime::native_extensions::NativeContextExtensions;
use move_vm_runtime::{
    AsUnsyncCodeStorage, AsUnsyncModuleStorage, CodeStorage, ModuleStorage, RuntimeEnvironment,
    WithRuntimeEnvironment,
};
use move_vm_types::code::ModuleBytesStorage;
use move_vm_types::resolver::ResourceResolver;

use crate::abi::{abi_is_compatible, compute_module_abi_hash};
use crate::move_gas_meter::NexusGasMeter;

use nexus_move_types::AccountAddress;

use crate::config::VmConfig;
use crate::executor::MoveVm;
use crate::gas::{
    clamp_gas_to_limit, estimate_call_gas, estimate_publish_gas, estimate_script_gas, GasSchedule,
};
use crate::state::{module_code_key, NexusStateView, MODULE_CODE_KEY, MODULE_METADATA_KEY};
use crate::types::{
    ContractEvent, FunctionCall, ModulePublish, QueryRequest, QueryResult, ScriptExecution,
    StateChange, VmError, VmOutput, VmResult, VmStatus,
};

// ── Address conversion ──────────────────────────────────────────────────

fn nexus_to_move_address(addr: &AccountAddress) -> MoveAddress {
    MoveAddress::new(addr.0)
}

fn move_to_nexus_address(addr: &MoveAddress) -> AccountAddress {
    AccountAddress(addr.into_bytes())
}

// ── RealMoveVm ──────────────────────────────────────────────────────────

/// Real Move VM backed by `move-vm-runtime`.
///
/// Stateless — the `RuntimeEnvironment` caches native function registrations,
/// VM config, and shared struct caches.  Each call constructs ephemeral
/// `NexusBytesStorage` from the provided state view.
pub struct RealMoveVm {
    runtime_env: RuntimeEnvironment,
    schedule: GasSchedule,
    #[allow(dead_code)]
    config: VmConfig,
}

impl RealMoveVm {
    pub fn new(config: &VmConfig) -> Self {
        let natives = nexus_move_stdlib::natives::native_functions();
        let runtime_env = RuntimeEnvironment::new(natives);
        Self {
            runtime_env,
            schedule: GasSchedule::from_config(config),
            config: config.clone(),
        }
    }
}

impl MoveVm for RealMoveVm {
    fn execute_function(
        &self,
        state: &NexusStateView<'_>,
        request: &FunctionCall,
    ) -> VmResult<VmOutput> {
        let (module_name, fn_name) = match parse_function_name(&request.function) {
            Ok(pair) => pair,
            Err(msg) => {
                return Ok(VmOutput::abort(
                    msg,
                    self.schedule.call_base,
                    request.gas_limit,
                ))
            }
        };

        let move_addr = nexus_to_move_address(&request.contract);
        let module_ident = match Identifier::new(module_name) {
            Ok(id) => id,
            Err(_) => {
                return Ok(VmOutput::abort(
                    format!("{module_name}::{fn_name}"),
                    self.schedule.call_base,
                    request.gas_limit,
                ))
            }
        };
        let module_id = ModuleId::new(move_addr, module_ident);
        let fn_ident = match Identifier::new(fn_name) {
            Ok(id) => id,
            Err(_) => {
                return Ok(VmOutput::abort(
                    format!("{module_name}::{fn_name}"),
                    self.schedule.call_base,
                    request.gas_limit,
                ))
            }
        };

        let ty_args = deserialize_type_args(&request.type_args)?;

        let bytes_storage = NexusMoveStorage::new(state, &self.runtime_env);
        let module_storage = bytes_storage.as_unsync_module_storage();

        let loaded_fn = match module_storage.load_function(&module_id, &fn_ident, &ty_args) {
            Ok(f) => f,
            Err(_e) => {
                return Ok(VmOutput::abort(
                    format!("{module_id}"),
                    self.schedule.call_base,
                    request.gas_limit,
                ))
            }
        };

        let mut data_cache = TransactionDataCache::empty();
        let mut gas_meter = NexusGasMeter::with_limit(request.gas_limit);
        let traversal_storage = TraversalStorage::new();
        let mut traversal_ctx = TraversalContext::new(&traversal_storage);
        let mut extensions = NativeContextExtensions::default();

        // Register event store extension so write_to_event_store can capture events.
        extensions.add(nexus_move_stdlib::NexusEventStore::new());

        // Prepend sender address as signer argument (Move convention).
        let mut sender_bytes = vec![0u8]; // RuntimeVariants variant 0
        sender_bytes.extend_from_slice(&nexus_to_move_address(&request.sender).into_bytes());
        let mut full_args: Vec<Vec<u8>> = vec![sender_bytes];
        full_args.extend(request.args.iter().cloned());

        let result = MoveVM::execute_loaded_function(
            loaded_fn,
            full_args,
            &mut data_cache,
            &mut gas_meter,
            &mut traversal_ctx,
            &mut extensions,
            &module_storage,
            &bytes_storage,
        );

        match result {
            Ok(serialized_return) => {
                let change_set = data_cache.into_effects(&module_storage).map_err(|e| {
                    VmError::InternalError(format!("effects extraction failed: {e}"))
                })?;
                let (state_changes, write_set) = changeset_to_nexus(change_set);

                // Extract events from the NativeContextExtensions.
                let event_store = extensions.remove::<nexus_move_stdlib::NexusEventStore>();
                let events: Vec<ContractEvent> = event_store
                    .drain()
                    .into_iter()
                    .map(|e| ContractEvent {
                        type_tag: e.type_tag,
                        guid: e.guid,
                        sequence_number: e.sequence_number,
                        data: e.data,
                    })
                    .collect();

                // Capture return values.
                let return_values: Vec<Vec<u8>> = serialized_return
                    .return_values
                    .into_iter()
                    .map(|(bytes, _layout)| bytes)
                    .collect();

                // Use actual gas consumed by the interpreter.
                let gas_used = gas_meter.consumed().max(estimate_call_gas(
                    &self.schedule,
                    &request.type_args,
                    &request.args,
                    &state_changes,
                ));

                Ok(VmOutput {
                    status: VmStatus::Success,
                    gas_used: clamp_gas_to_limit(gas_used, request.gas_limit),
                    state_changes,
                    write_set: write_set.into_iter().collect(),
                    events,
                    return_values,
                })
            }
            Err(vm_err) => {
                let gas_used = gas_meter.consumed().max(estimate_call_gas(
                    &self.schedule,
                    &request.type_args,
                    &request.args,
                    &[],
                ));
                Ok(VmOutput {
                    status: VmStatus::MoveAbort {
                        location: format!("{}", vm_err.location()),
                        code: vm_err.major_status() as u64,
                    },
                    gas_used: clamp_gas_to_limit(gas_used, request.gas_limit),
                    state_changes: Vec::new(),
                    write_set: BTreeMap::new(),
                    events: Vec::new(),
                    return_values: Vec::new(),
                })
            }
        }
    }

    fn publish_modules(
        &self,
        state: &NexusStateView<'_>,
        request: &ModulePublish,
    ) -> VmResult<VmOutput> {
        use nexus_move_package::UpgradePolicy as PkgUpgradePolicy;
        use nexus_move_package::{decode_metadata, encode_metadata, PackageMetadata};

        let config = DeserializerConfig::new(VERSION_MAX, IDENTIFIER_SIZE_MAX);
        let move_sender = nexus_to_move_address(&request.sender);

        let mut state_changes = Vec::new();
        let mut write_set = BTreeMap::new();

        for module_bytes in &request.modules {
            let compiled =
                CompiledModule::deserialize_with_config(module_bytes, &config).map_err(|e| {
                    VmError::InternalError(format!("module deserialization failed: {e}"))
                })?;

            if compiled.self_id().address() != &move_sender {
                return Err(VmError::InternalError(
                    "module address does not match sender".into(),
                ));
            }

            let contract_addr = move_to_nexus_address(compiled.self_id().address());

            // ── Upgrade policy check ────────────────────────────────
            // If a module already exists at this address, enforce the
            // stored upgrade policy.
            if state.has_module(&contract_addr)? {
                let policy = state
                    .get_raw(&contract_addr, MODULE_METADATA_KEY)?
                    .and_then(|bytes| decode_metadata(&bytes).ok())
                    .map(|m| m.upgrade_policy)
                    .unwrap_or(PkgUpgradePolicy::Immutable); // legacy: no metadata → immutable

                match policy {
                    PkgUpgradePolicy::Immutable => {
                        return Ok(VmOutput {
                            status: VmStatus::MoveAbort {
                                location: "nexus::publish".into(),
                                code: 20, // already published
                            },
                            gas_used: self.schedule.publish_base,
                            state_changes: Vec::new(),
                            write_set: BTreeMap::new(),
                            events: Vec::new(),
                            return_values: Vec::new(),
                        });
                    }
                    PkgUpgradePolicy::GovernanceOnly => {
                        return Ok(VmOutput {
                            status: VmStatus::MoveAbort {
                                location: "nexus::publish".into(),
                                code: 21, // governance required
                            },
                            gas_used: self.schedule.publish_base,
                            state_changes: Vec::new(),
                            write_set: BTreeMap::new(),
                            events: Vec::new(),
                            return_values: Vec::new(),
                        });
                    }
                    PkgUpgradePolicy::Compatible => {
                        // Check ABI compatibility: new module must not
                        // break callers by changing the public interface.
                        let old_abi_hash = state
                            .get_raw(&contract_addr, MODULE_METADATA_KEY)?
                            .and_then(|bytes| decode_metadata(&bytes).ok())
                            .map(|m| m.abi_hash)
                            .unwrap_or([0u8; 32]);

                        // Only enforce if old module had a real ABI hash
                        // (zero means legacy module without ABI tracking).
                        if old_abi_hash != [0u8; 32] {
                            let new_abi_hash = compute_module_abi_hash(&compiled);
                            if !abi_is_compatible(&old_abi_hash, &new_abi_hash) {
                                return Ok(VmOutput {
                                    status: VmStatus::MoveAbort {
                                        location: "nexus::publish".into(),
                                        code: 22, // ABI incompatible
                                    },
                                    gas_used: self.schedule.publish_base,
                                    state_changes: Vec::new(),
                                    write_set: BTreeMap::new(),
                                    events: Vec::new(),
                                    return_values: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }

            // Bytecode verification
            let bytes_storage = NexusMoveStorage::new(state, &self.runtime_env);
            let module_storage = bytes_storage.as_unsync_module_storage();
            self.runtime_env
                .build_locally_verified_module(
                    std::sync::Arc::new(compiled.clone()),
                    module_bytes.len(),
                    blake3::hash(module_bytes).as_bytes(),
                )
                .map_err(|e| {
                    VmError::InternalError(format!("bytecode verification failed: {e}"))
                })?;

            // Verify dependencies
            for dep in compiled.immediate_dependencies() {
                let dep_exists = module_storage
                    .check_module_exists(dep.address(), dep.name())
                    .map_err(|e| VmError::InternalError(format!("dependency check failed: {e}")))?;
                if !dep_exists && dependency_must_exist(dep.address(), &move_sender) {
                    return Err(VmError::InternalError(format!("missing dependency: {dep}")));
                }
            }

            let code_hash = blake3::hash(module_bytes);

            let code_hash_bytes: [u8; 32] = *code_hash.as_bytes();

            // Build and store package metadata.
            let module_name = compiled.self_id().name().to_string();
            let abi_hash = compute_module_abi_hash(&compiled);
            let metadata = PackageMetadata {
                name: module_name.clone(),
                package_hash: code_hash_bytes,
                named_addresses: vec![(module_name.clone(), contract_addr)],
                module_hashes: vec![(module_name.clone(), code_hash_bytes)],
                abi_hash,
                upgrade_policy: match request.upgrade_policy {
                    Some(crate::types::UpgradePolicy::Compatible) => PkgUpgradePolicy::Compatible,
                    Some(crate::types::UpgradePolicy::GovernanceOnly) => {
                        PkgUpgradePolicy::GovernanceOnly
                    }
                    _ => PkgUpgradePolicy::Immutable,
                },
                deployer: request.sender,
                version: 1,
            };
            let metadata_bytes = encode_metadata(&metadata)
                .map_err(|e| VmError::InternalError(format!("metadata encoding failed: {e}")))?;

            // Per-module key: code::{module_name}
            let per_module_key = module_code_key(&module_name);
            state_changes.push(StateChange {
                account: contract_addr,
                key: per_module_key.clone(),
                value: Some(module_bytes.clone()),
            });
            write_set.insert((contract_addr, per_module_key), Some(module_bytes.clone()));

            // Legacy key: code  (kept for backwards compatibility)
            state_changes.push(StateChange {
                account: contract_addr,
                key: MODULE_CODE_KEY.to_vec(),
                value: Some(module_bytes.clone()),
            });
            state_changes.push(StateChange {
                account: contract_addr,
                key: b"code_hash".to_vec(),
                value: Some(code_hash.as_bytes().to_vec()),
            });
            state_changes.push(StateChange {
                account: contract_addr,
                key: MODULE_METADATA_KEY.to_vec(),
                value: Some(metadata_bytes.clone()),
            });

            write_set.insert(
                (contract_addr, MODULE_CODE_KEY.to_vec()),
                Some(module_bytes.clone()),
            );
            write_set.insert(
                (contract_addr, b"code_hash".to_vec()),
                Some(code_hash.as_bytes().to_vec()),
            );
            write_set.insert(
                (contract_addr, MODULE_METADATA_KEY.to_vec()),
                Some(metadata_bytes),
            );
        }

        Ok(VmOutput {
            status: VmStatus::Success,
            gas_used: clamp_gas_to_limit(
                estimate_publish_gas(&self.schedule, &request.modules, &state_changes),
                request.gas_limit,
            ),
            state_changes,
            write_set,
            events: Vec::new(),
            return_values: Vec::new(),
        })
    }

    fn execute_script(
        &self,
        state: &NexusStateView<'_>,
        request: &ScriptExecution,
    ) -> VmResult<VmOutput> {
        let ty_args = deserialize_type_args(&request.type_args)?;

        let bytes_storage = NexusMoveStorage::new(state, &self.runtime_env);
        let code_storage = bytes_storage.as_unsync_code_storage();

        // Prepend sender address as signer argument (Move convention).
        let mut sender_bytes = vec![0u8]; // RuntimeVariants variant 0
        sender_bytes.extend_from_slice(&nexus_to_move_address(&request.sender).into_bytes());
        let mut full_args: Vec<Vec<u8>> = vec![sender_bytes];
        full_args.extend(request.args.iter().cloned());

        // Load and verify the script, then get its entry point.
        let loaded_fn = match code_storage.load_script(&request.bytecode, &ty_args) {
            Ok(f) => f,
            Err(vm_err) => {
                return Ok(VmOutput {
                    status: VmStatus::MoveAbort {
                        location: format!("{}", vm_err.location()),
                        code: vm_err.major_status() as u64,
                    },
                    gas_used: clamp_gas_to_limit(self.schedule.call_base, request.gas_limit),
                    state_changes: Vec::new(),
                    write_set: BTreeMap::new(),
                    events: Vec::new(),
                    return_values: Vec::new(),
                })
            }
        };

        let mut data_cache = TransactionDataCache::empty();
        let mut gas_meter = NexusGasMeter::with_limit(request.gas_limit);
        let traversal_storage = TraversalStorage::new();
        let mut traversal_ctx = TraversalContext::new(&traversal_storage);
        let mut extensions = NativeContextExtensions::default();

        extensions.add(nexus_move_stdlib::NexusEventStore::new());

        let result = MoveVM::execute_loaded_function(
            loaded_fn,
            full_args,
            &mut data_cache,
            &mut gas_meter,
            &mut traversal_ctx,
            &mut extensions,
            &code_storage,
            &bytes_storage,
        );

        match result {
            Ok(serialized_return) => {
                let change_set = data_cache.into_effects(&code_storage).map_err(|e| {
                    VmError::InternalError(format!("effects extraction failed: {e}"))
                })?;
                let (state_changes, write_set) = changeset_to_nexus(change_set);

                let event_store = extensions.remove::<nexus_move_stdlib::NexusEventStore>();
                let events: Vec<ContractEvent> = event_store
                    .drain()
                    .into_iter()
                    .map(|e| ContractEvent {
                        type_tag: e.type_tag,
                        guid: e.guid,
                        sequence_number: e.sequence_number,
                        data: e.data,
                    })
                    .collect();

                let return_values: Vec<Vec<u8>> = serialized_return
                    .return_values
                    .into_iter()
                    .map(|(bytes, _layout)| bytes)
                    .collect();

                let gas_used = gas_meter.consumed().max(estimate_script_gas(
                    &self.schedule,
                    &request.bytecode,
                    &request.type_args,
                    &request.args,
                ));

                Ok(VmOutput {
                    status: VmStatus::Success,
                    gas_used: clamp_gas_to_limit(gas_used, request.gas_limit),
                    state_changes,
                    write_set: write_set.into_iter().collect(),
                    events,
                    return_values,
                })
            }
            Err(vm_err) => {
                let gas_used = gas_meter.consumed().max(estimate_script_gas(
                    &self.schedule,
                    &request.bytecode,
                    &request.type_args,
                    &request.args,
                ));
                Ok(VmOutput {
                    status: VmStatus::MoveAbort {
                        location: format!("{}", vm_err.location()),
                        code: vm_err.major_status() as u64,
                    },
                    gas_used: clamp_gas_to_limit(gas_used, request.gas_limit),
                    state_changes: Vec::new(),
                    write_set: BTreeMap::new(),
                    events: Vec::new(),
                    return_values: Vec::new(),
                })
            }
        }
    }

    fn query_view(
        &self,
        state: &NexusStateView<'_>,
        request: &QueryRequest,
    ) -> VmResult<QueryResult> {
        let (module_name, fn_name) =
            parse_function_name(&request.function).map_err(|msg| VmError::InternalError(msg))?;

        let move_addr = nexus_to_move_address(&request.contract);
        let module_id = ModuleId::new(
            move_addr,
            Identifier::new(module_name).map_err(|_| {
                VmError::InternalError(format!("invalid module name: {module_name}"))
            })?,
        );
        let fn_ident = Identifier::new(fn_name)
            .map_err(|_| VmError::InternalError(format!("invalid function name: {fn_name}")))?;

        let ty_args = deserialize_type_args(&request.type_args)?;

        let bytes_storage = NexusMoveStorage::new(state, &self.runtime_env);
        let module_storage = bytes_storage.as_unsync_module_storage();

        let loaded_fn = module_storage
            .load_function(&module_id, &fn_ident, &ty_args)
            .map_err(|e| VmError::InternalError(format!("load_function failed: {e}")))?;

        let mut data_cache = TransactionDataCache::empty();
        let mut gas_meter = NexusGasMeter::with_limit(request.gas_budget);
        let traversal_storage = TraversalStorage::new();
        let mut traversal_ctx = TraversalContext::new(&traversal_storage);
        let mut extensions = NativeContextExtensions::default();

        // Register event store extension (needed even for views, in case
        // a view function calls code that emits events as a side effect).
        extensions.add(nexus_move_stdlib::NexusEventStore::new());

        // View functions do NOT prepend a signer.
        let full_args: Vec<Vec<u8>> = request.args.clone();

        let result = MoveVM::execute_loaded_function(
            loaded_fn,
            full_args,
            &mut data_cache,
            &mut gas_meter,
            &mut traversal_ctx,
            &mut extensions,
            &module_storage,
            &bytes_storage,
        );

        match result {
            Ok(serialized) => {
                let return_values: Vec<Vec<u8>> = serialized
                    .return_values
                    .into_iter()
                    .map(|(bytes, _layout)| bytes)
                    .collect();
                Ok(QueryResult {
                    return_value: return_values.into_iter().next(),
                    gas_used: gas_meter.consumed(),
                    gas_budget: request.gas_budget,
                })
            }
            Err(vm_err) => Err(VmError::InternalError(format!(
                "query execution failed: {:?} at {}",
                vm_err.major_status(),
                vm_err.location()
            ))),
        }
    }
}

// ── NexusMoveStorage ────────────────────────────────────────────────────

/// Bridges `NexusStateView` to the upstream Move VM's storage traits.
struct NexusMoveStorage<'a> {
    state: &'a NexusStateView<'a>,
    runtime_env: &'a RuntimeEnvironment,
}

impl<'a> NexusMoveStorage<'a> {
    fn new(state: &'a NexusStateView<'a>, runtime_env: &'a RuntimeEnvironment) -> Self {
        Self { state, runtime_env }
    }
}

impl WithRuntimeEnvironment for NexusMoveStorage<'_> {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        self.runtime_env
    }
}

impl ModuleBytesStorage for NexusMoveStorage<'_> {
    fn fetch_module_bytes(
        &self,
        address: &MoveAddress,
        module_name: &IdentStr,
    ) -> VMResult<Option<Bytes>> {
        // Serve embedded framework modules (0x1) first.
        let framework_addr = MoveAddress::new(nexus_move_stdlib::framework_address_bytes());
        if *address == framework_addr {
            if let Some(bytes) = nexus_move_stdlib::get_framework_module(module_name.as_str()) {
                return Ok(Some(Bytes::from(bytes)));
            }
        }

        // Fall through to on-chain storage (per-module key with legacy fallback).
        let nexus_addr = move_to_nexus_address(address);
        match self
            .state
            .get_module_by_name(&nexus_addr, module_name.as_str())
        {
            Ok(Some(bytes)) => Ok(Some(Bytes::from(bytes))),
            Ok(None) => Ok(None),
            Err(e) => Err(PartialVMError::new(StatusCode::STORAGE_ERROR)
                .with_message(format!("state read failed: {e:?}"))
                .finish(Location::Undefined)),
        }
    }
}

impl ResourceResolver for NexusMoveStorage<'_> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &MoveAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        let key = bcs::to_bytes(struct_tag).map_err(|_| {
            PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR)
                .with_message("failed to serialize struct tag".into())
        })?;
        let nexus_addr = move_to_nexus_address(address);
        match self.state.get_resource(&nexus_addr, &key) {
            Ok(Some(bytes)) => {
                let size = bytes.len();
                Ok((Some(Bytes::from(bytes)), size))
            }
            Ok(None) => Ok((None, 0)),
            Err(e) => Err(PartialVMError::new(StatusCode::STORAGE_ERROR)
                .with_message(format!("resource read failed: {e:?}"))),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn parse_function_name(function: &str) -> Result<(&str, &str), String> {
    let parts: Vec<&str> = function.splitn(2, "::").collect();
    if parts.len() != 2 {
        return Err(format!(
            "invalid function format '{}', expected 'module::function'",
            function
        ));
    }
    Ok((parts[0], parts[1]))
}

fn deserialize_type_args(type_args: &[Vec<u8>]) -> VmResult<Vec<TypeTag>> {
    type_args
        .iter()
        .map(|bytes| {
            bcs::from_bytes(bytes).map_err(|e| {
                VmError::InternalError(format!("type arg deserialization failed: {e}"))
            })
        })
        .collect()
}

fn dependency_must_exist(dep_address: &MoveAddress, move_sender: &MoveAddress) -> bool {
    let framework = MoveAddress::new(nexus_move_stdlib::framework_address_bytes());
    dep_address != move_sender && dep_address != &framework
}

#[allow(clippy::type_complexity)]
fn changeset_to_nexus(
    change_set: move_core_types::effects::ChangeSet,
) -> (
    Vec<StateChange>,
    BTreeMap<(AccountAddress, Vec<u8>), Option<Vec<u8>>>,
) {
    let mut state_changes = Vec::new();
    let mut write_set = BTreeMap::new();

    for (addr, account_changes) in change_set.into_inner() {
        let nexus_addr = move_to_nexus_address(&addr);
        for (struct_tag, op) in account_changes.into_resources() {
            let key = bcs::to_bytes(&struct_tag).unwrap_or_default();
            match op {
                move_core_types::effects::Op::New(bytes)
                | move_core_types::effects::Op::Modify(bytes) => {
                    state_changes.push(StateChange {
                        account: nexus_addr,
                        key: key.clone(),
                        value: Some(bytes.to_vec()),
                    });
                    write_set.insert((nexus_addr, key), Some(bytes.to_vec()));
                }
                move_core_types::effects::Op::Delete => {
                    state_changes.push(StateChange {
                        account: nexus_addr,
                        key: key.clone(),
                        value: None,
                    });
                    write_set.insert((nexus_addr, key), None);
                }
            }
        }
    }

    (state_changes, write_set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StateReader;
    use crate::types::{FunctionCall, ModulePublish, QueryRequest, VmStatus};
    use std::collections::BTreeMap;
    use std::sync::RwLock;

    /// In-memory state store for testing.
    struct MemState {
        data: RwLock<BTreeMap<(AccountAddress, Vec<u8>), Vec<u8>>>,
    }

    impl MemState {
        fn new() -> Self {
            Self {
                data: RwLock::new(BTreeMap::new()),
            }
        }

        fn apply_output(&self, output: &VmOutput) {
            let mut data = self.data.write().unwrap();
            for change in &output.state_changes {
                match &change.value {
                    Some(value) => {
                        data.insert((change.account, change.key.clone()), value.clone());
                    }
                    None => {
                        data.remove(&(change.account, change.key.clone()));
                    }
                }
            }
        }
    }

    impl StateReader for MemState {
        fn get(
            &self,
            account: &AccountAddress,
            key: &[u8],
        ) -> crate::types::VmResult<Option<Vec<u8>>> {
            Ok(self
                .data
                .read()
                .unwrap()
                .get(&(*account, key.to_vec()))
                .cloned())
        }
    }

    fn cafe_address() -> AccountAddress {
        let mut bytes = [0u8; 32];
        bytes[30] = 0xCA;
        bytes[31] = 0xFE;
        AccountAddress(bytes)
    }

    /// Compiled counter module (addresses: counter_addr = 0xCAFE).
    const COUNTER_MV: &[u8] =
        include_bytes!("../../../examples/counter/nexus-artifact/bytecode/counter.mv");

    #[test]
    fn real_vm_publishes_counter_module() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());
        let deployer = cafe_address();

        let view = NexusStateView::new(&state);
        let output = vm
            .publish_modules(
                &view,
                &ModulePublish {
                    sender: deployer,
                    modules: vec![COUNTER_MV.to_vec()],
                    gas_limit: 100_000,
                    upgrade_policy: None,
                },
            )
            .expect("publish should succeed");

        assert_eq!(
            output.status,
            VmStatus::Success,
            "publish failed: {:?}",
            output.status
        );
        assert!(
            !output.state_changes.is_empty(),
            "no state changes after publish"
        );
    }

    #[test]
    fn real_vm_counter_lifecycle() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());
        let deployer = cafe_address();

        // ── Step 1: Publish counter module ──────────────────────────
        let view = NexusStateView::new(&state);
        let pub_output = vm
            .publish_modules(
                &view,
                &ModulePublish {
                    sender: deployer,
                    modules: vec![COUNTER_MV.to_vec()],
                    gas_limit: 100_000,
                    upgrade_policy: None,
                },
            )
            .expect("publish should succeed");
        assert_eq!(pub_output.status, VmStatus::Success);
        state.apply_output(&pub_output);

        // ── Step 2: Initialize counter (creates Counter resource) ───
        let view = NexusStateView::new(&state);
        let init_output = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "counter::initialize".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .expect("initialize should succeed");
        assert_eq!(
            init_output.status,
            VmStatus::Success,
            "initialize failed: {:?}",
            init_output.status
        );
        state.apply_output(&init_output);

        // ── Step 3: Increment counter ───────────────────────────────
        let view = NexusStateView::new(&state);
        let inc_output = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "counter::increment".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .expect("increment should succeed");
        assert_eq!(
            inc_output.status,
            VmStatus::Success,
            "increment failed: {:?}",
            inc_output.status
        );
        state.apply_output(&inc_output);

        // ── Step 4: Query get_count → expect 1 ─────────────────────
        let view = NexusStateView::new(&state);
        let addr_bytes = deployer.0.to_vec(); // BCS: address is fixed 32 bytes
        let query_result = vm
            .query_view(
                &view,
                &QueryRequest {
                    contract: deployer,
                    function: "counter::get_count".into(),
                    type_args: Vec::new(),
                    args: vec![addr_bytes],
                    gas_budget: 100_000,
                },
            )
            .expect("query should succeed");

        let return_bytes = query_result
            .return_value
            .expect("get_count should return a value");
        let count = u64::from_le_bytes(
            return_bytes
                .as_slice()
                .try_into()
                .expect("should be 8 bytes"),
        );
        assert_eq!(count, 1, "counter should be 1 after one increment");
    }

    #[test]
    fn real_vm_double_initialize_aborts() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());
        let deployer = cafe_address();

        // Publish
        let view = NexusStateView::new(&state);
        let pub_output = vm
            .publish_modules(
                &view,
                &ModulePublish {
                    sender: deployer,
                    modules: vec![COUNTER_MV.to_vec()],
                    gas_limit: 100_000,
                    upgrade_policy: None,
                },
            )
            .unwrap();
        state.apply_output(&pub_output);

        // First initialize (success)
        let view = NexusStateView::new(&state);
        let init1 = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "counter::initialize".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .unwrap();
        assert_eq!(init1.status, VmStatus::Success);
        state.apply_output(&init1);

        // Second initialize (should abort — resource already exists)
        let view = NexusStateView::new(&state);
        let init2 = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "counter::initialize".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .unwrap();
        assert!(
            matches!(init2.status, VmStatus::MoveAbort { .. }),
            "second initialize should abort, got: {:?}",
            init2.status
        );
    }

    #[test]
    fn real_vm_query_nonexistent_resource_fails() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());
        let deployer = cafe_address();

        // Publish (but do NOT initialize)
        let view = NexusStateView::new(&state);
        let pub_output = vm
            .publish_modules(
                &view,
                &ModulePublish {
                    sender: deployer,
                    modules: vec![COUNTER_MV.to_vec()],
                    gas_limit: 100_000,
                    upgrade_policy: None,
                },
            )
            .unwrap();
        state.apply_output(&pub_output);

        // Query get_count without initializing → should fail
        let view = NexusStateView::new(&state);
        let result = vm.query_view(
            &view,
            &QueryRequest {
                contract: deployer,
                function: "counter::get_count".into(),
                type_args: Vec::new(),
                args: vec![deployer.0.to_vec()],
                gas_budget: 100_000,
            },
        );
        assert!(result.is_err(), "query on missing resource should fail");
    }

    #[test]
    fn real_vm_multiple_increments() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());
        let deployer = cafe_address();

        // Publish + initialize
        let view = NexusStateView::new(&state);
        let pub_output = vm
            .publish_modules(
                &view,
                &ModulePublish {
                    sender: deployer,
                    modules: vec![COUNTER_MV.to_vec()],
                    gas_limit: 100_000,
                    upgrade_policy: None,
                },
            )
            .unwrap();
        state.apply_output(&pub_output);

        let view = NexusStateView::new(&state);
        let init = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "counter::initialize".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .unwrap();
        state.apply_output(&init);

        // Increment 5 times
        for _ in 0..5 {
            let view = NexusStateView::new(&state);
            let inc = vm
                .execute_function(
                    &view,
                    &FunctionCall {
                        sender: deployer,
                        contract: deployer,
                        function: "counter::increment".into(),
                        type_args: Vec::new(),
                        args: Vec::new(),
                        gas_limit: 100_000,
                    },
                )
                .unwrap();
            assert_eq!(inc.status, VmStatus::Success);
            state.apply_output(&inc);
        }

        // Query → expect 5
        let view = NexusStateView::new(&state);
        let query = vm
            .query_view(
                &view,
                &QueryRequest {
                    contract: deployer,
                    function: "counter::get_count".into(),
                    type_args: Vec::new(),
                    args: vec![deployer.0.to_vec()],
                    gas_budget: 100_000,
                },
            )
            .unwrap();
        let count = u64::from_le_bytes(query.return_value.unwrap().as_slice().try_into().unwrap());
        assert_eq!(count, 5);
    }

    // ── Gap #2: Wrong sender address rejection ─────────────────────────

    #[test]
    fn publish_rejects_wrong_sender_address() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());

        // counter.mv is compiled for address 0xCAFE.
        // Attempt to publish from a different sender (0xBEEF).
        let wrong_sender = {
            let mut bytes = [0u8; 32];
            bytes[30] = 0xBE;
            bytes[31] = 0xEF;
            AccountAddress(bytes)
        };

        let view = NexusStateView::new(&state);
        let result = vm.publish_modules(
            &view,
            &ModulePublish {
                sender: wrong_sender,
                modules: vec![COUNTER_MV.to_vec()],
                gas_limit: 100_000,
                upgrade_policy: None,
            },
        );

        match result {
            Err(VmError::InternalError(msg)) => {
                assert!(
                    msg.contains("module address does not match sender"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!(
                "expected InternalError('module address does not match sender'), got: {other:?}"
            ),
        }
    }

    // ── Gap #5: Function name parse errors ─────────────────────────────

    #[test]
    fn execute_function_rejects_missing_separator() {
        let (state, vm, deployer) = publish_counter();

        let view = NexusStateView::new(&state);
        let output = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "no_separator".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .unwrap();
        assert!(
            matches!(output.status, VmStatus::MoveAbort { .. }),
            "should abort for function name without '::', got: {:?}",
            output.status
        );
    }

    #[test]
    fn execute_function_rejects_empty_function_name() {
        let (state, vm, deployer) = publish_counter();

        let view = NexusStateView::new(&state);
        let output = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "counter::".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .unwrap();
        // Empty function name → abort (invalid identifier)
        assert!(
            matches!(output.status, VmStatus::MoveAbort { .. }),
            "should abort for empty function name, got: {:?}",
            output.status
        );
    }

    #[test]
    fn execute_function_rejects_nonexistent_function() {
        let (state, vm, deployer) = publish_counter();

        let view = NexusStateView::new(&state);
        let output = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: deployer,
                    contract: deployer,
                    function: "counter::nonexistent_fn".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .unwrap();
        assert!(
            matches!(output.status, VmStatus::MoveAbort { .. }),
            "should abort for nonexistent function, got: {:?}",
            output.status
        );
    }

    // ── Gap #6: AccountAddress boundary values ─────────────────────────

    #[test]
    fn zero_address_publish_fails_gracefully() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());

        // Zero address as sender with counter.mv (compiled for 0xCAFE)
        let zero = AccountAddress([0u8; 32]);

        let view = NexusStateView::new(&state);
        let result = vm.publish_modules(
            &view,
            &ModulePublish {
                sender: zero,
                modules: vec![COUNTER_MV.to_vec()],
                gas_limit: 100_000,
                upgrade_policy: None,
            },
        );
        // Should fail: zero address != 0xCAFE
        assert!(
            result.is_err(),
            "publish with zero address should fail (address mismatch)"
        );
    }

    #[test]
    fn max_address_publish_fails_gracefully() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());

        // All-0xFF address as sender with counter.mv (compiled for 0xCAFE)
        let max = AccountAddress([0xFF; 32]);

        let view = NexusStateView::new(&state);
        let result = vm.publish_modules(
            &view,
            &ModulePublish {
                sender: max,
                modules: vec![COUNTER_MV.to_vec()],
                gas_limit: 100_000,
                upgrade_policy: None,
            },
        );
        // Should fail: 0xFF..FF != 0xCAFE
        assert!(
            result.is_err(),
            "publish with max address should fail (address mismatch)"
        );
    }

    #[test]
    fn execute_on_nonexistent_module_aborts() {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());
        let addr = AccountAddress([0x42; 32]);

        let view = NexusStateView::new(&state);
        let output = vm
            .execute_function(
                &view,
                &FunctionCall {
                    sender: addr,
                    contract: addr,
                    function: "missing::call".into(),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    gas_limit: 100_000,
                },
            )
            .unwrap();
        assert!(
            matches!(output.status, VmStatus::MoveAbort { .. }),
            "execute on non-deployed module should abort, got: {:?}",
            output.status
        );
    }

    // ── Gap #7: Missing dependency publish failure ─────────────────────

    #[test]
    fn publish_module_with_missing_dependency_fails() {
        // Build a minimal Move bytecode module that declares a dependency on
        // a non-framework, non-self address module that doesn't exist.
        use move_binary_format::file_format::*;
        use move_binary_format::file_format_common::VERSION_MAX;

        let self_addr = AccountAddress([0xAA; 32]);
        let self_addr_move = nexus_to_move_address(&self_addr);

        let dep_addr = AccountAddress([0xBB; 32]);
        let dep_addr_move = nexus_to_move_address(&dep_addr);

        let module = CompiledModule {
            version: VERSION_MAX,
            self_module_handle_idx: ModuleHandleIndex(0),
            module_handles: vec![
                // Index 0: self
                ModuleHandle {
                    address: AddressIdentifierIndex(0),
                    name: IdentifierIndex(0),
                },
                // Index 1: dependency at 0xBB that doesn't exist
                ModuleHandle {
                    address: AddressIdentifierIndex(1),
                    name: IdentifierIndex(1),
                },
            ],
            identifiers: vec![
                Identifier::new("test_mod").unwrap(),
                Identifier::new("dep_module").unwrap(),
            ],
            address_identifiers: vec![self_addr_move, dep_addr_move],
            struct_handles: Vec::new(),
            function_handles: Vec::new(),
            field_handles: Vec::new(),
            friend_decls: Vec::new(),
            struct_def_instantiations: Vec::new(),
            function_instantiations: Vec::new(),
            field_instantiations: Vec::new(),
            signatures: vec![Signature(Vec::new())],
            constant_pool: Vec::new(),
            metadata: Vec::new(),
            struct_defs: Vec::new(),
            function_defs: Vec::new(),
            struct_variant_handles: Vec::new(),
            struct_variant_instantiations: Vec::new(),
            variant_field_handles: Vec::new(),
            variant_field_instantiations: Vec::new(),
        };

        let mut bytes = Vec::new();
        module.serialize(&mut bytes).expect("serialization");

        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());

        let view = NexusStateView::new(&state);
        let result = vm.publish_modules(
            &view,
            &ModulePublish {
                sender: self_addr,
                modules: vec![bytes],
                gas_limit: 100_000,
                upgrade_policy: None,
            },
        );

        assert!(
            result.is_err(),
            "publish with missing dependency should fail"
        );
        if let Err(VmError::InternalError(msg)) = &result {
            assert!(
                msg.contains("missing dependency") || msg.contains("dep_module"),
                "error should mention missing dependency: {msg}"
            );
        }
    }

    // ── Helper: publish counter and return state ───────────────────────

    fn publish_counter() -> (MemState, RealMoveVm, AccountAddress) {
        let state = MemState::new();
        let vm = RealMoveVm::new(&VmConfig::for_testing());
        let deployer = cafe_address();

        let view = NexusStateView::new(&state);
        let pub_output = vm
            .publish_modules(
                &view,
                &ModulePublish {
                    sender: deployer,
                    modules: vec![COUNTER_MV.to_vec()],
                    gas_limit: 100_000,
                    upgrade_policy: None,
                },
            )
            .unwrap();
        assert_eq!(pub_output.status, VmStatus::Success);
        state.apply_output(&pub_output);

        (state, vm, deployer)
    }
}
