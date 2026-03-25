#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdlibModule {
    pub name: &'static str,
    pub relative_path: &'static str,
}

const BOOTSTRAP_MODULES: &[StdlibModule] = &[
    StdlibModule {
        name: "ascii",
        relative_path: "framework/ascii.move",
    },
    StdlibModule {
        name: "bcs",
        relative_path: "framework/bcs.move",
    },
    StdlibModule {
        name: "bit_vector",
        relative_path: "framework/bit_vector.move",
    },
    StdlibModule {
        name: "error",
        relative_path: "framework/error.move",
    },
    StdlibModule {
        name: "event",
        relative_path: "framework/event.move",
    },
    StdlibModule {
        name: "fixed_point32",
        relative_path: "framework/fixed_point32.move",
    },
    StdlibModule {
        name: "guid",
        relative_path: "framework/guid.move",
    },
    StdlibModule {
        name: "hash",
        relative_path: "framework/hash.move",
    },
    StdlibModule {
        name: "option",
        relative_path: "framework/option.move",
    },
    StdlibModule {
        name: "signer",
        relative_path: "framework/signer.move",
    },
    StdlibModule {
        name: "string",
        relative_path: "framework/string.move",
    },
    StdlibModule {
        name: "type_name",
        relative_path: "framework/type_name.move",
    },
    StdlibModule {
        name: "vector",
        relative_path: "framework/vector.move",
    },
];

/// Native functions provided by the stdlib host.
///
/// Note: vector operations (length, borrow, push_back, pop_back, destroy_empty,
/// swap) are VM bytecodes — they are **not** native functions and should not
/// appear in this list.
const BOOTSTRAP_NATIVES: &[&str] = &[
    "0x1::signer::borrow_address",
    "0x1::bcs::to_bytes",
    "0x1::hash::sha2_256",
    "0x1::hash::sha3_256",
    "0x1::type_name::get",
    "0x1::string::internal_check_utf8",
    "0x1::string::internal_sub_string",
    "0x1::string::internal_index_of",
    "0x1::string::internal_is_char_boundary",
    "0x1::event::write_to_event_store",
];

/// Compiled framework module bytecodes (aptos-node-v1.30.4).
const ASCII_MV: &[u8] = include_bytes!("framework/ascii.mv");
const BCS_MV: &[u8] = include_bytes!("framework/bcs.mv");
const BIT_VECTOR_MV: &[u8] = include_bytes!("framework/bit_vector.mv");
const ERROR_MV: &[u8] = include_bytes!("framework/error.mv");
const EVENT_MV: &[u8] = include_bytes!("framework/event.mv");
const FIXED_POINT32_MV: &[u8] = include_bytes!("framework/fixed_point32.mv");
const GUID_MV: &[u8] = include_bytes!("framework/guid.mv");
const HASH_MV: &[u8] = include_bytes!("framework/hash.mv");
const OPTION_MV: &[u8] = include_bytes!("framework/option.mv");
const SIGNER_MV: &[u8] = include_bytes!("framework/signer.mv");
const STRING_MV: &[u8] = include_bytes!("framework/string.mv");
const TYPE_NAME_MV: &[u8] = include_bytes!("framework/type_name.mv");
const VECTOR_MV: &[u8] = include_bytes!("framework/vector.mv");

/// All embedded framework modules as (name, bytecode) pairs.
pub const FRAMEWORK_MODULES: &[(&str, &[u8])] = &[
    ("ascii", ASCII_MV),
    ("bcs", BCS_MV),
    ("bit_vector", BIT_VECTOR_MV),
    ("error", ERROR_MV),
    ("event", EVENT_MV),
    ("fixed_point32", FIXED_POINT32_MV),
    ("guid", GUID_MV),
    ("hash", HASH_MV),
    ("option", OPTION_MV),
    ("signer", SIGNER_MV),
    ("string", STRING_MV),
    ("type_name", TYPE_NAME_MV),
    ("vector", VECTOR_MV),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdlibSnapshot {
    pub source: &'static str,
    pub includes_precompiled_modules: bool,
    pub native_functions: &'static [&'static str],
    pub modules: &'static [StdlibModule],
}

impl StdlibSnapshot {
    pub const fn bootstrap() -> Self {
        Self {
            source: "stdlib/",
            includes_precompiled_modules: true,
            native_functions: BOOTSTRAP_NATIVES,
            modules: BOOTSTRAP_MODULES,
        }
    }
}

pub fn bootstrap_modules() -> &'static [StdlibModule] {
    BOOTSTRAP_MODULES
}

/// The framework address `0x0000…0001`.
pub fn framework_address_bytes() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[31] = 1;
    bytes
}

/// Look up an embedded framework module by name.
///
/// Returns the compiled `.mv` bytecode if the module exists in the
/// bootstrap set, or `None` otherwise.
pub fn get_framework_module(module_name: &str) -> Option<&'static [u8]> {
    FRAMEWORK_MODULES
        .iter()
        .find(|(name, _)| *name == module_name)
        .map(|(_, bytes)| *bytes)
}

/// Returns the number of embedded framework modules.
pub fn framework_module_count() -> usize {
    FRAMEWORK_MODULES.len()
}

// ── VM-level native functions (feature-gated) ───────────────────────────

#[cfg(feature = "vm-backend")]
pub mod natives {
    use move_core_types::account_address::AccountAddress as MoveAddress;
    use move_core_types::gas_algebra::InternalGas;
    use move_core_types::identifier::Identifier;
    use move_vm_runtime::native_functions::NativeFunction;
    use move_vm_types::loaded_data::runtime_types::Type;
    use move_vm_types::natives::function::NativeResult;
    use move_vm_types::pop_arg;
    use move_vm_types::value_serde::ValueSerDeContext;
    use move_vm_types::values::values_impl::{Reference, SignerRef, Struct, VectorRef};
    use move_vm_types::values::Value;
    use smallvec::smallvec;
    use std::collections::VecDeque;
    use std::sync::Arc;

    use super::events::{ContractEvent, NexusEventStore};

    /// Zero-cost gas constant used for all natives in the devnet.
    const ZERO: InternalGas = InternalGas::zero();

    /// Build the native function table required by the Move runtime.
    ///
    /// Registers all native functions declared in [`BOOTSTRAP_NATIVES`](super::BOOTSTRAP_NATIVES),
    /// plus the event `write_to_event_store` native.
    pub fn native_functions() -> Vec<(MoveAddress, Identifier, Identifier, NativeFunction)> {
        let addr = {
            let bytes = super::framework_address_bytes();
            MoveAddress::new(bytes)
        };

        let id = |s: &str| Identifier::new(s).expect("valid identifier");

        vec![
            // signer
            (
                addr,
                id("signer"),
                id("borrow_address"),
                make_native_borrow_address(),
            ),
            // bcs
            (addr, id("bcs"), id("to_bytes"), make_native_to_bytes()),
            // hash
            (addr, id("hash"), id("sha2_256"), make_native_sha2_256()),
            (addr, id("hash"), id("sha3_256"), make_native_sha3_256()),
            // type_name
            (
                addr,
                id("type_name"),
                id("get"),
                make_native_type_name_get(),
            ),
            // string
            (
                addr,
                id("string"),
                id("internal_check_utf8"),
                make_native_check_utf8(),
            ),
            (
                addr,
                id("string"),
                id("internal_sub_string"),
                make_native_sub_string(),
            ),
            (
                addr,
                id("string"),
                id("internal_index_of"),
                make_native_index_of(),
            ),
            (
                addr,
                id("string"),
                id("internal_is_char_boundary"),
                make_native_is_char_boundary(),
            ),
            // event
            (
                addr,
                id("event"),
                id("write_to_event_store"),
                make_native_write_to_event_store(),
            ),
        ]
    }

    // ── signer::borrow_address ──────────────────────────────────────────

    fn make_native_borrow_address() -> NativeFunction {
        Arc::new(
            |_context, _ty_args: Vec<Type>, mut arguments: VecDeque<Value>| {
                let signer_ref = pop_arg!(arguments, SignerRef);
                NativeResult::map_partial_vm_result_one(ZERO, signer_ref.borrow_signer())
            },
        )
    }

    // ── bcs::to_bytes ───────────────────────────────────────────────────

    fn make_native_to_bytes() -> NativeFunction {
        use move_core_types::vm_status::sub_status::NFE_BCS_SERIALIZATION_FAILURE;

        Arc::new(
            |context, mut ty_args: Vec<Type>, mut args: VecDeque<Value>| {
                let ref_to_val = pop_arg!(args, Reference);
                let arg_type = ty_args.pop().unwrap();

                let layout = match context.type_to_type_layout(&arg_type) {
                    Ok(layout) => layout,
                    Err(_) => {
                        return Ok(NativeResult::err(ZERO, NFE_BCS_SERIALIZATION_FAILURE));
                    }
                };

                let val = ref_to_val.read_ref()?;
                let function_value_extension = context.function_value_extension();
                let serialized_value = match ValueSerDeContext::new()
                    .with_legacy_signer()
                    .with_func_args_deserialization(&function_value_extension)
                    .serialize(&val, &layout)?
                {
                    Some(bytes) => bytes,
                    None => {
                        return Ok(NativeResult::err(ZERO, NFE_BCS_SERIALIZATION_FAILURE));
                    }
                };

                Ok(NativeResult::ok(
                    ZERO,
                    smallvec![Value::vector_u8(serialized_value)],
                ))
            },
        )
    }

    // ── hash::sha2_256 ─────────────────────────────────────────────────

    fn make_native_sha2_256() -> NativeFunction {
        use sha2::Digest;
        Arc::new(
            |_context, _ty_args: Vec<Type>, mut arguments: VecDeque<Value>| {
                let hash_arg = pop_arg!(arguments, Vec<u8>);
                let hash_vec = sha2::Sha256::digest(hash_arg.as_slice()).to_vec();
                Ok(NativeResult::ok(
                    ZERO,
                    smallvec![Value::vector_u8(hash_vec)],
                ))
            },
        )
    }

    // ── hash::sha3_256 ─────────────────────────────────────────────────

    fn make_native_sha3_256() -> NativeFunction {
        use sha3::Digest;
        Arc::new(
            |_context, _ty_args: Vec<Type>, mut arguments: VecDeque<Value>| {
                let hash_arg = pop_arg!(arguments, Vec<u8>);
                let hash_vec = sha3::Sha3_256::digest(hash_arg.as_slice()).to_vec();
                Ok(NativeResult::ok(
                    ZERO,
                    smallvec![Value::vector_u8(hash_vec)],
                ))
            },
        )
    }

    // ── type_name::get ──────────────────────────────────────────────────

    fn make_native_type_name_get() -> NativeFunction {
        Arc::new(|context, ty_args: Vec<Type>, _arguments: VecDeque<Value>| {
            let type_tag = context.type_to_type_tag(&ty_args[0])?;
            let type_name = type_tag.to_canonical_string();

            // Build std::string::String { bytes: vector<u8> }
            let string_val = Value::struct_(Struct::pack(vec![Value::vector_u8(
                type_name.as_bytes().to_vec(),
            )]));
            // Build std::type_name::TypeName { name: String }
            let type_name_val = Value::struct_(Struct::pack(vec![string_val]));

            Ok(NativeResult::ok(ZERO, smallvec![type_name_val]))
        })
    }

    // ── string::internal_check_utf8 ─────────────────────────────────────

    fn make_native_check_utf8() -> NativeFunction {
        Arc::new(|_context, _ty_args: Vec<Type>, mut args: VecDeque<Value>| {
            let s_arg = pop_arg!(args, VectorRef);
            let s_ref = s_arg.as_bytes_ref();
            let ok = std::str::from_utf8(s_ref.as_slice()).is_ok();
            NativeResult::map_partial_vm_result_one(ZERO, Ok(Value::bool(ok)))
        })
    }

    // ── string::internal_is_char_boundary ───────────────────────────────

    fn make_native_is_char_boundary() -> NativeFunction {
        Arc::new(|_context, _ty_args: Vec<Type>, mut args: VecDeque<Value>| {
            let i = pop_arg!(args, u64);
            let s_arg = pop_arg!(args, VectorRef);
            let s_ref = s_arg.as_bytes_ref();
            let ok = std::str::from_utf8(s_ref.as_slice())
                .map(|s| s.is_char_boundary(i as usize))
                .unwrap_or(false);
            NativeResult::map_partial_vm_result_one(ZERO, Ok(Value::bool(ok)))
        })
    }

    // ── string::internal_sub_string ─────────────────────────────────────

    fn make_native_sub_string() -> NativeFunction {
        Arc::new(|_context, _ty_args: Vec<Type>, mut args: VecDeque<Value>| {
            let j = pop_arg!(args, u64) as usize;
            let i = pop_arg!(args, u64) as usize;

            if j < i {
                return Ok(NativeResult::err(ZERO, 1));
            }

            let s_arg = pop_arg!(args, VectorRef);
            let s_ref = s_arg.as_bytes_ref();
            let bytes = s_ref.as_slice();
            if j > bytes.len() {
                return Ok(NativeResult::err(ZERO, 1));
            }
            let v = Value::vector_u8(bytes[i..j].iter().cloned());
            NativeResult::map_partial_vm_result_one(ZERO, Ok(v))
        })
    }

    // ── string::internal_index_of ───────────────────────────────────────

    fn make_native_index_of() -> NativeFunction {
        Arc::new(|_context, _ty_args: Vec<Type>, mut args: VecDeque<Value>| {
            let r_arg = pop_arg!(args, VectorRef);
            let r_ref = r_arg.as_bytes_ref();
            let r_bytes = r_ref.as_slice();
            let s_arg = pop_arg!(args, VectorRef);
            let s_ref = s_arg.as_bytes_ref();
            let s_bytes = s_ref.as_slice();
            // Byte-level search (equivalent to str::find for valid UTF-8)
            let pos = if r_bytes.is_empty() {
                0
            } else {
                s_bytes
                    .windows(r_bytes.len())
                    .position(|w| w == r_bytes)
                    .unwrap_or(s_bytes.len())
            };
            NativeResult::map_partial_vm_result_one(ZERO, Ok(Value::u64(pos as u64)))
        })
    }

    // ── event::write_to_event_store ─────────────────────────────────────

    /// Captures an event emitted by Move code into the `NexusEventStore`
    /// extension registered in the `NativeContextExtensions`.
    ///
    /// Signature: `native fun write_to_event_store<T: drop + store>(guid: vector<u8>, count: u64, msg: T)`
    fn make_native_write_to_event_store() -> NativeFunction {
        Arc::new(
            |context, ty_args: Vec<Type>, mut arguments: VecDeque<Value>| {
                debug_assert!(ty_args.len() == 1);
                debug_assert!(arguments.len() == 3);

                // Pop args in reverse order: msg (T), count (u64), guid (vector<u8>).
                let msg = arguments.pop_back().unwrap();
                let count = pop_arg!(arguments, u64);
                let guid = pop_arg!(arguments, Vec<u8>);

                // Resolve the type tag for the event payload.
                let type_tag = context.type_to_type_tag(&ty_args[0])?;

                // Serialize the event payload.
                let layout = context.type_to_type_layout(&ty_args[0])?;
                let function_value_extension = context.function_value_extension();
                let data = ValueSerDeContext::new()
                    .with_legacy_signer()
                    .with_func_args_deserialization(&function_value_extension)
                    .serialize(&msg, &layout)?
                    .unwrap_or_default();

                // Store the event in the NexusEventStore extension.
                let store = context.extensions_mut().get_mut::<NexusEventStore>();
                store.push(ContractEvent {
                    type_tag: type_tag.to_canonical_string(),
                    guid,
                    sequence_number: count,
                    data,
                });

                Ok(NativeResult::ok(ZERO, smallvec![]))
            },
        )
    }
}

// ── Event capture types (feature-gated) ─────────────────────────────────

#[cfg(feature = "vm-backend")]
pub mod events {
    use better_any::{Tid, TidAble};

    /// A contract event captured during Move execution.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ContractEvent {
        /// Event type tag (e.g. `"0xCAFE::counter::IncrementEvent"`).
        pub type_tag: String,
        /// GUID identifying the event stream.
        pub guid: Vec<u8>,
        /// Sequence number within the event stream (monotonic).
        pub sequence_number: u64,
        /// BCS-encoded event payload.
        pub data: Vec<u8>,
    }

    /// Accumulator for events produced during a single Move transaction.
    ///
    /// Registered as a `NativeContextExtension` so that native functions
    /// (e.g. `write_to_event_store`) can push events during execution.
    #[derive(Default, Tid)]
    pub struct NexusEventStore {
        events: Vec<ContractEvent>,
    }

    impl NexusEventStore {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn push(&mut self, event: ContractEvent) {
            self.events.push(event);
        }

        pub fn drain(self) -> Vec<ContractEvent> {
            self.events
        }

        pub fn is_empty(&self) -> bool {
            self.events.is_empty()
        }

        pub fn len(&self) -> usize {
            self.events.len()
        }
    }
}

#[cfg(feature = "vm-backend")]
pub use events::{ContractEvent, NexusEventStore};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_snapshot_exposes_all_modules() {
        let snapshot = StdlibSnapshot::bootstrap();
        assert_eq!(snapshot.modules.len(), 13);
        let names: Vec<&str> = snapshot.modules.iter().map(|m| m.name).collect();
        assert!(names.contains(&"signer"));
        assert!(names.contains(&"vector"));
        assert!(names.contains(&"option"));
        assert!(names.contains(&"error"));
    }

    #[test]
    fn all_framework_modules_have_valid_magic() {
        for (name, bytes) in FRAMEWORK_MODULES {
            assert!(!bytes.is_empty(), "module '{name}' is empty");
            assert_eq!(
                &bytes[..4],
                &[0xa1, 0x1c, 0xeb, 0x0b],
                "module '{name}' has invalid magic bytes"
            );
        }
    }

    #[test]
    fn framework_module_count_matches() {
        assert_eq!(framework_module_count(), 13);
        assert_eq!(FRAMEWORK_MODULES.len(), BOOTSTRAP_MODULES.len());
    }

    #[test]
    fn nonexistent_module_returns_none() {
        assert!(get_framework_module("nonexistent").is_none());
    }

    #[test]
    fn framework_address_is_0x1() {
        let addr = framework_address_bytes();
        assert_eq!(addr[31], 1);
        assert!(addr[..31].iter().all(|&b| b == 0));
    }

    #[test]
    fn get_framework_module_returns_each_module() {
        for (name, expected) in FRAMEWORK_MODULES {
            let actual =
                get_framework_module(name).unwrap_or_else(|| panic!("module '{name}' not found"));
            assert_eq!(actual.len(), expected.len(), "size mismatch for '{name}'");
        }
    }
}

#[cfg(all(test, feature = "vm-backend"))]
mod native_tests {
    use crate::natives;

    #[test]
    fn native_functions_registered_count_matches_bootstrap() {
        let table = natives::native_functions();
        assert_eq!(
            table.len(),
            super::BOOTSTRAP_NATIVES.len(),
            "native_functions() count must match BOOTSTRAP_NATIVES"
        );
    }

    #[test]
    fn native_functions_cover_all_bootstrap_entries() {
        let table = natives::native_functions();
        for entry in super::BOOTSTRAP_NATIVES {
            let parts: Vec<&str> = entry.split("::").collect();
            let (module, function) = (parts[1], parts[2]);
            let found = table
                .iter()
                .any(|(_, m, f, _)| m.as_str() == module && f.as_str() == function);
            assert!(
                found,
                "BOOTSTRAP_NATIVES entry {entry} not found in native_functions()"
            );
        }
    }

    #[test]
    fn all_natives_are_at_framework_address() {
        let addr = {
            let bytes = super::framework_address_bytes();
            move_core_types::account_address::AccountAddress::new(bytes)
        };
        for (a, m, f, _) in natives::native_functions() {
            assert_eq!(a, addr, "native {}::{} has wrong address", m, f);
        }
    }
}
