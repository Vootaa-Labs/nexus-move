#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmConfig {
    pub max_binary_size: usize,
    pub call_base_gas: u64,
    pub publish_base_gas: u64,
    pub publish_per_byte_gas: u64,
    pub read_per_byte_gas: u64,
    pub write_per_byte_gas: u64,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_binary_size: 524_288,
            call_base_gas: 5_000,
            publish_base_gas: 10_000,
            publish_per_byte_gas: 1,
            read_per_byte_gas: 1,
            write_per_byte_gas: 5,
        }
    }
}

impl VmConfig {
    pub const fn for_testing() -> Self {
        Self {
            max_binary_size: 65_536,
            call_base_gas: 1_000,
            publish_base_gas: 2_000,
            publish_per_byte_gas: 1,
            read_per_byte_gas: 1,
            write_per_byte_gas: 5,
        }
    }
}
