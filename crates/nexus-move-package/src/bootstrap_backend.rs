use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{BuildError, BuildOptions, BuildPlan, CompileBackend, CompiledModule, CompiledPackage};

pub struct MovePackageBackend {
    manifest_path: PathBuf,
}

impl MovePackageBackend {
    pub fn new() -> Self {
        Self {
            manifest_path: default_workspace_manifest(),
        }
    }

    pub fn with_manifest_path(manifest_path: impl Into<PathBuf>) -> Self {
        Self {
            manifest_path: manifest_path.into(),
        }
    }
}

impl Default for MovePackageBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CompileBackend for MovePackageBackend {
    fn compile(&self, plan: &BuildPlan) -> Result<CompiledPackage, BuildError> {
        let output = build_command(&self.manifest_path, &plan.options)
            .output()
            .map_err(|error| BuildError::Backend(error.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(BuildError::Backend(message));
        }

        let modules = load_build_modules(&plan.artifact_layout.build_dir)?;
        if modules.is_empty() {
            return Err(BuildError::NoModulesProduced);
        }

        let package_name = plan
            .move_toml
            .package_name
            .clone()
            .unwrap_or_else(|| package_name_from_path(&plan.artifact_layout.package_dir));

        Ok(CompiledPackage {
            package_name,
            upgrade_policy: plan.move_toml.upgrade_policy,
            named_addresses: crate::merged_named_addresses(plan),
            modules,
        })
    }
}

fn default_workspace_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../Nexus_Devnet_0.1.12_Pre/Cargo.toml")
}

fn build_command(manifest_path: &Path, options: &BuildOptions) -> Command {
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--bin")
        .arg("nexus-wallet")
        .arg("--")
        .arg("move")
        .arg("build")
        .arg("--package-dir")
        .arg(&options.package_dir);

    if options.skip_fetch_latest_git_deps {
        command.arg("--skip-fetch");
    }

    if !options.named_addresses.is_empty() {
        let csv = options
            .named_addresses
            .iter()
            .map(|entry| format!("{}={}", entry.name, entry.address))
            .collect::<Vec<_>>()
            .join(",");
        command.arg("--named-addresses").arg(csv);
    }

    command
}

fn load_build_modules(build_dir: &Path) -> Result<Vec<CompiledModule>, BuildError> {
    let mut modules = Vec::new();
    for package_entry in
        fs::read_dir(build_dir).map_err(|error| BuildError::Backend(error.to_string()))?
    {
        let package_entry =
            package_entry.map_err(|error| BuildError::Backend(error.to_string()))?;
        if !package_entry
            .file_type()
            .map_err(|error| BuildError::Backend(error.to_string()))?
            .is_dir()
        {
            continue;
        }

        let bytecode_dir = package_entry.path().join("bytecode_modules");
        if !bytecode_dir.exists() {
            continue;
        }

        for entry in
            fs::read_dir(&bytecode_dir).map_err(|error| BuildError::Backend(error.to_string()))?
        {
            let entry = entry.map_err(|error| BuildError::Backend(error.to_string()))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "mv") && path.is_file() {
                modules.push(CompiledModule {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    bytes: fs::read(&path)
                        .map_err(|error| BuildError::Backend(error.to_string()))?,
                });
            }
        }
    }

    modules.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(modules)
}

fn package_name_from_path(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{make_build_plan, BuildOptions};

    #[test]
    fn builds_expected_command_line() {
        let manifest = PathBuf::from("/tmp/main/Cargo.toml");
        let options = BuildOptions {
            package_dir: "/tmp/pkg".into(),
            named_addresses: vec![
                crate::parse_named_address_assignment("counter_addr=0xCAFE").unwrap()
            ],
            skip_fetch_latest_git_deps: true,
            known_attributes: vec![crate::KNOWN_ATTRIBUTE_VIEW.into()],
        };

        let command = build_command(&manifest, &options);
        let rendered = format!("{:?}", command);
        assert!(rendered.contains("--manifest-path"));
        assert!(rendered.contains("/tmp/main/Cargo.toml"));
        assert!(rendered.contains("nexus-wallet"));
        assert!(rendered.contains("move"));
        assert!(rendered.contains("build"));
        assert!(rendered.contains("--skip-fetch"));
        assert!(rendered.contains("counter_addr=0x"));
    }

    #[test]
    fn loads_modules_from_existing_counter_build() {
        let build_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../Nexus_Devnet_0.1.12_Pre/contracts/examples/counter/build");
        let modules = load_build_modules(&build_dir).unwrap();
        assert!(!modules.is_empty());
        assert_eq!(modules[0].name, "counter");
    }

    #[test]
    fn make_build_plan_can_target_existing_counter_example() {
        let package_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../Nexus_Devnet_0.1.12_Pre/contracts/examples/counter");
        let move_toml = fs::read_to_string(package_dir.join("Move.toml")).unwrap();
        let plan = make_build_plan(
            BuildOptions::bootstrap(package_dir.display().to_string()),
            &move_toml,
        )
        .unwrap();
        assert_eq!(plan.move_toml.package_name.as_deref(), Some("counter"));
    }
}
