use std::collections::BTreeMap;

use nexus_move_bytecode::VerificationError;
use nexus_move_types::AccountAddress;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VmStatus {
    Success,
    MoveAbort { location: String, code: u64 },
    OutOfGas,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateChange {
    pub account: AccountAddress,
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmOutput {
    pub status: VmStatus,
    pub gas_used: u64,
    pub state_changes: Vec<StateChange>,
    pub write_set: BTreeMap<(AccountAddress, Vec<u8>), Option<Vec<u8>>>,
    /// Events emitted during execution (from `write_to_event_store` native).
    pub events: Vec<ContractEvent>,
    /// Serialized return values from entry function execution.
    pub return_values: Vec<Vec<u8>>,
}

/// A contract event captured during Move execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractEvent {
    /// Event type tag (e.g. `"0xCAFE::counter::IncrementEvent"`).
    pub type_tag: String,
    /// GUID identifying the event stream.
    pub guid: Vec<u8>,
    /// Sequence number within the event stream (monotonic).
    pub sequence_number: u64,
    /// BCS-encoded event payload.
    pub data: Vec<u8>,
}

impl VmOutput {
    pub fn unsupported(gas_used: u64) -> Self {
        Self {
            status: VmStatus::Unsupported,
            gas_used,
            state_changes: Vec::new(),
            write_set: BTreeMap::new(),
            events: Vec::new(),
            return_values: Vec::new(),
        }
    }

    pub fn abort(location: String, base_gas: u64, gas_limit: u64) -> Self {
        Self {
            status: VmStatus::MoveAbort {
                location,
                code: 4001,
            },
            gas_used: base_gas.min(gas_limit),
            state_changes: Vec::new(),
            write_set: BTreeMap::new(),
            events: Vec::new(),
            return_values: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishOutcome {
    pub vm_output: VmOutput,
    pub contract_address: AccountAddress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VmError {
    Storage(String),
    Verification(Vec<VerificationError>),
    Unsupported(&'static str),
    InternalError(String),
}

pub type VmResult<T> = Result<T, VmError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionCall {
    pub sender: AccountAddress,
    pub contract: AccountAddress,
    pub function: String,
    pub type_args: Vec<Vec<u8>>,
    pub args: Vec<Vec<u8>>,
    pub gas_limit: u64,
}

/// Module upgrade policy — controls whether published modules can be replaced.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UpgradePolicy {
    /// Modules cannot be upgraded after initial deployment.
    #[default]
    Immutable,
    /// Modules may be upgraded with ABI-compatible changes.
    Compatible,
    /// Upgrades require governance approval.
    GovernanceOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePublish {
    pub sender: AccountAddress,
    pub modules: Vec<Vec<u8>>,
    pub gas_limit: u64,
    /// Upgrade policy to store with the published package.
    /// Defaults to `Immutable` if `None`.
    pub upgrade_policy: Option<UpgradePolicy>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptExecution {
    pub sender: AccountAddress,
    pub bytecode: Vec<u8>,
    pub type_args: Vec<Vec<u8>>,
    pub args: Vec<Vec<u8>>,
    pub gas_limit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryRequest {
    pub contract: AccountAddress,
    pub function: String,
    pub type_args: Vec<Vec<u8>>,
    pub args: Vec<Vec<u8>>,
    pub gas_budget: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryResult {
    pub return_value: Option<Vec<u8>>,
    pub gas_used: u64,
    pub gas_budget: u64,
}
