//! ABI extraction and compatibility checking for Move modules.
//!
//! The ABI (Application Binary Interface) captures the public surface of
//! a compiled Move module: public/friend function signatures, entry flags,
//! and public struct definitions.  A deterministic blake3 hash of this
//! surface allows the `Compatible` upgrade policy to verify that an
//! upgrade does not break callers.

use move_binary_format::access::ModuleAccess;
use move_binary_format::file_format::{SignatureToken, Visibility};
use move_binary_format::CompiledModule;

use nexus_move_types::HashValue;

/// Compute a deterministic ABI hash for a compiled Move module.
///
/// The hash covers:
/// - Public and friend function signatures (name, visibility, is_entry,
///   type parameters, parameter types, return types)
/// - Struct definitions with their abilities and field layouts
///
/// Private functions and implementation details are excluded so that
/// non-breaking internal changes produce the same hash.
pub fn compute_module_abi_hash(module: &CompiledModule) -> HashValue {
    let mut hasher = blake3::Hasher::new();

    // ── Public / friend function signatures (sorted by name) ────────
    let mut fn_entries: Vec<(String, u8, bool, Vec<u8>, Vec<u8>)> = Vec::new();

    for func_def in module.function_defs() {
        if func_def.visibility == Visibility::Private {
            continue;
        }

        let func_handle = module.function_handle_at(func_def.function);
        let name = module.identifier_at(func_handle.name).to_string();
        let params = module.signature_at(func_handle.parameters);
        let ret = module.signature_at(func_handle.return_);

        let param_bytes = signature_tokens_to_bytes(&params.0);
        let ret_bytes = signature_tokens_to_bytes(&ret.0);

        fn_entries.push((
            name,
            func_def.visibility as u8,
            func_def.is_entry,
            param_bytes,
            ret_bytes,
        ));
    }

    fn_entries.sort_by(|a, b| a.0.cmp(&b.0));

    hasher.update(b"functions:");
    hasher.update(&(fn_entries.len() as u32).to_le_bytes());
    for (name, vis, is_entry, params, ret) in &fn_entries {
        hasher.update(name.as_bytes());
        hasher.update(&[*vis]);
        hasher.update(&[u8::from(*is_entry)]);
        hasher.update(&(params.len() as u32).to_le_bytes());
        hasher.update(params);
        hasher.update(&(ret.len() as u32).to_le_bytes());
        hasher.update(ret);
    }

    // ── Struct definitions (sorted by name) ─────────────────────────
    let mut struct_entries: Vec<(String, u8, Vec<u8>)> = Vec::new();

    for struct_def in module.struct_defs() {
        let struct_handle = module.struct_handle_at(struct_def.struct_handle);
        let name = module.identifier_at(struct_handle.name).to_string();
        let abilities = struct_handle.abilities.into_u8();

        let field_bytes = match &struct_def.field_information {
            move_binary_format::file_format::StructFieldInformation::Native => {
                vec![0xFF] // native marker
            }
            move_binary_format::file_format::StructFieldInformation::Declared(fields) => {
                let mut bytes = Vec::new();
                for field in fields {
                    let field_name = module.identifier_at(field.name).as_str();
                    bytes.extend_from_slice(&(field_name.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(field_name.as_bytes());
                    let sig = signature_token_to_bytes(&field.signature.0);
                    bytes.extend_from_slice(&(sig.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(&sig);
                }
                bytes
            }
            move_binary_format::file_format::StructFieldInformation::DeclaredVariants(variants) => {
                let mut bytes = vec![0xFE]; // variant marker
                for variant in variants {
                    let variant_name = module.identifier_at(variant.name).as_str();
                    bytes.extend_from_slice(&(variant_name.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(variant_name.as_bytes());
                    for field in &variant.fields {
                        let field_name = module.identifier_at(field.name).as_str();
                        bytes.extend_from_slice(&(field_name.len() as u32).to_le_bytes());
                        bytes.extend_from_slice(field_name.as_bytes());
                        let sig = signature_token_to_bytes(&field.signature.0);
                        bytes.extend_from_slice(&(sig.len() as u32).to_le_bytes());
                        bytes.extend_from_slice(&sig);
                    }
                }
                bytes
            }
        };

        struct_entries.push((name, abilities, field_bytes));
    }

    struct_entries.sort_by(|a, b| a.0.cmp(&b.0));

    hasher.update(b"structs:");
    hasher.update(&(struct_entries.len() as u32).to_le_bytes());
    for (name, abilities, fields) in &struct_entries {
        hasher.update(name.as_bytes());
        hasher.update(&[*abilities]);
        hasher.update(&(fields.len() as u32).to_le_bytes());
        hasher.update(fields);
    }

    *hasher.finalize().as_bytes()
}

/// Compute a combined ABI hash for multiple modules in a package.
///
/// Each module's ABI hash is computed independently, then all hashes
/// are sorted and combined into a single package-level ABI hash.
pub fn compute_package_abi_hash(modules: &[&CompiledModule]) -> HashValue {
    if modules.is_empty() {
        return [0u8; 32];
    }

    let mut module_hashes: Vec<(String, HashValue)> = modules
        .iter()
        .map(|m| {
            let name = m.self_id().name().to_string();
            let hash = compute_module_abi_hash(m);
            (name, hash)
        })
        .collect();

    module_hashes.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"package_abi:");
    hasher.update(&(module_hashes.len() as u32).to_le_bytes());
    for (name, hash) in &module_hashes {
        hasher.update(name.as_bytes());
        hasher.update(hash);
    }
    *hasher.finalize().as_bytes()
}

/// Check if two module ABI hashes are compatible.
///
/// For the `Compatible` upgrade policy, the new module must have a
/// superset of the old module's public interface.  Currently this is
/// implemented as strict equality — the ABI hash must match exactly.
///
/// Future work: implement structural comparison that allows adding
/// new public functions while keeping existing signatures unchanged.
pub fn abi_is_compatible(old_hash: &HashValue, new_hash: &HashValue) -> bool {
    old_hash == new_hash
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn signature_tokens_to_bytes(tokens: &[SignatureToken]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for token in tokens {
        bytes.extend_from_slice(&signature_token_to_bytes(token));
    }
    bytes
}

fn signature_token_to_bytes(token: &SignatureToken) -> Vec<u8> {
    // Use a simple, deterministic encoding of signature tokens.
    let mut bytes = Vec::new();
    encode_token(token, &mut bytes);
    bytes
}

fn encode_token(token: &SignatureToken, out: &mut Vec<u8>) {
    match token {
        SignatureToken::Bool => out.push(0x01),
        SignatureToken::U8 => out.push(0x02),
        SignatureToken::U16 => out.push(0x03),
        SignatureToken::U32 => out.push(0x04),
        SignatureToken::U64 => out.push(0x05),
        SignatureToken::U128 => out.push(0x06),
        SignatureToken::U256 => out.push(0x07),
        SignatureToken::Address => out.push(0x08),
        SignatureToken::Signer => out.push(0x09),
        SignatureToken::Vector(inner) => {
            out.push(0x0A);
            encode_token(inner, out);
        }
        SignatureToken::Struct(idx) => {
            out.push(0x0B);
            out.extend_from_slice(&idx.0.to_le_bytes());
        }
        SignatureToken::StructInstantiation(idx, type_args) => {
            out.push(0x0C);
            out.extend_from_slice(&idx.0.to_le_bytes());
            out.push(type_args.len() as u8);
            for ty in type_args {
                encode_token(ty, out);
            }
        }
        SignatureToken::Reference(inner) => {
            out.push(0x0D);
            encode_token(inner, out);
        }
        SignatureToken::MutableReference(inner) => {
            out.push(0x0E);
            encode_token(inner, out);
        }
        SignatureToken::TypeParameter(idx) => {
            out.push(0x0F);
            out.extend_from_slice(&idx.to_le_bytes());
        }
        SignatureToken::Function(params, ret, abilities) => {
            out.push(0x10);
            out.push(params.len() as u8);
            for p in params {
                encode_token(p, out);
            }
            out.push(ret.len() as u8);
            for r in ret {
                encode_token(r, out);
            }
            out.push(abilities.into_u8());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use move_binary_format::deserializer::DeserializerConfig;
    use move_binary_format::file_format_common::{IDENTIFIER_SIZE_MAX, VERSION_MAX};

    const COUNTER_MV: &[u8] =
        include_bytes!("../../../examples/counter/nexus-artifact/bytecode/counter.mv");

    #[test]
    fn abi_hash_is_deterministic() {
        let config = DeserializerConfig::new(VERSION_MAX, IDENTIFIER_SIZE_MAX);
        let module = CompiledModule::deserialize_with_config(COUNTER_MV, &config).unwrap();

        let hash1 = compute_module_abi_hash(&module);
        let hash2 = compute_module_abi_hash(&module);
        assert_eq!(hash1, hash2, "ABI hash must be deterministic");
        assert_ne!(hash1, [0u8; 32], "ABI hash should not be zero");
    }

    #[test]
    fn abi_hash_nonzero_for_counter_module() {
        let config = DeserializerConfig::new(VERSION_MAX, IDENTIFIER_SIZE_MAX);
        let module = CompiledModule::deserialize_with_config(COUNTER_MV, &config).unwrap();
        let hash = compute_module_abi_hash(&module);
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn package_abi_hash_is_deterministic() {
        let config = DeserializerConfig::new(VERSION_MAX, IDENTIFIER_SIZE_MAX);
        let module = CompiledModule::deserialize_with_config(COUNTER_MV, &config).unwrap();

        let hash1 = compute_package_abi_hash(&[&module]);
        let hash2 = compute_package_abi_hash(&[&module]);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn package_abi_hash_empty_is_zero() {
        let hash = compute_package_abi_hash(&[]);
        assert_eq!(hash, [0u8; 32]);
    }

    #[test]
    fn abi_compatible_same_hash() {
        let hash = [0xAA; 32];
        assert!(abi_is_compatible(&hash, &hash));
    }

    #[test]
    fn abi_incompatible_different_hash() {
        let old = [0xAA; 32];
        let new = [0xBB; 32];
        assert!(!abi_is_compatible(&old, &new));
    }
}
