//! Turnkey package build API.
//!
//! [`build_package`] is the replacement for the upstream
//! `move_package::BuildConfig::compile_package()` flow.  It reads the
//! package's `Move.toml`, compiles all sources using the
//! [`NativeCompileBackend`], and writes the resulting bytecode and
//! metadata to the `nexus-artifact/` directory.
//!
//! Feature-gated behind `native-compile`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    encode_artifact_manifest_json, encode_metadata, make_build_plan,
    native_backend::NativeCompileBackend, orchestrate_build, BuildError, BuildOptions,
    CompiledPackage, PackageMetadata,
};
use nexus_move_types::{AccountAddress, NamedAddressAssignment};

/// Result of a successful package build.
#[derive(Clone, Debug)]
pub struct BuildResult {
    /// The compiled package (modules + metadata).
    pub compiled: CompiledPackage,
    /// Package metadata for BCS serialization and on-chain storage.
    pub metadata: PackageMetadata,
    /// Path to the artifact directory.
    pub artifact_dir: PathBuf,
    /// Number of modules compiled.
    pub module_count: usize,
    /// Total bytecode size in bytes.
    pub total_bytes: usize,
}

/// Build a Move package from a directory containing `Move.toml` and
/// `sources/`.
///
/// This is a turnkey function that:
/// 1. Reads and parses `Move.toml`
/// 2. Merges CLI-provided named addresses with those in `Move.toml`
/// 3. Compiles all Move sources using the vendored `move-compiler-v2`
/// 4. Writes `.mv` bytecode files to `nexus-artifact/bytecode/`
/// 5. Writes `package-metadata.bcs` and `manifest.json` to `nexus-artifact/`
///
/// # Arguments
/// - `package_dir`: path to the Move package root (containing `Move.toml`)
/// - `additional_named_addresses`: extra `name=0xADDR` assignments from CLI
/// - `framework_bytecode_dir`: directory containing the stdlib `.mv` files;
///   if `None`, uses the embedded framework from `nexus-move-stdlib`
pub fn build_package(
    package_dir: &Path,
    additional_named_addresses: &[NamedAddressAssignment],
    framework_bytecode_dir: Option<&Path>,
) -> Result<BuildResult, BuildError> {
    // Read Move.toml
    let move_toml_path = package_dir.join("Move.toml");
    let move_toml_contents = fs::read_to_string(&move_toml_path)
        .map_err(|e| BuildError::PackageDirMissing(format!("{}: {e}", move_toml_path.display())))?;

    // Build plan
    let mut options = BuildOptions::bootstrap(package_dir.display().to_string());
    options
        .named_addresses
        .extend(additional_named_addresses.iter().cloned());

    // Add std=0x1 if not already provided (framework modules live at 0x1)
    let has_std = options.named_addresses.iter().any(|a| a.name == "std")
        || crate::inspect_move_toml(&move_toml_contents)
            .named_addresses
            .iter()
            .any(|a| a.name == "std");
    if !has_std {
        options.named_addresses.push(NamedAddressAssignment {
            name: "std".into(),
            address: AccountAddress::from_hex_literal("0x1").expect("0x1 is a valid address"),
        });
    }

    let plan = make_build_plan(options, &move_toml_contents)?;

    // Resolve framework .mv directory for the compiler
    let fw_dir = match framework_bytecode_dir {
        Some(dir) => dir.to_path_buf(),
        None => {
            // Write embedded framework modules to a temp dir
            let tmp = plan.artifact_layout.build_dir.join("framework");
            write_embedded_framework(&tmp)?;
            tmp
        }
    };

    // Compile
    let backend = NativeCompileBackend::with_bytecode_deps(vec![fw_dir]);
    let (compiled, metadata, manifest) = orchestrate_build(&backend, &plan)?;

    // Write artifacts
    let layout = &plan.artifact_layout;
    fs::create_dir_all(&layout.bytecode_dir).map_err(|e| {
        BuildError::Backend(format!(
            "failed to create {}: {e}",
            layout.bytecode_dir.display()
        ))
    })?;

    // Write .mv files
    for module in &compiled.modules {
        let mv_path = layout.bytecode_dir.join(format!("{}.mv", module.name));
        fs::write(&mv_path, &module.bytes).map_err(|e| {
            BuildError::Backend(format!("failed to write {}: {e}", mv_path.display()))
        })?;
    }

    // Write package-metadata.bcs
    let metadata_bytes = encode_metadata(&metadata)
        .map_err(|e| BuildError::Backend(format!("metadata serialization failed: {e}")))?;
    let metadata_path = layout.artifact_dir.join("package-metadata.bcs");
    fs::write(&metadata_path, &metadata_bytes)
        .map_err(|e| BuildError::Backend(format!("failed to write metadata: {e}")))?;

    // Write manifest.json
    let manifest_json = encode_artifact_manifest_json(&manifest)
        .map_err(|e| BuildError::Backend(format!("manifest JSON failed: {e}")))?;
    let manifest_path = layout.artifact_dir.join("manifest.json");
    fs::write(&manifest_path, manifest_json.as_bytes())
        .map_err(|e| BuildError::Backend(format!("failed to write manifest: {e}")))?;

    let total_bytes = compiled.modules.iter().map(|m| m.bytes.len()).sum();
    let module_count = compiled.modules.len();

    Ok(BuildResult {
        compiled,
        metadata,
        artifact_dir: layout.artifact_dir.clone(),
        module_count,
        total_bytes,
    })
}

/// Write the embedded framework `.mv` modules to a directory on disk
/// so they can be passed to the compiler as bytecode dependencies.
fn write_embedded_framework(dir: &Path) -> Result<(), BuildError> {
    fs::create_dir_all(dir)
        .map_err(|e| BuildError::Backend(format!("failed to create framework dir: {e}")))?;

    for (name, bytes) in nexus_move_stdlib::FRAMEWORK_MODULES {
        let path = dir.join(format!("{name}.mv"));
        // Only write if missing or different size (idempotent)
        let needs_write = match fs::metadata(&path) {
            Ok(meta) => meta.len() != bytes.len() as u64,
            Err(_) => true,
        };
        if needs_write {
            fs::write(&path, bytes).map_err(|e| {
                BuildError::Backend(format!("failed to write {}: {e}", path.display()))
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_package_compiles_counter_example() {
        let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter");

        if !example_dir.join("Move.toml").exists() {
            eprintln!("skipping: examples/counter not found");
            return;
        }

        let result = build_package(&example_dir, &[], None).unwrap();

        assert_eq!(result.compiled.package_name, "counter");
        assert_eq!(result.module_count, 1);
        assert!(result.total_bytes > 0);

        // Verify artifacts were written
        assert!(result.artifact_dir.join("package-metadata.bcs").exists());
        assert!(result.artifact_dir.join("manifest.json").exists());
        assert!(result.artifact_dir.join("bytecode/counter.mv").exists());

        // Verify bytecode magic
        let mv = fs::read(result.artifact_dir.join("bytecode/counter.mv")).unwrap();
        assert_eq!(&mv[..4], &[0xa1, 0x1c, 0xeb, 0x0b]);
    }

    #[test]
    fn build_package_with_custom_named_address() {
        let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter");

        if !example_dir.join("Move.toml").exists() {
            eprintln!("skipping: examples/counter not found");
            return;
        }

        let custom_addr = NamedAddressAssignment {
            name: "counter_addr".into(),
            address: AccountAddress::from_hex_literal("0xBEEF").unwrap(),
        };

        let result = build_package(&example_dir, &[custom_addr], None).unwrap();
        assert_eq!(result.compiled.package_name, "counter");
        assert_eq!(result.module_count, 1);
    }

    #[test]
    fn build_package_compiles_emitter_example() {
        let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/emitter");

        if !example_dir.join("Move.toml").exists() {
            eprintln!("skipping: examples/emitter not found");
            return;
        }

        let result = build_package(&example_dir, &[], None).unwrap();

        assert_eq!(result.compiled.package_name, "emitter");
        assert_eq!(result.module_count, 1);
        assert!(result.total_bytes > 0);

        // Verify artifacts
        assert!(result.artifact_dir.join("bytecode/emitter.mv").exists());
        let mv = fs::read(result.artifact_dir.join("bytecode/emitter.mv")).unwrap();
        assert_eq!(&mv[..4], &[0xa1, 0x1c, 0xeb, 0x0b]);
    }
}
