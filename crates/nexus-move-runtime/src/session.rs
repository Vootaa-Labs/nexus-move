use std::collections::BTreeMap;

use nexus_move_types::AccountAddress;

use crate::gas::{GasMeter, GasSchedule, SimpleGasMeter};
use crate::resources::ResourceStore;
use crate::state::NexusStateView;
use crate::types::{VmOutput, VmStatus};

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct MoveGasSummary {
    pub execution_gas: u64,
    pub io_gas: u64,
    pub storage_fee: u64,
}

impl MoveGasSummary {
    pub fn total(&self) -> u64 {
        self.execution_gas
            .saturating_add(self.io_gas)
            .saturating_add(self.storage_fee)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SessionKind {
    Publish,
    Execute,
    ReadOnly,
}

pub struct ExecuteSession<'a> {
    pub kind: SessionKind,
    pub sender: AccountAddress,
    pub meter: SimpleGasMeter,
    pub schedule: GasSchedule,
    pub resources: ResourceStore<'a>,
    pub gas_summary: MoveGasSummary,
}

impl<'a> ExecuteSession<'a> {
    pub fn new(
        kind: SessionKind,
        sender: AccountAddress,
        gas_limit: u64,
        schedule: GasSchedule,
        view: &'a NexusStateView<'a>,
    ) -> Self {
        Self {
            kind,
            sender,
            meter: SimpleGasMeter::new(gas_limit),
            schedule,
            resources: ResourceStore::new(view),
            gas_summary: MoveGasSummary::default(),
        }
    }

    pub fn charge_execution(&mut self, amount: u64) -> Result<(), WriteError> {
        self.gas_summary.execution_gas = self.gas_summary.execution_gas.saturating_add(amount);
        self.meter.charge(amount).map_err(|_| WriteError::OutOfGas)
    }

    pub fn charge_io(&mut self, amount: u64) -> Result<(), WriteError> {
        self.gas_summary.io_gas = self.gas_summary.io_gas.saturating_add(amount);
        self.meter.charge(amount).map_err(|_| WriteError::OutOfGas)
    }

    pub fn charge_storage(&mut self, amount: u64) -> Result<(), WriteError> {
        self.gas_summary.storage_fee = self.gas_summary.storage_fee.saturating_add(amount);
        self.meter.charge(amount).map_err(|_| WriteError::OutOfGas)
    }

    pub fn read_resource(
        &mut self,
        account: &AccountAddress,
        type_tag: &str,
    ) -> crate::types::VmResult<Option<Vec<u8>>> {
        let value = self.resources.get(account, type_tag)?;
        let read_cost = value
            .as_ref()
            .map(|bytes| (bytes.len() as u64).saturating_mul(self.schedule.read_per_byte))
            .unwrap_or(0);
        let _ = self.charge_io(read_cost);
        Ok(value)
    }

    pub fn write_resource(
        &mut self,
        account: AccountAddress,
        type_tag: &str,
        value: Vec<u8>,
    ) -> Result<(), WriteError> {
        if self.kind == SessionKind::ReadOnly {
            return Err(WriteError::ReadOnly);
        }

        let cost = (value.len() as u64).saturating_mul(self.schedule.write_per_byte);
        self.charge_storage(cost)?;
        self.resources.set(account, type_tag, value);
        Ok(())
    }

    pub fn commit(self, status: VmStatus) -> VmOutput {
        let (write_set, state_changes) = self.resources.into_changes();
        VmOutput {
            status,
            gas_used: self.meter.consumed(),
            state_changes,
            write_set,
            events: Vec::new(),
            return_values: Vec::new(),
        }
    }

    pub fn abort(self, status: VmStatus) -> VmOutput {
        VmOutput {
            status,
            gas_used: self.meter.consumed(),
            state_changes: Vec::new(),
            write_set: BTreeMap::new(),
            events: Vec::new(),
            return_values: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WriteError {
    ReadOnly,
    OutOfGas,
}
