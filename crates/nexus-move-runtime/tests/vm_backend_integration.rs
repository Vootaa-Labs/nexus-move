//! Integration test: deploy and execute a real Move contract through
//! the vendored Move VM runtime.

#![cfg(feature = "vm-backend")]

use std::collections::BTreeMap;

use nexus_move_runtime::config::VmConfig;
use nexus_move_runtime::state::{NexusStateView, StateReader};
use nexus_move_runtime::types::{FunctionCall, ModulePublish, QueryRequest, VmResult, VmStatus};
use nexus_move_runtime::vm_backend::RealMoveVm;
use nexus_move_runtime::MoveVm;
use nexus_move_types::AccountAddress;

/// Simple in-memory state for testing.
struct MemState {
    data: BTreeMap<(AccountAddress, Vec<u8>), Vec<u8>>,
}

impl MemState {
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }
}

impl StateReader for MemState {
    fn get(&self, account: &AccountAddress, key: &[u8]) -> VmResult<Option<Vec<u8>>> {
        Ok(self.data.get(&(*account, key.to_vec())).cloned())
    }
}

/// Build the 0xCAFE address (matching counter's dev-addresses).
fn cafe_address() -> AccountAddress {
    let mut bytes = [0u8; 32];
    bytes[30] = 0xCA;
    bytes[31] = 0xFE;
    AccountAddress(bytes)
}

#[test]
fn real_vm_publishes_counter_module() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();

    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    let view = NexusStateView::new(&state);
    let request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };

    let output = vm
        .publish_modules(&view, &request)
        .expect("publish should succeed");
    assert_eq!(
        output.status,
        VmStatus::Success,
        "publish status: {:?}",
        output.status
    );
    assert!(
        !output.state_changes.is_empty(),
        "should produce state changes"
    );

    // Apply state changes.
    for change in &output.state_changes {
        if let Some(ref value) = change.value {
            state
                .data
                .insert((change.account, change.key.clone()), value.clone());
        }
    }

    // Verify module is stored.
    let view = NexusStateView::new(&state);
    assert!(
        view.get_module(&deployer).unwrap().is_some(),
        "module should be stored"
    );
}

#[test]
fn real_vm_counter_lifecycle() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();

    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    // Step 1: Publish.
    let view = NexusStateView::new(&state);
    let pub_request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let pub_output = vm.publish_modules(&view, &pub_request).unwrap();
    assert_eq!(pub_output.status, VmStatus::Success);
    for change in &pub_output.state_changes {
        if let Some(ref value) = change.value {
            state
                .data
                .insert((change.account, change.key.clone()), value.clone());
        }
    }

    // Step 2: Initialize.
    let view = NexusStateView::new(&state);
    let init_request = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::initialize".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let init_output = vm.execute_function(&view, &init_request).unwrap();
    assert_eq!(
        init_output.status,
        VmStatus::Success,
        "initialize status: {:?}",
        init_output.status
    );
    for change in &init_output.state_changes {
        if let Some(ref value) = change.value {
            state
                .data
                .insert((change.account, change.key.clone()), value.clone());
        }
    }

    // Step 3: Query the counter (should be 0).
    let view = NexusStateView::new(&state);
    let query = nexus_move_runtime::QueryRequest {
        contract: deployer,
        function: "counter::get_count".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer)],
        gas_budget: 10_000,
    };
    let result = vm.query_view(&view, &query).unwrap();
    assert!(result.return_value.is_some());
    let count = u64::from_le_bytes(result.return_value.unwrap().try_into().unwrap());
    assert_eq!(count, 0, "initial counter value should be 0");
}

/// BCS-encode a Move address for use as function argument.
fn bcs_address(addr: &AccountAddress) -> Vec<u8> {
    use move_core_types::account_address::AccountAddress as MoveAddress;
    bcs::to_bytes(&MoveAddress::new(addr.0)).unwrap()
}

/// Helper: deploy counter, apply state changes, call initialize, apply.
fn deploy_and_init_counter() -> (MemState, RealMoveVm, AccountAddress) {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    // Publish.
    let view = NexusStateView::new(&state);
    let pub_request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let pub_output = vm.publish_modules(&view, &pub_request).unwrap();
    assert_eq!(pub_output.status, VmStatus::Success);
    apply_changes(&mut state, &pub_output.state_changes);

    // Initialize.
    let view = NexusStateView::new(&state);
    let init_request = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::initialize".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let init_output = vm.execute_function(&view, &init_request).unwrap();
    assert_eq!(
        init_output.status,
        VmStatus::Success,
        "init: {:?}",
        init_output.status
    );
    apply_changes(&mut state, &init_output.state_changes);

    (state, vm, deployer)
}

fn apply_changes(state: &mut MemState, changes: &[nexus_move_runtime::types::StateChange]) {
    for change in changes {
        if let Some(ref value) = change.value {
            state
                .data
                .insert((change.account, change.key.clone()), value.clone());
        }
    }
}

fn query_count(vm: &RealMoveVm, state: &MemState, deployer: &AccountAddress) -> u64 {
    let view = NexusStateView::new(state);
    let query = nexus_move_runtime::QueryRequest {
        contract: *deployer,
        function: "counter::get_count".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(deployer)],
        gas_budget: 10_000,
    };
    let result = vm.query_view(&view, &query).unwrap();
    let bytes = result.return_value.expect("should return a value");
    u64::from_le_bytes(bytes.try_into().unwrap())
}

#[test]
fn real_vm_counter_increment() {
    let (mut state, vm, deployer) = deploy_and_init_counter();

    // Counter starts at 0.
    assert_eq!(query_count(&vm, &state, &deployer), 0);

    // Increment once.
    let view = NexusStateView::new(&state);
    let inc_request = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::increment".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let inc_output = vm.execute_function(&view, &inc_request).unwrap();
    assert_eq!(
        inc_output.status,
        VmStatus::Success,
        "increment: {:?}",
        inc_output.status
    );
    apply_changes(&mut state, &inc_output.state_changes);

    // Counter should be 1.
    assert_eq!(query_count(&vm, &state, &deployer), 1);
}

#[test]
fn real_vm_counter_multiple_increments() {
    let (mut state, vm, deployer) = deploy_and_init_counter();

    for expected in 1..=5u64 {
        let view = NexusStateView::new(&state);
        let inc_request = FunctionCall {
            sender: deployer,
            contract: deployer,
            function: "counter::increment".into(),
            type_args: Vec::new(),
            args: Vec::new(),
            gas_limit: 100_000,
        };
        let inc_output = vm.execute_function(&view, &inc_request).unwrap();
        assert_eq!(
            inc_output.status,
            VmStatus::Success,
            "increment #{expected}: {:?}",
            inc_output.status
        );
        apply_changes(&mut state, &inc_output.state_changes);
        assert_eq!(query_count(&vm, &state, &deployer), expected);
    }
}

// ── Token contract integration tests ────────────────────────────────────

/// Deploy the token module and initialise.
fn deploy_and_init_token() -> (MemState, RealMoveVm, AccountAddress) {
    let token_bytes = include_bytes!("fixtures/token.mv");
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    // Publish.
    let view = NexusStateView::new(&state);
    let pub_request = ModulePublish {
        sender: deployer,
        modules: vec![token_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let pub_output = vm.publish_modules(&view, &pub_request).unwrap();
    assert_eq!(pub_output.status, VmStatus::Success);
    apply_changes(&mut state, &pub_output.state_changes);

    // Initialize.
    let view = NexusStateView::new(&state);
    let init_request = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::initialize".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let init_output = vm.execute_function(&view, &init_request).unwrap();
    assert_eq!(
        init_output.status,
        VmStatus::Success,
        "token init: {:?}",
        init_output.status
    );
    apply_changes(&mut state, &init_output.state_changes);

    (state, vm, deployer)
}

/// BCS-encode a u64 for use as a function argument.
fn bcs_u64(v: u64) -> Vec<u8> {
    bcs::to_bytes(&v).unwrap()
}

fn query_balance(
    vm: &RealMoveVm,
    state: &MemState,
    contract: &AccountAddress,
    addr: &AccountAddress,
) -> u64 {
    let view = NexusStateView::new(state);
    let query = QueryRequest {
        contract: *contract,
        function: "token::balance_of".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(addr)],
        gas_budget: 100_000,
    };
    let result = vm.query_view(&view, &query).unwrap();
    let bytes = result.return_value.expect("should return a value");
    u64::from_le_bytes(bytes.try_into().unwrap())
}

fn query_total_supply(vm: &RealMoveVm, state: &MemState, deployer: &AccountAddress) -> u64 {
    let view = NexusStateView::new(state);
    let query = QueryRequest {
        contract: *deployer,
        function: "token::total_supply".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(deployer)],
        gas_budget: 100_000,
    };
    let result = vm.query_view(&view, &query).unwrap();
    let bytes = result.return_value.expect("should return a value");
    u64::from_le_bytes(bytes.try_into().unwrap())
}

#[test]
fn token_initialize_creates_zero_balance() {
    let (state, vm, deployer) = deploy_and_init_token();
    assert_eq!(query_balance(&vm, &state, &deployer, &deployer), 0);
    assert_eq!(query_total_supply(&vm, &state, &deployer), 0);
}

#[test]
fn token_mint_increases_balance_and_supply() {
    let (mut state, vm, deployer) = deploy_and_init_token();

    // Mint 1000 tokens to deployer.
    let view = NexusStateView::new(&state);
    let mint_request = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::mint".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer), bcs_u64(1000)],
        gas_limit: 100_000,
    };
    let output = vm.execute_function(&view, &mint_request).unwrap();
    assert_eq!(
        output.status,
        VmStatus::Success,
        "mint: {:?}",
        output.status
    );
    apply_changes(&mut state, &output.state_changes);

    assert_eq!(query_balance(&vm, &state, &deployer, &deployer), 1000);
    assert_eq!(query_total_supply(&vm, &state, &deployer), 1000);
}

#[test]
fn token_transfer_moves_funds() {
    let (mut state, vm, deployer) = deploy_and_init_token();

    // Mint 500 tokens to deployer.
    let view = NexusStateView::new(&state);
    let mint = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::mint".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer), bcs_u64(500)],
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &mint).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    apply_changes(&mut state, &out.state_changes);

    // Create a second user with a Balance resource.
    // The token module only credits existing balances in mint/transfer,
    // so we need the deployer to also initialize the recipient.
    // Since there's no register(recipient) function, we can only transfer
    // between accounts that already have Balance, i.e. the deployer itself.
    // Exercise a self-transfer (verifies accounting).
    let view = NexusStateView::new(&state);
    let transfer = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::transfer".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer), bcs_u64(200)],
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &transfer).unwrap();
    assert_eq!(out.status, VmStatus::Success, "transfer: {:?}", out.status);
    apply_changes(&mut state, &out.state_changes);

    // Self-transfer: balance stays the same.
    assert_eq!(query_balance(&vm, &state, &deployer, &deployer), 500);
    assert_eq!(query_total_supply(&vm, &state, &deployer), 500);
}

#[test]
fn token_burn_decreases_balance_and_supply() {
    let (mut state, vm, deployer) = deploy_and_init_token();

    // Mint 1000, then burn 300.
    let view = NexusStateView::new(&state);
    let mint = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::mint".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer), bcs_u64(1000)],
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &mint).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    apply_changes(&mut state, &out.state_changes);

    let view = NexusStateView::new(&state);
    let burn = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::burn".into(),
        type_args: Vec::new(),
        args: vec![bcs_u64(300)],
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &burn).unwrap();
    assert_eq!(out.status, VmStatus::Success, "burn: {:?}", out.status);
    apply_changes(&mut state, &out.state_changes);

    assert_eq!(query_balance(&vm, &state, &deployer, &deployer), 700);
    assert_eq!(query_total_supply(&vm, &state, &deployer), 700);
}

#[test]
fn token_insufficient_balance_aborts() {
    let (mut state, vm, deployer) = deploy_and_init_token();

    // Mint 10 then try to burn 100 -- should abort.
    let view = NexusStateView::new(&state);
    let mint = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::mint".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer), bcs_u64(10)],
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &mint).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    apply_changes(&mut state, &out.state_changes);

    let view = NexusStateView::new(&state);
    let burn = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "token::burn".into(),
        type_args: Vec::new(),
        args: vec![bcs_u64(100)],
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &burn).unwrap();
    assert!(
        matches!(out.status, VmStatus::MoveAbort { .. }),
        "expected MoveAbort, got: {:?}",
        out.status
    );
}

// ── Gas metering integration tests ──────────────────────────────────────

#[test]
fn gas_used_is_nonzero_for_function_call() {
    let (mut state, vm, deployer) = deploy_and_init_counter();

    let view = NexusStateView::new(&state);
    let inc = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::increment".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &inc).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    assert!(
        out.gas_used > 0,
        "gas_used should be > 0 for real execution, got {}",
        out.gas_used
    );
    apply_changes(&mut state, &out.state_changes);
}

#[test]
fn gas_used_is_nonzero_for_query_view() {
    let (state, vm, deployer) = deploy_and_init_counter();

    let view = NexusStateView::new(&state);
    let query = QueryRequest {
        contract: deployer,
        function: "counter::get_count".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer)],
        gas_budget: 100_000,
    };
    let result = vm.query_view(&view, &query).unwrap();
    assert!(
        result.gas_used > 0,
        "query gas_used should be > 0, got {}",
        result.gas_used
    );
}

#[test]
fn more_work_consumes_more_gas() {
    let (mut state, vm, deployer) = deploy_and_init_counter();

    // Single increment.
    let view = NexusStateView::new(&state);
    let inc = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::increment".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let out1 = vm.execute_function(&view, &inc).unwrap();
    assert_eq!(out1.status, VmStatus::Success);
    let gas_one = out1.gas_used;
    apply_changes(&mut state, &out1.state_changes);

    // Token init in a separate state to avoid address collision.
    let (_, vm2, deployer2) = deploy_and_init_token();
    // Token init touches more globals (MintCapability + Balance) so the
    // deploy_and_init_token init step is heavier.  Measure it directly.
    let (tok_state, _, _) = deploy_and_init_token();
    let view = NexusStateView::new(&tok_state);
    let mint = FunctionCall {
        sender: deployer2,
        contract: deployer2,
        function: "token::mint".into(),
        type_args: Vec::new(),
        args: vec![bcs_address(&deployer2), bcs_u64(100)],
        gas_limit: 100_000,
    };
    let mint_out = vm2.execute_function(&view, &mint).unwrap();
    assert_eq!(mint_out.status, VmStatus::Success);

    // Token mint reads MintCapability + Balance and writes both back,
    // so it should use more gas than a simple counter increment.
    assert!(
        mint_out.gas_used > gas_one,
        "token mint ({}) should use more gas than counter increment ({})",
        mint_out.gas_used,
        gas_one,
    );
}

#[test]
fn low_gas_limit_causes_out_of_gas() {
    let (state, vm, deployer) = deploy_and_init_counter();

    // Use a very small gas limit -- should cause OUT_OF_GAS.
    let view = NexusStateView::new(&state);
    let inc = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::increment".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 1, // absurdly low
    };
    let out = vm.execute_function(&view, &inc).unwrap();
    assert!(
        matches!(out.status, VmStatus::MoveAbort { .. }),
        "expected abort due to low gas, got: {:?}",
        out.status,
    );
}

// ── Upgrade policy enforcement tests ────────────────────────────────────

use nexus_move_runtime::types::UpgradePolicy;

#[test]
fn immutable_module_rejects_redeploy() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    // First deploy (immutable by default).
    let view = NexusStateView::new(&state);
    let request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let out = vm.publish_modules(&view, &request).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    apply_changes(&mut state, &out.state_changes);

    // Second deploy of same module → should be rejected (code 20).
    let view = NexusStateView::new(&state);
    let out2 = vm.publish_modules(&view, &request).unwrap();
    assert!(
        matches!(out2.status, VmStatus::MoveAbort { code: 20, .. }),
        "expected immutable rejection (code 20), got: {:?}",
        out2.status,
    );
    assert!(
        out2.state_changes.is_empty(),
        "no state changes on rejection"
    );
}

#[test]
fn compatible_module_allows_redeploy() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    // First deploy with Compatible policy.
    let view = NexusStateView::new(&state);
    let request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: Some(UpgradePolicy::Compatible),
    };
    let out = vm.publish_modules(&view, &request).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    apply_changes(&mut state, &out.state_changes);

    // Second deploy of same module → should succeed.
    let view = NexusStateView::new(&state);
    let out2 = vm.publish_modules(&view, &request).unwrap();
    assert_eq!(
        out2.status,
        VmStatus::Success,
        "compatible redeploy: {:?}",
        out2.status
    );
    assert!(
        !out2.state_changes.is_empty(),
        "should produce state changes"
    );
}

#[test]
fn metadata_is_stored_on_publish() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    let view = NexusStateView::new(&state);
    let request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let out = vm.publish_modules(&view, &request).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    apply_changes(&mut state, &out.state_changes);

    // Verify metadata was stored.
    let meta_bytes = state
        .data
        .get(&(deployer, b"package_metadata".to_vec()))
        .expect("metadata should be stored");
    let meta: nexus_move_package::PackageMetadata =
        bcs::from_bytes(meta_bytes).expect("metadata should deserialize");
    assert_eq!(meta.name, "counter");
    assert_eq!(meta.deployer, deployer);
    assert_eq!(
        meta.upgrade_policy,
        nexus_move_package::UpgradePolicy::Immutable
    );
}

// ── Multi-module storage tests ──────────────────────────────────────────

#[test]
fn publish_writes_per_module_key() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    let view = NexusStateView::new(&state);
    let request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let out = vm.publish_modules(&view, &request).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    apply_changes(&mut state, &out.state_changes);

    // Per-module key "code::counter" should exist.
    assert!(
        state
            .data
            .get(&(deployer, b"code::counter".to_vec()))
            .is_some(),
        "per-module key 'code::counter' should be stored"
    );

    // Legacy key "code" should also exist (backwards compat).
    assert!(
        state.data.get(&(deployer, b"code".to_vec())).is_some(),
        "legacy key 'code' should also be stored"
    );

    // Both should contain the same bytes.
    let per_module = state
        .data
        .get(&(deployer, b"code::counter".to_vec()))
        .unwrap();
    let legacy = state.data.get(&(deployer, b"code".to_vec())).unwrap();
    assert_eq!(
        per_module, legacy,
        "per-module and legacy keys should have same bytecode"
    );
}

#[test]
fn state_view_get_module_by_name_prefers_per_module_key() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    let view = NexusStateView::new(&state);
    let request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let out = vm.publish_modules(&view, &request).unwrap();
    apply_changes(&mut state, &out.state_changes);

    // The state view should find the module by name.
    let view = NexusStateView::new(&state);
    let module_bytes = view.get_module_by_name(&deployer, "counter").unwrap();
    assert!(
        module_bytes.is_some(),
        "get_module_by_name should find 'counter'"
    );
    assert_eq!(module_bytes.unwrap(), counter_bytes.to_vec());
}

#[test]
fn state_view_get_module_by_name_falls_back_to_legacy() {
    let deployer = cafe_address();
    let mut state = MemState::new();

    // Write ONLY the legacy key (simulating pre-migration data).
    let fake_module = vec![0xA1, 0x1C, 0xEB, 0x0B]; // fake magic bytes
    state
        .data
        .insert((deployer, b"code".to_vec()), fake_module.clone());

    let view = NexusStateView::new(&state);
    let result = view.get_module_by_name(&deployer, "anything").unwrap();
    assert_eq!(
        result.unwrap(),
        fake_module,
        "should fall back to legacy 'code' key"
    );
}

// ── Return value capture tests ──────────────────────────────────────────

#[test]
fn execute_function_captures_empty_return_values() {
    let (state, vm, deployer) = deploy_and_init_counter();

    // counter::increment returns nothing.
    let view = NexusStateView::new(&state);
    let inc = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::increment".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &inc).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    assert!(
        out.return_values.is_empty(),
        "increment has no return values"
    );
}

#[test]
fn execute_function_events_field_is_present() {
    let (state, vm, deployer) = deploy_and_init_counter();

    let view = NexusStateView::new(&state);
    let inc = FunctionCall {
        sender: deployer,
        contract: deployer,
        function: "counter::increment".into(),
        type_args: Vec::new(),
        args: Vec::new(),
        gas_limit: 100_000,
    };
    let out = vm.execute_function(&view, &inc).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    // Counter contract doesn't emit events, so events should be empty.
    assert!(out.events.is_empty(), "counter::increment emits no events");
}

#[test]
fn publish_has_empty_events_and_return_values() {
    let counter_bytes = include_bytes!("fixtures/counter.mv");
    let deployer = cafe_address();
    let state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    let view = NexusStateView::new(&state);
    let request = ModulePublish {
        sender: deployer,
        modules: vec![counter_bytes.to_vec()],
        gas_limit: 100_000,
        upgrade_policy: None,
    };
    let out = vm.publish_modules(&view, &request).unwrap();
    assert_eq!(out.status, VmStatus::Success);
    assert!(out.events.is_empty(), "publish should have no events");
    assert!(
        out.return_values.is_empty(),
        "publish should have no return values"
    );
}

#[test]
fn query_view_still_works_with_event_extension() {
    let (state, vm, deployer) = deploy_and_init_counter();
    let count = query_count(&vm, &state, &deployer);
    assert_eq!(
        count, 0,
        "query_view should still work with event extension registered"
    );
}

// ── Event store unit tests ──────────────────────────────────────────────

#[test]
fn event_store_accumulates_events() {
    use nexus_move_stdlib::ContractEvent;
    use nexus_move_stdlib::NexusEventStore;

    let mut store = NexusEventStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    store.push(ContractEvent {
        type_tag: "0xCAFE::counter::Increment".into(),
        guid: vec![1, 2, 3],
        sequence_number: 0,
        data: vec![42],
    });
    store.push(ContractEvent {
        type_tag: "0xCAFE::counter::Increment".into(),
        guid: vec![1, 2, 3],
        sequence_number: 1,
        data: vec![43],
    });

    assert_eq!(store.len(), 2);
    let events = store.drain();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].sequence_number, 0);
    assert_eq!(events[1].sequence_number, 1);
}

// ── Gap #4: Event capture through real Move VM execution ────────────────

/// Emitter contract fixture (compiled from examples/emitter).
const EMITTER_MV: &[u8] = include_bytes!("fixtures/emitter.mv");

/// Proves that events emitted by a Move contract's `event::emit_event`
/// are captured in `VmOutput.events` through the real VM execution path.
#[test]
fn emitter_contract_captures_events() {
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    // Publish emitter module.
    let view = NexusStateView::new(&state);
    let pub_output = vm
        .publish_modules(
            &view,
            &ModulePublish {
                sender: deployer,
                modules: vec![EMITTER_MV.to_vec()],
                gas_limit: 100_000,
                upgrade_policy: None,
            },
        )
        .expect("emitter publish should succeed");
    assert_eq!(pub_output.status, VmStatus::Success);
    apply_changes(&mut state, &pub_output.state_changes);

    // Initialize emitter (creates EventHandle + Emitter resource).
    let view = NexusStateView::new(&state);
    let init_output = vm
        .execute_function(
            &view,
            &FunctionCall {
                sender: deployer,
                contract: deployer,
                function: "emitter::initialize".into(),
                type_args: Vec::new(),
                args: Vec::new(),
                gas_limit: 100_000,
            },
        )
        .expect("emitter initialize should succeed");
    assert_eq!(
        init_output.status,
        VmStatus::Success,
        "emitter init failed: {:?}",
        init_output.status
    );
    apply_changes(&mut state, &init_output.state_changes);

    // Call ping — should emit exactly one PingEvent.
    let view = NexusStateView::new(&state);
    let ping_output = vm
        .execute_function(
            &view,
            &FunctionCall {
                sender: deployer,
                contract: deployer,
                function: "emitter::ping".into(),
                type_args: Vec::new(),
                args: Vec::new(),
                gas_limit: 100_000,
            },
        )
        .expect("emitter ping should succeed");
    assert_eq!(
        ping_output.status,
        VmStatus::Success,
        "emitter ping failed: {:?}",
        ping_output.status
    );

    // Verify event was captured.
    assert_eq!(
        ping_output.events.len(),
        1,
        "ping should emit exactly 1 event, got: {}",
        ping_output.events.len()
    );

    let event = &ping_output.events[0];
    // The type_tag should reference the PingEvent struct.
    assert!(
        event.type_tag.contains("PingEvent"),
        "event type_tag should contain 'PingEvent', got: {}",
        event.type_tag
    );
    // Sequence number for first event should be 0.
    assert_eq!(event.sequence_number, 0);
    // Data should be non-empty (BCS-encoded PingEvent { count: 1 }).
    assert!(!event.data.is_empty(), "event data should be non-empty");
    // GUID should be non-empty.
    assert!(!event.guid.is_empty(), "event GUID should be non-empty");

    apply_changes(&mut state, &ping_output.state_changes);

    // Call ping a second time — second event with sequence_number=1.
    let view = NexusStateView::new(&state);
    let ping2_output = vm
        .execute_function(
            &view,
            &FunctionCall {
                sender: deployer,
                contract: deployer,
                function: "emitter::ping".into(),
                type_args: Vec::new(),
                args: Vec::new(),
                gas_limit: 100_000,
            },
        )
        .expect("second ping should succeed");
    assert_eq!(ping2_output.status, VmStatus::Success);
    assert_eq!(
        ping2_output.events.len(),
        1,
        "second ping should emit 1 event"
    );
    assert_eq!(
        ping2_output.events[0].sequence_number, 1,
        "second event should have sequence_number=1"
    );
}

/// Proves that the counter contract (which doesn't use events) produces
/// an empty events vector, confirming no spurious events leak through.
#[test]
fn counter_increment_produces_no_events() {
    let (state, vm, deployer) = deploy_and_init_counter();

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
        .unwrap();
    assert_eq!(inc_output.status, VmStatus::Success);
    assert!(
        inc_output.events.is_empty(),
        "counter increment should not emit events"
    );
}
