use crate::config::VmConfig;
use crate::types::StateChange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasExhausted {
    pub needed: u64,
    pub available: u64,
}

impl std::fmt::Display for GasExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "out of gas: needed {} but only {} available",
            self.needed, self.available
        )
    }
}

impl std::error::Error for GasExhausted {}

pub trait GasMeter: Send + Sync {
    fn charge(&mut self, amount: u64) -> Result<(), GasExhausted>;
    fn remaining(&self) -> u64;
    fn consumed(&self) -> u64;
    fn limit(&self) -> u64;
}

pub struct SimpleGasMeter {
    limit: u64,
    consumed: u64,
}

impl SimpleGasMeter {
    pub fn new(limit: u64) -> Self {
        Self { limit, consumed: 0 }
    }
}

impl GasMeter for SimpleGasMeter {
    fn charge(&mut self, amount: u64) -> Result<(), GasExhausted> {
        let next = self.consumed.saturating_add(amount);
        if next > self.limit {
            Err(GasExhausted {
                needed: amount,
                available: self.remaining(),
            })
        } else {
            self.consumed = next;
            Ok(())
        }
    }

    fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }

    fn consumed(&self) -> u64 {
        self.consumed
    }

    fn limit(&self) -> u64 {
        self.limit
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GasSchedule {
    pub transfer_base: u64,
    pub call_base: u64,
    pub publish_base: u64,
    pub publish_per_byte: u64,
    pub read_per_byte: u64,
    pub write_per_byte: u64,
}

const DEFAULT_TRANSFER_BASE: u64 = 1_000;

impl GasSchedule {
    pub fn from_config(config: &VmConfig) -> Self {
        Self {
            transfer_base: DEFAULT_TRANSFER_BASE,
            call_base: config.call_base_gas,
            publish_base: config.publish_base_gas,
            publish_per_byte: config.publish_per_byte_gas,
            read_per_byte: config.read_per_byte_gas,
            write_per_byte: config.write_per_byte_gas,
        }
    }
}

impl Default for GasSchedule {
    fn default() -> Self {
        Self::from_config(&VmConfig::default())
    }
}

pub fn publish_gas_cost(schedule: &GasSchedule, total_bytes: u64) -> u64 {
    schedule
        .publish_base
        .saturating_add(total_bytes.saturating_mul(schedule.publish_per_byte))
}

pub fn write_gas_cost(schedule: &GasSchedule, size: u64) -> u64 {
    size.saturating_mul(schedule.write_per_byte)
}

pub fn read_gas_cost(schedule: &GasSchedule, size: u64) -> u64 {
    size.saturating_mul(schedule.read_per_byte)
}

fn encoded_chunks_len(chunks: &[Vec<u8>]) -> u64 {
    chunks.iter().fold(0u64, |total, chunk| {
        total.saturating_add(chunk.len() as u64)
    })
}

fn state_change_bytes(state_changes: &[StateChange]) -> u64 {
    state_changes.iter().fold(0u64, |total, change| {
        let key_len = change.key.len() as u64;
        let value_len = change
            .value
            .as_ref()
            .map(|value| value.len() as u64)
            .unwrap_or(0);
        total.saturating_add(key_len).saturating_add(value_len)
    })
}

pub fn clamp_gas_to_limit(estimated: u64, gas_limit: u64) -> u64 {
    estimated.min(gas_limit)
}

pub fn estimate_call_gas(
    schedule: &GasSchedule,
    type_args: &[Vec<u8>],
    args: &[Vec<u8>],
    state_changes: &[StateChange],
) -> u64 {
    let input_bytes = encoded_chunks_len(type_args).saturating_add(encoded_chunks_len(args));
    schedule
        .call_base
        .saturating_add(read_gas_cost(schedule, input_bytes))
        .saturating_add(write_gas_cost(schedule, state_change_bytes(state_changes)))
}

pub fn estimate_publish_gas(
    schedule: &GasSchedule,
    modules: &[Vec<u8>],
    state_changes: &[StateChange],
) -> u64 {
    let module_bytes = encoded_chunks_len(modules);
    publish_gas_cost(schedule, module_bytes)
        .saturating_add(write_gas_cost(schedule, state_change_bytes(state_changes)))
}

pub fn estimate_script_gas(
    schedule: &GasSchedule,
    bytecode: &[u8],
    type_args: &[Vec<u8>],
    args: &[Vec<u8>],
) -> u64 {
    let input_bytes = (bytecode.len() as u64)
        .saturating_add(encoded_chunks_len(type_args))
        .saturating_add(encoded_chunks_len(args));
    schedule
        .call_base
        .saturating_add(read_gas_cost(schedule, input_bytes))
}
