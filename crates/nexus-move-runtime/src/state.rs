use nexus_move_types::AccountAddress;

use crate::types::VmResult;

pub const BALANCE_KEY: &[u8] = b"balance";
pub const MODULE_CODE_KEY: &[u8] = b"code";
pub const MODULE_CODE_HASH_KEY: &[u8] = b"code_hash";
pub const MODULE_DEPLOYER_KEY: &[u8] = b"deployer";
pub const MODULE_COUNT_KEY: &[u8] = b"module_count";
pub const MODULE_METADATA_KEY: &[u8] = b"package_metadata";

/// Per-module key prefix.  The full key is `code::{module_name}`.
pub const MODULE_CODE_PREFIX: &[u8] = b"code::";

/// Build a per-module storage key: `code::{module_name}`.
pub fn module_code_key(module_name: &str) -> Vec<u8> {
    let mut key = MODULE_CODE_PREFIX.to_vec();
    key.extend_from_slice(module_name.as_bytes());
    key
}

pub trait StateReader: Send + Sync {
    fn get(&self, account: &AccountAddress, key: &[u8]) -> VmResult<Option<Vec<u8>>>;

    fn contains(&self, account: &AccountAddress, key: &[u8]) -> VmResult<bool> {
        Ok(self.get(account, key)?.is_some())
    }
}

pub struct NexusStateView<'a> {
    state: &'a dyn StateReader,
}

impl std::fmt::Debug for NexusStateView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NexusStateView").finish_non_exhaustive()
    }
}

impl<'a> NexusStateView<'a> {
    pub fn new(state: &'a dyn StateReader) -> Self {
        Self { state }
    }

    pub fn get_raw(&self, account: &AccountAddress, key: &[u8]) -> VmResult<Option<Vec<u8>>> {
        self.state.get(account, key)
    }

    pub fn get_module(&self, address: &AccountAddress) -> VmResult<Option<Vec<u8>>> {
        self.state.get(address, MODULE_CODE_KEY)
    }

    /// Fetch a specific named module from an address.
    ///
    /// Tries the per-module key `code::{module_name}` first, then falls
    /// back to the legacy single-module key `code`.
    pub fn get_module_by_name(
        &self,
        address: &AccountAddress,
        module_name: &str,
    ) -> VmResult<Option<Vec<u8>>> {
        let per_module_key = module_code_key(module_name);
        if let Some(bytes) = self.state.get(address, &per_module_key)? {
            return Ok(Some(bytes));
        }
        // Fallback: legacy single-module key.
        self.state.get(address, MODULE_CODE_KEY)
    }

    pub fn has_module(&self, address: &AccountAddress) -> VmResult<bool> {
        self.state.contains(address, MODULE_CODE_KEY)
    }

    pub fn get_balance(&self, account: &AccountAddress) -> VmResult<u64> {
        let raw = self.state.get(account, BALANCE_KEY)?;
        Ok(parse_balance_bytes(raw.as_deref()))
    }

    pub fn get_resource(
        &self,
        account: &AccountAddress,
        resource_key: &[u8],
    ) -> VmResult<Option<Vec<u8>>> {
        self.state.get(account, resource_key)
    }
}

pub fn parse_balance_bytes(raw: Option<&[u8]>) -> u64 {
    raw.and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes)
        .unwrap_or(0)
}
