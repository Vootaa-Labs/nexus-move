//! Precompiled backend.
//!
//! Loads pre-compiled `.mv` bytecode files from an artifact directory.
//! Used for offline builds where bytecode has already been compiled
//! and just needs to be packaged with metadata.

use std::fs;
use std::path::PathBuf;

use crate::{
    BuildError, BuildPlan, CompileBackend, CompiledModule, CompiledPackage, DEFAULT_BYTECODE_DIR,
};

/// Backend that loads pre-compiled `.mv` files from the artifact directory.
pub struct PrecompiledBackend {
    bytecode_dir: Option<PathBuf>,
}

impl PrecompiledBackend {
    /// Create a backend that will discover `.mv` files from the plan's artifact layout.
    pub fn new() -> Self {
        Self { bytecode_dir: None }
    }

    /// Create a backend with an explicit bytecode directory.
    pub fn with_dir(bytecode_dir: PathBuf) -> Self {
        Self {
            bytecode_dir: Some(bytecode_dir),
        }
    }
}

impl Default for PrecompiledBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CompileBackend for PrecompiledBackend {
    fn compile(&self, plan: &BuildPlan) -> Result<CompiledPackage, BuildError> {
        let bytecode_dir = self.bytecode_dir.clone().unwrap_or_else(|| {
            plan.artifact_layout
                .package_dir
                .join("nexus-artifact")
                .join(DEFAULT_BYTECODE_DIR)
        });

        if !bytecode_dir.exists() {
            return Err(BuildError::PackageDirMissing(
                bytecode_dir.display().to_string(),
            ));
        }

        let mut modules = Vec::new();
        let entries = fs::read_dir(&bytecode_dir).map_err(|e| {
            BuildError::Backend(format!(
                "failed to read bytecode dir {}: {e}",
                bytecode_dir.display()
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Backend(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mv") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let bytes = fs::read(&path).map_err(|e| {
                    BuildError::Backend(format!("failed to read {}: {e}", path.display()))
                })?;
                modules.push(CompiledModule { name, bytes });
            }
        }

        modules.sort_by(|a, b| a.name.cmp(&b.name));

        let package_name = plan
            .move_toml
            .package_name
            .clone()
            .unwrap_or_else(|| "unknown".into());

        Ok(CompiledPackage {
            package_name,
            upgrade_policy: plan.move_toml.upgrade_policy,
            named_addresses: crate::merged_named_addresses(plan),
            modules,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_build_plan, BuildOptions};

    #[test]
    fn loads_precompiled_counter_from_fixture_dir() {
        let fixture_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../nexus-move-runtime/tests/fixtures");

        let temp_dir = std::env::temp_dir().join("nexus-precompiled-test");
        let _ = std::fs::create_dir_all(&temp_dir);

        let plan = make_build_plan(
            BuildOptions::bootstrap(temp_dir.display().to_string()),
            "[package]\nname = \"counter\"\n",
        )
        .unwrap();

        let backend = PrecompiledBackend::with_dir(fixture_dir);
        let result = backend.compile(&plan).unwrap();
        assert_eq!(result.package_name, "counter");
        assert!(result.modules.len() >= 1);
        assert!(result
            .modules
            .iter()
            .any(|m| m.name == "counter" && !m.bytes.is_empty()));
    }

    #[test]
    fn rejects_missing_bytecode_dir() {
        let temp_dir = std::env::temp_dir().join("nexus-precompiled-missing");
        let _ = std::fs::create_dir_all(&temp_dir);

        let plan = make_build_plan(
            BuildOptions::bootstrap(temp_dir.display().to_string()),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();

        let backend = PrecompiledBackend::with_dir(PathBuf::from("/nonexistent/path"));
        let result = backend.compile(&plan);
        assert!(result.is_err());
    }
}
