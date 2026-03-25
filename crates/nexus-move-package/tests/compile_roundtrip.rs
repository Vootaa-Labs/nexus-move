//! Gap #1: Compile→Publish→Execute roundtrip test.
//!
//! Proves that `build_package()` output can be published and executed
//! through `RealMoveVm`, validating the full toolchain end-to-end.

#![cfg(all(feature = "native-compile"))]

use std::collections::BTreeMap;
use std::path::PathBuf;

use nexus_move_runtime::config::VmConfig;
use nexus_move_runtime::state::{NexusStateView, StateReader};
use nexus_move_runtime::types::{FunctionCall, ModulePublish, QueryRequest, VmResult, VmStatus};
use nexus_move_runtime::vm_backend::RealMoveVm;
use nexus_move_runtime::MoveVm;
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
}

impl StateReader for MemState {
    fn get(&self, account: &AccountAddress, key: &[u8]) -> VmResult<Option<Vec<u8>>> {
        Ok(self.data.get(&(*account, key.to_vec())).cloned())
    }
}

fn cafe_address() -> AccountAddress {
    let mut bytes = [0u8; 32];
    bytes[30] = 0xCA;
    bytes[31] = 0xFE;
    AccountAddress(bytes)
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

/// Full toolchain test: `build_package()` → `publish_modules()` →
/// `execute_function()` → `query_view()`.
#[test]
fn compile_publish_execute_roundtrip() {
    let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter");
    if !example_dir.join("Move.toml").exists() {
        eprintln!("skipping: examples/counter not found");
        return;
    }

    // Step 1: Compile counter from source.
    let build_result = nexus_move_package::build::build_package(&example_dir, &[], None)
        .expect("build_package should succeed");
    assert_eq!(build_result.module_count, 1);
    assert!(build_result.total_bytes > 0);

    let compiled_bytes: Vec<Vec<u8>> = build_result
        .compiled
        .modules
        .iter()
        .map(|m| m.bytes.clone())
        .collect();
    assert_eq!(compiled_bytes.len(), 1);

    // Verify bytecode magic
    assert_eq!(
        &compiled_bytes[0][..4],
        &[0xa1, 0x1c, 0xeb, 0x0b],
        "compiled bytecode should have Move magic"
    );

    // Step 2: Publish through RealMoveVm.
    let deployer = cafe_address();
    let mut state = MemState::new();
    let vm = RealMoveVm::new(&VmConfig::default());

    let view = NexusStateView::new(&state);
    let pub_output = vm
        .publish_modules(
            &view,
            &ModulePublish {
                sender: deployer,
                modules: compiled_bytes,
                gas_limit: 100_000,
                upgrade_policy: None,
            },
        )
        .expect("publish should succeed");
    assert_eq!(pub_output.status, VmStatus::Success);
    assert!(pub_output.gas_used > 0);
    apply_changes(&mut state, &pub_output.state_changes);

    // Step 3: Initialize counter.
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
    assert_eq!(init_output.status, VmStatus::Success);
    apply_changes(&mut state, &init_output.state_changes);

    // Step 4: Increment twice.
    for _ in 0..2 {
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
        assert_eq!(inc_output.status, VmStatus::Success);
        apply_changes(&mut state, &inc_output.state_changes);
    }

    // Step 5: Query get_count → expect 2.
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
        .expect("query should succeed");
    let count = u64::from_le_bytes(
        query
            .return_value
            .expect("should return value")
            .as_slice()
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        count, 2,
        "compile→publish→execute roundtrip: counter should be 2"
    );
}

/// Proves that build_package output matches the pre-compiled fixture
/// used in other tests (bytecodes are identical).
#[test]
fn build_package_output_matches_fixture() {
    let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter");
    if !example_dir.join("Move.toml").exists() {
        eprintln!("skipping: examples/counter not found");
        return;
    }

    let build_result = nexus_move_package::build::build_package(&example_dir, &[], None)
        .expect("build_package should succeed");

    let compiled_bytes = &build_result.compiled.modules[0].bytes;

    // The pre-compiled counter.mv fixture used by other tests
    let fixture_bytes =
        include_bytes!("../../../examples/counter/nexus-artifact/bytecode/counter.mv");

    assert_eq!(
        compiled_bytes, fixture_bytes,
        "build_package output should match the pre-compiled fixture"
    );
}
