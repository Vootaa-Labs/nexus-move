//! Nexus gas meter for the upstream Move VM interpreter.
//!
//! Bridges between Nexus's simple I/O-based gas model and the upstream
//! per-instruction `move_vm_types::gas::GasMeter` trait.
//!
//! All instructions are charged a small flat cost (configurable via
//! `NexusGasParameters`).  This avoids the complexity of per-opcode
//! pricing tables while still providing bounded execution.

use move_binary_format::errors::PartialVMResult;
use move_binary_format::file_format::CodeOffset;
use move_core_types::account_address::AccountAddress;
use move_core_types::gas_algebra::{InternalGas, NumArgs, NumBytes, NumTypeNodes};
use move_core_types::identifier::IdentStr;
use move_core_types::language_storage::ModuleId;
use move_vm_types::gas::{GasMeter, SimpleInstruction};
use move_vm_types::views::{TypeView, ValueView};

// ── Gas parameters ──────────────────────────────────────────────────────

/// Flat cost parameters for Nexus's simplified gas charging.
///
/// All costs are in internal gas units.  The conversion ratio between
/// internal gas and external (user-visible) gas is 1:1 for devnet.
#[derive(Debug, Clone)]
pub struct NexusGasParameters {
    /// Cost per simple instruction (arithmetic, comparison, branch, etc.)
    pub instruction_base: u64,
    /// Cost per function call / return.
    pub call_base: u64,
    /// Per-byte cost when loading constants.
    pub load_const_per_byte: u64,
    /// Per-byte cost when loading a resource from storage.
    pub load_resource_per_byte: u64,
    /// Cost per struct pack/unpack.
    pub pack_unpack_base: u64,
    /// Cost per borrow_global / exists / move_from / move_to.
    pub global_op_base: u64,
    /// Cost per read_ref / write_ref.
    pub ref_op_base: u64,
    /// Cost per vector operation.
    pub vec_op_base: u64,
    /// Cost per native function call (flat, plus whatever the native itself charges).
    pub native_call_base: u64,
    /// Per-byte cost for module dependency loading.
    pub dependency_per_byte: u64,
}

impl Default for NexusGasParameters {
    fn default() -> Self {
        Self {
            instruction_base: 1,
            call_base: 10,
            load_const_per_byte: 1,
            load_resource_per_byte: 2,
            pack_unpack_base: 3,
            global_op_base: 5,
            ref_op_base: 2,
            vec_op_base: 3,
            native_call_base: 10,
            dependency_per_byte: 1,
        }
    }
}

// ── NexusGasMeter ───────────────────────────────────────────────────────

/// A gas meter that implements the upstream Move VM's per-instruction
/// `GasMeter` trait using Nexus's simplified flat-cost model.
///
/// **Invariant**: `consumed ≤ limit` at all times, or an `OutOfGas`
/// error is returned from the next charge.
pub struct NexusGasMeter {
    params: NexusGasParameters,
    limit: u64,
    consumed: u64,
}

impl NexusGasMeter {
    pub fn new(limit: u64, params: NexusGasParameters) -> Self {
        Self {
            params,
            limit,
            consumed: 0,
        }
    }

    /// Create a gas meter with default parameters.
    pub fn with_limit(limit: u64) -> Self {
        Self::new(limit, NexusGasParameters::default())
    }

    /// How many gas units have been consumed so far.
    pub fn consumed(&self) -> u64 {
        self.consumed
    }

    /// The gas limit.
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Remaining gas before exhaustion.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }

    fn charge(&mut self, amount: u64) -> PartialVMResult<()> {
        let next = self.consumed.saturating_add(amount);
        if next > self.limit {
            Err(move_binary_format::errors::PartialVMError::new(
                move_core_types::vm_status::StatusCode::OUT_OF_GAS,
            ))
        } else {
            self.consumed = next;
            Ok(())
        }
    }
}

// ── upstream GasMeter trait implementation ───────────────────────────────

impl GasMeter for NexusGasMeter {
    fn balance_internal(&self) -> InternalGas {
        InternalGas::new(self.remaining())
    }

    fn charge_simple_instr(&mut self, _instr: SimpleInstruction) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_br_true(&mut self, _target_offset: Option<CodeOffset>) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_br_false(&mut self, _target_offset: Option<CodeOffset>) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_branch(&mut self, _target_offset: CodeOffset) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_pop(&mut self, _popped_val: impl ValueView) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_call(
        &mut self,
        _module_id: &ModuleId,
        _func_name: &str,
        _args: impl ExactSizeIterator<Item = impl ValueView> + Clone,
        _num_locals: NumArgs,
    ) -> PartialVMResult<()> {
        self.charge(self.params.call_base)
    }

    fn charge_call_generic(
        &mut self,
        _module_id: &ModuleId,
        _func_name: &str,
        _ty_args: impl ExactSizeIterator<Item = impl TypeView> + Clone,
        _args: impl ExactSizeIterator<Item = impl ValueView> + Clone,
        _num_locals: NumArgs,
    ) -> PartialVMResult<()> {
        self.charge(self.params.call_base)
    }

    fn charge_ld_const(&mut self, size: NumBytes) -> PartialVMResult<()> {
        self.charge(
            self.params
                .instruction_base
                .saturating_add(u64::from(size).saturating_mul(self.params.load_const_per_byte)),
        )
    }

    fn charge_ld_const_after_deserialization(
        &mut self,
        _val: impl ValueView,
    ) -> PartialVMResult<()> {
        Ok(())
    }

    fn charge_copy_loc(&mut self, _val: impl ValueView) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_move_loc(&mut self, _val: impl ValueView) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_store_loc(&mut self, _val: impl ValueView) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_pack(
        &mut self,
        _is_generic: bool,
        _args: impl ExactSizeIterator<Item = impl ValueView> + Clone,
    ) -> PartialVMResult<()> {
        self.charge(self.params.pack_unpack_base)
    }

    fn charge_unpack(
        &mut self,
        _is_generic: bool,
        _args: impl ExactSizeIterator<Item = impl ValueView> + Clone,
    ) -> PartialVMResult<()> {
        self.charge(self.params.pack_unpack_base)
    }

    fn charge_read_ref(&mut self, _val: impl ValueView) -> PartialVMResult<()> {
        self.charge(self.params.ref_op_base)
    }

    fn charge_write_ref(
        &mut self,
        _new_val: impl ValueView,
        _old_val: impl ValueView,
    ) -> PartialVMResult<()> {
        self.charge(self.params.ref_op_base)
    }

    fn charge_eq(&mut self, _lhs: impl ValueView, _rhs: impl ValueView) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_neq(&mut self, _lhs: impl ValueView, _rhs: impl ValueView) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_borrow_global(
        &mut self,
        _is_mut: bool,
        _is_generic: bool,
        _ty: impl TypeView,
        _is_success: bool,
    ) -> PartialVMResult<()> {
        self.charge(self.params.global_op_base)
    }

    fn charge_exists(
        &mut self,
        _is_generic: bool,
        _ty: impl TypeView,
        _exists: bool,
    ) -> PartialVMResult<()> {
        self.charge(self.params.global_op_base)
    }

    fn charge_move_from(
        &mut self,
        _is_generic: bool,
        _ty: impl TypeView,
        _val: Option<impl ValueView>,
    ) -> PartialVMResult<()> {
        self.charge(self.params.global_op_base)
    }

    fn charge_move_to(
        &mut self,
        _is_generic: bool,
        _ty: impl TypeView,
        _val: impl ValueView,
        _is_success: bool,
    ) -> PartialVMResult<()> {
        self.charge(self.params.global_op_base)
    }

    fn charge_vec_pack<'a>(
        &mut self,
        _ty: impl TypeView + 'a,
        _args: impl ExactSizeIterator<Item = impl ValueView> + Clone,
    ) -> PartialVMResult<()> {
        self.charge(self.params.vec_op_base)
    }

    fn charge_vec_len(&mut self, _ty: impl TypeView) -> PartialVMResult<()> {
        self.charge(self.params.vec_op_base)
    }

    fn charge_vec_borrow(
        &mut self,
        _is_mut: bool,
        _ty: impl TypeView,
        _is_success: bool,
    ) -> PartialVMResult<()> {
        self.charge(self.params.vec_op_base)
    }

    fn charge_vec_push_back(
        &mut self,
        _ty: impl TypeView,
        _val: impl ValueView,
    ) -> PartialVMResult<()> {
        self.charge(self.params.vec_op_base)
    }

    fn charge_vec_pop_back(
        &mut self,
        _ty: impl TypeView,
        _val: Option<impl ValueView>,
    ) -> PartialVMResult<()> {
        self.charge(self.params.vec_op_base)
    }

    fn charge_vec_unpack(
        &mut self,
        _ty: impl TypeView,
        _expect_num_elements: NumArgs,
        _elems: impl ExactSizeIterator<Item = impl ValueView> + Clone,
    ) -> PartialVMResult<()> {
        self.charge(self.params.vec_op_base)
    }

    fn charge_vec_swap(&mut self, _ty: impl TypeView) -> PartialVMResult<()> {
        self.charge(self.params.vec_op_base)
    }

    fn charge_load_resource(
        &mut self,
        _addr: AccountAddress,
        _ty: impl TypeView,
        _val: Option<impl ValueView>,
        bytes_loaded: NumBytes,
    ) -> PartialVMResult<()> {
        self.charge(u64::from(bytes_loaded).saturating_mul(self.params.load_resource_per_byte))
    }

    fn charge_native_function(
        &mut self,
        amount: InternalGas,
        _ret_vals: Option<impl ExactSizeIterator<Item = impl ValueView> + Clone>,
    ) -> PartialVMResult<()> {
        self.charge(
            self.params
                .native_call_base
                .saturating_add(u64::from(amount)),
        )
    }

    fn charge_native_function_before_execution(
        &mut self,
        _ty_args: impl ExactSizeIterator<Item = impl TypeView> + Clone,
        _args: impl ExactSizeIterator<Item = impl ValueView> + Clone,
    ) -> PartialVMResult<()> {
        Ok(())
    }

    fn charge_drop_frame(
        &mut self,
        _locals: impl Iterator<Item = impl ValueView> + Clone,
    ) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_create_ty(&mut self, _num_nodes: NumTypeNodes) -> PartialVMResult<()> {
        self.charge(self.params.instruction_base)
    }

    fn charge_dependency(
        &mut self,
        _is_new: bool,
        _addr: &AccountAddress,
        _name: &IdentStr,
        size: NumBytes,
    ) -> PartialVMResult<()> {
        self.charge(u64::from(size).saturating_mul(self.params.dependency_per_byte))
    }

    fn charge_heap_memory(&mut self, amount: u64) -> PartialVMResult<()> {
        self.charge(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges_until_exhausted() {
        let mut meter = NexusGasMeter::with_limit(100);
        assert_eq!(meter.remaining(), 100);
        assert_eq!(meter.consumed(), 0);

        meter.charge(50).unwrap();
        assert_eq!(meter.remaining(), 50);
        assert_eq!(meter.consumed(), 50);

        meter.charge(50).unwrap();
        assert_eq!(meter.remaining(), 0);
        assert_eq!(meter.consumed(), 100);

        // Next charge should fail
        let err = meter.charge(1).unwrap_err();
        assert_eq!(
            err.major_status(),
            move_core_types::vm_status::StatusCode::OUT_OF_GAS
        );
        // Consumed should not have changed
        assert_eq!(meter.consumed(), 100);
    }

    #[test]
    fn simple_instr_costs_default_1() {
        let mut meter = NexusGasMeter::with_limit(1000);
        meter.charge_simple_instr(SimpleInstruction::Add).unwrap();
        assert_eq!(meter.consumed(), 1);
    }

    #[test]
    fn call_costs_default_10() {
        use move_core_types::identifier::Identifier;
        use move_vm_types::values::Value;
        let mut meter = NexusGasMeter::with_limit(1000);
        let module_id = ModuleId::new(AccountAddress::ONE, Identifier::new("m").unwrap());
        meter
            .charge_call(
                &module_id,
                "f",
                std::iter::empty::<Value>(),
                NumArgs::new(0),
            )
            .unwrap();
        assert_eq!(meter.consumed(), 10);
    }

    #[test]
    fn balance_internal_reflects_remaining() {
        let meter = NexusGasMeter::with_limit(500);
        assert_eq!(u64::from(meter.balance_internal()), 500);
    }

    #[test]
    fn zero_limit_reject_any_charge() {
        let mut meter = NexusGasMeter::with_limit(0);
        assert!(meter.charge(1).is_err());
        assert_eq!(meter.consumed(), 0);
    }
}
