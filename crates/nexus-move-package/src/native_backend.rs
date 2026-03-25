//! Native compile backend.
//!
//! Uses the vendored `move-compiler-v2` to compile Move source files
//! directly into bytecode without shelling out to any external process.
//!
//! Feature-gated behind `native-compile`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codespan_reporting::term::termcolor::NoColor;
use legacy_move_compiler::compiled_unit::AnnotatedCompiledUnit;
use move_compiler_v2::Options;

use crate::{
    BuildError, BuildPlan, CompileBackend, CompiledModule, CompiledPackage, KNOWN_ATTRIBUTE_VIEW,
};

/// Backend that compiles Move source files using the vendored `move-compiler-v2`.
pub struct NativeCompileBackend {
    /// Additional stdlib source directories to include as dependencies.
    stdlib_dirs: Vec<PathBuf>,
    /// Directories containing pre-compiled `.mv` bytecode dependencies.
    bytecode_dep_dirs: Vec<PathBuf>,
}

impl NativeCompileBackend {
    /// Create a new native backend that compiles with no extra stdlib dirs.
    pub fn new() -> Self {
        Self {
            stdlib_dirs: Vec::new(),
            bytecode_dep_dirs: Vec::new(),
        }
    }

    /// Create a backend with extra stdlib source directories as dependencies.
    pub fn with_stdlib_dirs(stdlib_dirs: Vec<PathBuf>) -> Self {
        Self {
            stdlib_dirs,
            bytecode_dep_dirs: Vec::new(),
        }
    }

    /// Create a backend with pre-compiled bytecode dependency directories.
    pub fn with_bytecode_deps(bytecode_dep_dirs: Vec<PathBuf>) -> Self {
        Self {
            stdlib_dirs: Vec::new(),
            bytecode_dep_dirs,
        }
    }
}

impl Default for NativeCompileBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CompileBackend for NativeCompileBackend {
    fn compile(&self, plan: &BuildPlan) -> Result<CompiledPackage, BuildError> {
        let sources_dir = Path::new(&plan.options.package_dir).join("sources");
        if !sources_dir.exists() {
            return Err(BuildError::PackageDirMissing(
                sources_dir.display().to_string(),
            ));
        }

        let named_addresses = crate::merged_named_addresses(plan);
        let address_mapping: Vec<String> = named_addresses
            .iter()
            .map(|na| {
                let hex = na
                    .address
                    .0
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                format!("{}=0x{}", na.name, hex)
            })
            .collect();

        let mut known_attrs = BTreeSet::new();
        for attr in &plan.options.known_attributes {
            known_attrs.insert(attr.clone());
        }
        known_attrs.insert(KNOWN_ATTRIBUTE_VIEW.to_string());

        let mut sources_deps = Vec::new();
        for dir in &self.stdlib_dirs {
            if dir.exists() {
                sources_deps.push(dir.display().to_string());
            }
        }

        // Pre-compiled .mv bytecode dependency directories
        let dependencies: Vec<String> = self
            .bytecode_dep_dirs
            .iter()
            .filter(|d| d.exists())
            .map(|d| d.display().to_string())
            .collect();

        let options = Options {
            sources: vec![sources_dir.display().to_string()],
            sources_deps,
            dependencies,
            named_address_mapping: address_mapping,
            skip_attribute_checks: false,
            known_attributes: known_attrs,
            whole_program: false,
            ..Options::default()
        };

        let mut error_buf = NoColor::new(Vec::new());
        let mut emitter = options.error_emitter(&mut error_buf);
        let (_env, units) = move_compiler_v2::run_move_compiler(emitter.as_mut(), options)
            .map_err(|e| BuildError::Backend(format!("compilation failed: {e}")))?;

        let mut modules = Vec::new();
        for unit in units {
            match unit {
                AnnotatedCompiledUnit::Module(m) => {
                    let named = m.named_module;
                    let name = named.name.to_string();
                    let mut bytes = Vec::new();
                    named
                        .module
                        .serialize(&mut bytes)
                        .map_err(|e| BuildError::Backend(format!("serialization failed: {e}")))?;
                    modules.push(CompiledModule { name, bytes });
                }
                AnnotatedCompiledUnit::Script(_) => {
                    // Scripts are not packaged into modules
                }
            }
        }

        let package_name = plan
            .move_toml
            .package_name
            .clone()
            .unwrap_or_else(|| "unknown".into());

        Ok(CompiledPackage {
            package_name,
            upgrade_policy: plan.move_toml.upgrade_policy,
            named_addresses: named_addresses.clone(),
            modules,
        })
    }
}

/// Compile a single Move source file to bytecode with named addresses.
///
/// This is a convenience function for compiling individual files without
/// setting up a full package build plan.
///
/// - `source_path`: path to the `.move` source file
/// - `source_dep_dirs`: directories of additional Move source dependencies
/// - `bytecode_dep_dirs`: directories containing pre-compiled `.mv` files
/// - `named_addresses`: list of `(name, hex_address)` pairs
pub fn compile_source_file(
    source_path: &Path,
    source_dep_dirs: &[PathBuf],
    bytecode_dep_dirs: &[PathBuf],
    named_addresses: &[(String, String)],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let address_mapping: Vec<String> = named_addresses
        .iter()
        .map(|(name, addr)| format!("{name}={addr}"))
        .collect();

    let mut known_attrs = BTreeSet::new();
    known_attrs.insert(KNOWN_ATTRIBUTE_VIEW.to_string());

    let sources_deps: Vec<String> = source_dep_dirs
        .iter()
        .filter(|d| d.exists())
        .map(|d| d.display().to_string())
        .collect();

    let dependencies: Vec<String> = bytecode_dep_dirs
        .iter()
        .filter(|d| d.exists())
        .map(|d| d.display().to_string())
        .collect();

    let options = Options {
        sources: vec![source_path.display().to_string()],
        sources_deps,
        dependencies,
        named_address_mapping: address_mapping,
        skip_attribute_checks: true,
        known_attributes: known_attrs,
        whole_program: false,
        ..Options::default()
    };

    let mut error_buf = NoColor::new(Vec::new());
    let mut emitter = options.error_emitter(&mut error_buf);
    let (_env, units) = move_compiler_v2::run_move_compiler(emitter.as_mut(), options)
        .map_err(|e| format!("compilation failed: {e}"))?;

    let mut result = Vec::new();
    for unit in units {
        if let AnnotatedCompiledUnit::Module(m) = unit {
            let named = m.named_module;
            let name = named.name.to_string();
            let mut bytes = Vec::new();
            named
                .module
                .serialize(&mut bytes)
                .map_err(|e| format!("serialization failed: {e}"))?;
            result.push((name, bytes));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_build_plan, BuildOptions};
    use std::fs;

    fn framework_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../nexus-move-stdlib/src/framework")
    }

    #[test]
    fn compiles_counter_from_example_sources() {
        let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter");

        if !example_dir.join("sources/counter.move").exists() {
            eprintln!("skipping: examples/counter/sources/counter.move not found");
            return;
        }

        let mut opts = BuildOptions::bootstrap(example_dir.display().to_string());
        // The counter module uses std::signer, so add std=0x1
        opts.named_addresses.push(crate::NamedAddressAssignment {
            name: "std".into(),
            address: crate::AccountAddress([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ]),
        });

        let plan = make_build_plan(
            opts,
            &fs::read_to_string(example_dir.join("Move.toml")).unwrap_or_default(),
        )
        .unwrap();

        let backend = NativeCompileBackend::with_bytecode_deps(vec![framework_dir()]);
        let result = backend.compile(&plan).unwrap();
        assert_eq!(result.package_name, "counter");
        assert!(!result.modules.is_empty());

        let counter_module = result.modules.iter().find(|m| m.name == "counter");
        assert!(counter_module.is_some(), "counter module not found");
        assert!(
            !counter_module.unwrap().bytes.is_empty(),
            "counter bytecode is empty"
        );

        // Verify bytecode magic number
        let bytes = &counter_module.unwrap().bytes;
        assert!(bytes.len() >= 4);
        assert_eq!(bytes[0], 0xa1);
        assert_eq!(bytes[1], 0x1c);
        assert_eq!(bytes[2], 0xeb);
        assert_eq!(bytes[3], 0x0b);
    }

    #[test]
    fn compile_source_file_helper_works() {
        let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter");
        let source = example_dir.join("sources/counter.move");

        if !source.exists() {
            eprintln!("skipping: counter.move not found");
            return;
        }

        let result = compile_source_file(
            &source,
            &[],
            &[framework_dir()],
            &[
                ("counter_addr".to_string(), "0xCAFE".to_string()),
                ("std".to_string(), "0x1".to_string()),
            ],
        )
        .unwrap();

        assert!(!result.is_empty());
        assert_eq!(result[0].0, "counter");
    }

    /// Codegen test: compile nursery modules (guid.move, event.move) and write
    /// the resulting .mv files to the framework directory.
    /// Run with: cargo test -p nexus-move-package --features native-compile \
    ///           -- generate_nursery_bytecode --exact --ignored
    #[test]
    #[ignore]
    fn generate_nursery_bytecode() {
        let nursery_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/nursery/sources");
        let fw_dir = framework_dir();

        assert!(
            nursery_dir.exists(),
            "nursery dir not found: {}",
            nursery_dir.display()
        );

        // Compile both guid.move and event.move together as a single source dir.
        // guid depends on signer (in framework), event depends on guid + bcs (in framework).
        let result = compile_source_file(
            &nursery_dir,
            &[],
            &[fw_dir.clone()],
            &[("std".to_string(), "0x1".to_string())],
        )
        .expect("nursery compilation failed");

        assert!(
            result.len() >= 2,
            "expected at least guid + event modules, got {}",
            result.len()
        );

        for (name, bytes) in &result {
            let out_path = fw_dir.join(format!("{name}.mv"));
            fs::write(&out_path, bytes).unwrap_or_else(|e| {
                panic!("failed to write {}: {e}", out_path.display());
            });
            println!("  wrote {}.mv  ({} bytes)", name, bytes.len());

            // Verify magic number
            assert!(bytes.len() >= 4, "{name}.mv too short");
            assert_eq!(
                &bytes[..4],
                &[0xa1, 0x1c, 0xeb, 0x0b],
                "{name}.mv bad magic"
            );
        }

        // Verify expected modules
        let names: Vec<&str> = result.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"guid"), "guid module missing: {names:?}");
        assert!(names.contains(&"event"), "event module missing: {names:?}");
    }
}
