#![forbid(unsafe_code)]

pub mod config;
pub mod executor;
pub mod gas;
pub mod publisher;
pub mod resources;
pub mod session;
pub mod state;
pub mod types;

#[cfg(feature = "vm-backend")]
pub mod abi;

#[cfg(feature = "vm-backend")]
pub mod move_gas_meter;

#[cfg(feature = "vm-backend")]
pub mod vm_backend;

use nexus_move_bytecode::BytecodePolicy;

pub const CRATE_ROLE: &str = "runtime-facade";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBootstrap {
    pub bytecode_policy: BytecodePolicy,
    pub supports_query_path: bool,
}

impl RuntimeBootstrap {
    pub fn new(bytecode_policy: BytecodePolicy) -> Self {
        Self {
            bytecode_policy,
            supports_query_path: true,
        }
    }
}

pub use config::VmConfig;
pub use executor::{MoveExecutor, MoveVm, PlanningMoveVm};
pub use gas::{
    clamp_gas_to_limit, estimate_call_gas, estimate_publish_gas, estimate_script_gas,
    publish_gas_cost, read_gas_cost, write_gas_cost, GasExhausted, GasMeter, GasSchedule,
    SimpleGasMeter,
};
pub use publisher::{derive_contract_address, publish_verified_modules};
pub use resources::{resource_key, ResourceStore, WriteSet};
pub use session::{ExecuteSession, MoveGasSummary, SessionKind, WriteError};
pub use state::{
    parse_balance_bytes, NexusStateView, StateReader, BALANCE_KEY, MODULE_CODE_HASH_KEY,
    MODULE_CODE_KEY, MODULE_COUNT_KEY, MODULE_DEPLOYER_KEY, MODULE_METADATA_KEY,
};
pub use types::{
    FunctionCall, ModulePublish, PublishOutcome, QueryRequest, QueryResult, ScriptExecution,
    StateChange, UpgradePolicy, VmError, VmOutput, VmResult, VmStatus,
};

#[cfg(feature = "vm-backend")]
pub use abi::{abi_is_compatible, compute_module_abi_hash, compute_package_abi_hash};

#[cfg(feature = "vm-backend")]
pub use vm_backend::RealMoveVm;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use nexus_move_bytecode::VerificationError;
    use nexus_move_types::AccountAddress;

    struct MemState {
        data: BTreeMap<(AccountAddress, Vec<u8>), Vec<u8>>,
    }

    impl MemState {
        fn new() -> Self {
            Self {
                data: BTreeMap::new(),
            }
        }

        fn set(&mut self, account: AccountAddress, key: &[u8], value: Vec<u8>) {
            self.data.insert((account, key.to_vec()), value);
        }
    }

    impl StateReader for MemState {
        fn get(&self, account: &AccountAddress, key: &[u8]) -> VmResult<Option<Vec<u8>>> {
            Ok(self.data.get(&(*account, key.to_vec())).cloned())
        }
    }

    fn address(byte: u8) -> AccountAddress {
        AccountAddress([byte; 32])
    }

    #[test]
    fn reads_balance_through_state_view() {
        let mut state = MemState::new();
        state.set(address(0xAA), BALANCE_KEY, 42u64.to_le_bytes().to_vec());
        let view = NexusStateView::new(&state);

        assert_eq!(view.get_balance(&address(0xAA)).unwrap(), 42);
    }

    #[test]
    fn planning_backend_rejects_empty_publish_bundle() {
        let state = MemState::new();
        let view = NexusStateView::new(&state);
        let exec = MoveExecutor::new(VmConfig::for_testing());
        let request = ModulePublish {
            sender: address(0xAA),
            modules: Vec::new(),
            gas_limit: 10_000,
            upgrade_policy: None,
        };

        let error = exec.publish_modules(&view, &request).unwrap_err();
        assert_eq!(
            error,
            VmError::Verification(vec![VerificationError::EmptyModuleSet])
        );
    }

    #[test]
    fn query_path_returns_budgeted_stub_result() {
        let mut state = MemState::new();
        state.set(address(0xBB), MODULE_CODE_KEY, vec![1, 2, 3]);
        let view = NexusStateView::new(&state);
        let exec = MoveExecutor::new(VmConfig::default());
        let request = QueryRequest {
            contract: address(0xBB),
            function: "get_count".into(),
            type_args: Vec::new(),
            args: Vec::new(),
            gas_budget: 500,
        };

        let result = exec.query_view(&view, &request).unwrap();
        assert_eq!(result.gas_used, 500);
        assert_eq!(result.gas_budget, 500);
        assert_eq!(result.return_value, None);
    }

    #[test]
    fn contract_address_derivation_is_deterministic() {
        let sender = address(0xAB);
        let bytecode_hash = *blake3::hash(&[1, 2, 3, 4]).as_bytes();

        let left = derive_contract_address(&sender, &bytecode_hash);
        let right = derive_contract_address(&sender, &bytecode_hash);

        assert_eq!(left, right);
    }

    #[test]
    fn publish_verified_modules_writes_expected_keys() {
        let state = MemState::new();
        let view = NexusStateView::new(&state);
        let sender = address(0xCD);
        let modules = vec![
            vec![0xa1, 0x1c, 0xeb, 0x0b, 1, 0, 0, 0],
            vec![0xa1, 0x1c, 0xeb, 0x0b, 2, 0, 0, 0],
        ];

        let outcome =
            publish_verified_modules(&view, sender, &modules, 50_000, &VmConfig::for_testing())
                .unwrap();

        assert_eq!(outcome.vm_output.status, VmStatus::Success);
        assert_eq!(outcome.vm_output.state_changes.len(), 4);
        assert!(outcome
            .vm_output
            .write_set
            .contains_key(&(outcome.contract_address, MODULE_CODE_KEY.to_vec())));
        assert!(outcome
            .vm_output
            .write_set
            .contains_key(&(outcome.contract_address, MODULE_CODE_HASH_KEY.to_vec())));
        assert!(outcome
            .vm_output
            .write_set
            .contains_key(&(outcome.contract_address, MODULE_DEPLOYER_KEY.to_vec())));
        assert!(outcome
            .vm_output
            .write_set
            .contains_key(&(outcome.contract_address, MODULE_COUNT_KEY.to_vec())));
    }

    #[test]
    fn session_commit_produces_write_set() {
        let state = MemState::new();
        let view = NexusStateView::new(&state);
        let schedule = GasSchedule::default();
        let mut session = ExecuteSession::new(
            SessionKind::Execute,
            address(0xAA),
            100_000,
            schedule,
            &view,
        );

        session
            .write_resource(address(0xBB), "counter::Counter", vec![42])
            .unwrap();
        let output = session.commit(VmStatus::Success);

        assert_eq!(output.status, VmStatus::Success);
        assert_eq!(output.write_set.len(), 1);
        assert_eq!(output.state_changes.len(), 1);
    }
}
