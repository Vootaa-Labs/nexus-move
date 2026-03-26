// Copyright (c) The Nexus-Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Stable re-exports of upstream Move VM types for `nexus-node` consumption.
//!
//! **Contract**: `nexus-node` crates MUST acquire all move-* types through
//! this module (or other `nexus-move-*` facade crates) rather than depending
//! on vendor crates directly.  See FORMAL-03 §7 for the mapping strategy.
//!
//! The sub-module hierarchy mirrors the upstream crate paths so that
//! migration is mechanical:
//!
//! ```text
//! -use move_core_types::account_address::AccountAddress;
//! +use nexus_move_runtime::upstream::move_core_types::account_address::AccountAddress;
//! ```

// ─── move-core-types ─────────────────────────────────────────────────────

pub mod move_core_types {
    pub mod account_address {
        pub use move_core_types::account_address::AccountAddress;
    }
    pub mod effects {
        pub use move_core_types::effects::{ChangeSet, Op};
    }
    pub mod gas_algebra {
        pub use move_core_types::gas_algebra::InternalGas;
    }
    pub mod identifier {
        pub use move_core_types::identifier::{IdentStr, Identifier};
    }
    pub mod language_storage {
        pub use move_core_types::language_storage::{ModuleId, StructTag, TypeTag};
    }
    pub mod metadata {
        pub use move_core_types::metadata::Metadata;
    }
    pub mod value {
        pub use move_core_types::value::MoveTypeLayout;
    }
    pub mod vm_status {
        pub use move_core_types::vm_status::StatusCode;
    }
}

// ─── move-binary-format ──────────────────────────────────────────────────

pub mod move_binary_format {
    pub use move_binary_format::CompiledModule;

    pub mod access {
        pub use move_binary_format::access::ModuleAccess;
    }
    pub mod deserializer {
        pub use move_binary_format::deserializer::DeserializerConfig;
    }
    pub mod errors {
        pub use move_binary_format::errors::{Location, PartialVMError, PartialVMResult, VMResult};
    }
    pub mod file_format_common {
        pub use move_binary_format::file_format_common::{IDENTIFIER_SIZE_MAX, VERSION_MAX};
    }
}

// ─── move-vm-runtime ─────────────────────────────────────────────────────

pub mod move_vm_runtime {
    pub use move_vm_runtime::{
        AsUnsyncModuleStorage, ModuleStorage, RuntimeEnvironment, WithRuntimeEnvironment,
    };

    pub mod data_cache {
        pub use move_vm_runtime::data_cache::TransactionDataCache;
    }
    pub mod module_traversal {
        pub use move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage};
    }
    pub mod move_vm {
        pub use move_vm_runtime::move_vm::MoveVM;
    }
    pub mod native_extensions {
        pub use move_vm_runtime::native_extensions::NativeContextExtensions;
    }
    pub mod native_functions {
        pub use move_vm_runtime::native_functions::NativeFunction;
    }
}

// ─── move-vm-types ───────────────────────────────────────────────────────

pub mod move_vm_types {
    pub mod code {
        pub use move_vm_types::code::ModuleBytesStorage;
    }
    pub mod gas {
        pub use move_vm_types::gas::UnmeteredGasMeter;
    }
    pub mod loaded_data {
        pub mod runtime_types {
            pub use move_vm_types::loaded_data::runtime_types::Type;
        }
    }
    pub mod natives {
        pub mod function {
            pub use move_vm_types::natives::function::NativeResult;
        }
    }
    pub mod resolver {
        pub use move_vm_types::resolver::ResourceResolver;
    }
    pub mod values {
        pub use move_vm_types::values::Value;
        pub mod values_impl {
            pub use move_vm_types::values::values_impl::SignerRef;
        }
    }
    pub use move_vm_types::pop_arg;
}
