use std::collections::BTreeMap;

use nexus_move_types::AccountAddress;

use crate::state::NexusStateView;
use crate::types::{StateChange, VmResult};

pub type WriteSet = BTreeMap<(AccountAddress, Vec<u8>), Option<Vec<u8>>>;

pub fn resource_key(type_tag: &str) -> Vec<u8> {
    let mut key = b"resource::".to_vec();
    key.extend_from_slice(type_tag.as_bytes());
    key
}

#[derive(Debug)]
pub struct ResourceStore<'a> {
    view: &'a NexusStateView<'a>,
    overlay: WriteSet,
}

impl<'a> ResourceStore<'a> {
    pub fn new(view: &'a NexusStateView<'a>) -> Self {
        Self {
            view,
            overlay: BTreeMap::new(),
        }
    }

    pub fn get(&self, account: &AccountAddress, type_tag: &str) -> VmResult<Option<Vec<u8>>> {
        let key = resource_key(type_tag);
        if let Some(entry) = self.overlay.get(&(*account, key.clone())) {
            return Ok(entry.clone());
        }
        self.view.get_resource(account, &key)
    }

    pub fn set(&mut self, account: AccountAddress, type_tag: &str, value: Vec<u8>) {
        let key = resource_key(type_tag);
        self.overlay.insert((account, key), Some(value));
    }

    pub fn remove(&mut self, account: AccountAddress, type_tag: &str) {
        let key = resource_key(type_tag);
        self.overlay.insert((account, key), None);
    }

    pub fn into_changes(self) -> (WriteSet, Vec<StateChange>) {
        let mut state_changes = Vec::with_capacity(self.overlay.len());
        for ((account, key), value) in &self.overlay {
            state_changes.push(StateChange {
                account: *account,
                key: key.clone(),
                value: value.clone(),
            });
        }
        (self.overlay, state_changes)
    }
}
