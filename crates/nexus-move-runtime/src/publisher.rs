use std::collections::BTreeMap;

use nexus_move_bytecode::{verify_publish_bundle, BytecodePolicy};
use nexus_move_types::AccountAddress;

use crate::config::VmConfig;
use crate::state::{
    NexusStateView, MODULE_CODE_HASH_KEY, MODULE_CODE_KEY, MODULE_COUNT_KEY, MODULE_DEPLOYER_KEY,
};
use crate::types::{PublishOutcome, StateChange, VmError, VmOutput, VmResult, VmStatus};

const CONTRACT_ADDRESS_DOMAIN: &[u8] = b"nexus::contract::address::v1";

pub fn derive_contract_address(
    deployer: &AccountAddress,
    bytecode_hash: &[u8; 32],
) -> AccountAddress {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONTRACT_ADDRESS_DOMAIN);
    hasher.update(&deployer.0);
    hasher.update(bytecode_hash);
    AccountAddress(*hasher.finalize().as_bytes())
}

pub fn publish_verified_modules(
    state: &NexusStateView<'_>,
    sender: AccountAddress,
    modules: &[Vec<u8>],
    gas_limit: u64,
    config: &VmConfig,
) -> VmResult<PublishOutcome> {
    verify_publish_bundle(modules, &BytecodePolicy::bootstrap()).map_err(VmError::Verification)?;

    let total_size: usize = modules.iter().map(Vec::len).sum();
    let mut bytecode = Vec::with_capacity(total_size);
    for module in modules {
        bytecode.extend_from_slice(module);
    }

    let code_hash = *blake3::hash(&bytecode).as_bytes();
    let contract_address = derive_contract_address(&sender, &code_hash);

    if state.has_module(&contract_address)? {
        return Ok(PublishOutcome {
            vm_output: VmOutput {
                status: VmStatus::MoveAbort {
                    location: "nexus::publish".into(),
                    code: 20,
                },
                gas_used: config.publish_base_gas,
                state_changes: Vec::new(),
                write_set: BTreeMap::new(),
                events: Vec::new(),
                return_values: Vec::new(),
            },
            contract_address,
        });
    }

    let estimated = config
        .publish_base_gas
        .saturating_add((total_size as u64).saturating_mul(config.publish_per_byte_gas));

    if gas_limit != 0 && estimated >= gas_limit {
        return Ok(PublishOutcome {
            vm_output: VmOutput {
                status: VmStatus::OutOfGas,
                gas_used: gas_limit,
                state_changes: Vec::new(),
                write_set: BTreeMap::new(),
                events: Vec::new(),
                return_values: Vec::new(),
            },
            contract_address,
        });
    }

    let mut write_set = BTreeMap::new();
    let mut state_changes = Vec::new();

    push_write(
        &mut write_set,
        &mut state_changes,
        contract_address,
        MODULE_CODE_KEY,
        bytecode,
    );
    push_write(
        &mut write_set,
        &mut state_changes,
        contract_address,
        MODULE_CODE_HASH_KEY,
        code_hash.to_vec(),
    );
    push_write(
        &mut write_set,
        &mut state_changes,
        contract_address,
        MODULE_DEPLOYER_KEY,
        sender.0.to_vec(),
    );
    push_write(
        &mut write_set,
        &mut state_changes,
        contract_address,
        MODULE_COUNT_KEY,
        (modules.len() as u32).to_le_bytes().to_vec(),
    );

    Ok(PublishOutcome {
        vm_output: VmOutput {
            status: VmStatus::Success,
            gas_used: estimated,
            state_changes,
            write_set,
            events: Vec::new(),
            return_values: Vec::new(),
        },
        contract_address,
    })
}

fn push_write(
    write_set: &mut BTreeMap<(AccountAddress, Vec<u8>), Option<Vec<u8>>>,
    state_changes: &mut Vec<StateChange>,
    account: AccountAddress,
    key: &[u8],
    value: Vec<u8>,
) {
    write_set.insert((account, key.to_vec()), Some(value.clone()));
    state_changes.push(StateChange {
        account,
        key: key.to_vec(),
        value: Some(value),
    });
}
