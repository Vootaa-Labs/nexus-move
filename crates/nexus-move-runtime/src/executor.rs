use nexus_move_bytecode::{verify_publish_bundle, BytecodePolicy};

use crate::config::VmConfig;
use crate::publisher::publish_verified_modules;
use crate::state::NexusStateView;
use crate::types::{
    FunctionCall, ModulePublish, QueryRequest, QueryResult, ScriptExecution, VmError, VmOutput,
    VmResult,
};

pub trait MoveVm: Send + Sync {
    fn execute_function(
        &self,
        state: &NexusStateView<'_>,
        request: &FunctionCall,
    ) -> VmResult<VmOutput>;

    fn publish_modules(
        &self,
        state: &NexusStateView<'_>,
        request: &ModulePublish,
    ) -> VmResult<VmOutput>;

    fn execute_script(
        &self,
        state: &NexusStateView<'_>,
        request: &ScriptExecution,
    ) -> VmResult<VmOutput>;

    fn query_view(
        &self,
        state: &NexusStateView<'_>,
        request: &QueryRequest,
    ) -> VmResult<QueryResult>;
}

#[derive(Default)]
pub struct PlanningMoveVm;

impl MoveVm for PlanningMoveVm {
    fn execute_function(
        &self,
        _state: &NexusStateView<'_>,
        request: &FunctionCall,
    ) -> VmResult<VmOutput> {
        Ok(VmOutput::unsupported(request.gas_limit.min(1_000)))
    }

    fn publish_modules(
        &self,
        state: &NexusStateView<'_>,
        request: &ModulePublish,
    ) -> VmResult<VmOutput> {
        let policy = BytecodePolicy::bootstrap();
        verify_publish_bundle(&request.modules, &policy).map_err(VmError::Verification)?;
        let outcome = publish_verified_modules(
            state,
            request.sender,
            &request.modules,
            request.gas_limit,
            &VmConfig::default(),
        )?;
        Ok(outcome.vm_output)
    }

    fn execute_script(
        &self,
        _state: &NexusStateView<'_>,
        request: &ScriptExecution,
    ) -> VmResult<VmOutput> {
        Ok(VmOutput::unsupported(request.gas_limit.min(1_000)))
    }

    fn query_view(
        &self,
        state: &NexusStateView<'_>,
        request: &QueryRequest,
    ) -> VmResult<QueryResult> {
        let _ = state.has_module(&request.contract)?;
        Ok(QueryResult {
            return_value: None,
            gas_used: request.gas_budget.min(1_000),
            gas_budget: request.gas_budget,
        })
    }
}

pub struct MoveExecutor {
    vm: Box<dyn MoveVm>,
    config: VmConfig,
    bytecode_policy: BytecodePolicy,
}

impl MoveExecutor {
    pub fn new(config: VmConfig) -> Self {
        Self {
            vm: Box::<PlanningMoveVm>::default(),
            config,
            bytecode_policy: BytecodePolicy::bootstrap(),
        }
    }

    pub fn with_vm(vm: Box<dyn MoveVm>, config: VmConfig) -> Self {
        Self {
            vm,
            config,
            bytecode_policy: BytecodePolicy::bootstrap(),
        }
    }

    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    pub fn bytecode_policy(&self) -> &BytecodePolicy {
        &self.bytecode_policy
    }

    pub fn execute_function(
        &self,
        state: &NexusStateView<'_>,
        request: &FunctionCall,
    ) -> VmResult<VmOutput> {
        self.vm.execute_function(state, request)
    }

    pub fn publish_modules(
        &self,
        state: &NexusStateView<'_>,
        request: &ModulePublish,
    ) -> VmResult<VmOutput> {
        self.vm.publish_modules(state, request)
    }

    pub fn execute_script(
        &self,
        state: &NexusStateView<'_>,
        request: &ScriptExecution,
    ) -> VmResult<VmOutput> {
        self.vm.execute_script(state, request)
    }

    pub fn query_view(
        &self,
        state: &NexusStateView<'_>,
        request: &QueryRequest,
    ) -> VmResult<QueryResult> {
        self.vm.query_view(state, request)
    }
}
