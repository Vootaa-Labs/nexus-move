#![forbid(unsafe_code)]

#[cfg(feature = "bootstrap-vendor")]
pub mod bootstrap_backend;

#[cfg(feature = "native-compile")]
pub mod build;

#[cfg(feature = "native-compile")]
pub mod native_backend;

pub mod precompiled_backend;

#[cfg(feature = "verified-compile")]
pub mod verified_backend;

use std::fmt;
use std::path::{Path, PathBuf};

use nexus_move_stdlib::StdlibSnapshot;
use nexus_move_types::{AccountAddress, AddressParseError, HashValue, NamedAddressAssignment};
use serde::{Deserialize, Serialize};

pub const KNOWN_ATTRIBUTE_VIEW: &str = "view";
pub const DEFAULT_ARTIFACT_DIR: &str = "nexus-artifact";
pub const DEFAULT_BYTECODE_DIR: &str = "bytecode";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum UpgradePolicy {
    #[default]
    Immutable,
    Compatible,
    GovernanceOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PackageMetadata {
    pub name: String,
    pub package_hash: HashValue,
    pub named_addresses: Vec<(String, AccountAddress)>,
    pub module_hashes: Vec<(String, HashValue)>,
    pub abi_hash: HashValue,
    pub upgrade_policy: UpgradePolicy,
    pub deployer: AccountAddress,
    pub version: u64,
}

pub fn encode_metadata(meta: &PackageMetadata) -> Result<Vec<u8>, bcs::Error> {
    bcs::to_bytes(meta)
}

pub fn decode_metadata(bytes: &[u8]) -> Result<PackageMetadata, bcs::Error> {
    bcs::from_bytes(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildOptions {
    pub package_dir: String,
    pub named_addresses: Vec<NamedAddressAssignment>,
    pub skip_fetch_latest_git_deps: bool,
    pub known_attributes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildArtifactLayout {
    pub package_dir: PathBuf,
    pub build_dir: PathBuf,
    pub artifact_dir: PathBuf,
    pub bytecode_dir: PathBuf,
}

impl BuildArtifactLayout {
    pub fn for_package_dir(package_dir: impl Into<PathBuf>) -> Self {
        let package_dir = package_dir.into();
        let build_dir = package_dir.join("build");
        let artifact_dir = package_dir.join(DEFAULT_ARTIFACT_DIR);
        let bytecode_dir = artifact_dir.join(DEFAULT_BYTECODE_DIR);
        Self {
            package_dir,
            build_dir,
            artifact_dir,
            bytecode_dir,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledModule {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledPackage {
    pub package_name: String,
    pub upgrade_policy: UpgradePolicy,
    pub named_addresses: Vec<NamedAddressAssignment>,
    pub modules: Vec<CompiledModule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactManifest {
    pub package_name: String,
    pub package_hash: String,
    pub module_count: usize,
    pub total_bytecode_bytes: usize,
    pub upgrade_policy: String,
    pub modules: Vec<ArtifactModule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactModule {
    pub name: String,
    pub size_bytes: usize,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildPlan {
    pub options: BuildOptions,
    pub move_toml: MoveTomlInfo,
    pub artifact_layout: BuildArtifactLayout,
}

pub trait CompileBackend {
    fn compile(&self, plan: &BuildPlan) -> Result<CompiledPackage, BuildError>;
}

impl BuildOptions {
    pub fn bootstrap(package_dir: impl Into<String>) -> Self {
        Self {
            package_dir: package_dir.into(),
            named_addresses: Vec::new(),
            skip_fetch_latest_git_deps: true,
            known_attributes: vec![KNOWN_ATTRIBUTE_VIEW.to_string()],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageFrontendPlan {
    pub allows_remote_git: bool,
    pub stdlib_snapshot: StdlibSnapshot,
    pub build_options: BuildOptions,
}

impl PackageFrontendPlan {
    pub fn bootstrap(package_dir: impl Into<String>) -> Self {
        Self {
            allows_remote_git: false,
            stdlib_snapshot: StdlibSnapshot::bootstrap(),
            build_options: BuildOptions::bootstrap(package_dir),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveTomlInfo {
    pub package_name: Option<String>,
    pub upgrade_policy: UpgradePolicy,
    pub named_addresses: Vec<NamedAddressAssignment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildConfigError {
    MissingSeparator(String),
    InvalidAddress {
        input: String,
        reason: AddressParseError,
    },
}

impl fmt::Display for BuildConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSeparator(input) => {
                write!(
                    f,
                    "invalid named-address assignment '{input}', expected name=0xADDR"
                )
            }
            Self::InvalidAddress { input, reason } => {
                write!(f, "invalid address '{input}': {reason}")
            }
        }
    }
}

impl std::error::Error for BuildConfigError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildError {
    InvalidConfig(BuildConfigError),
    PackageDirMissing(String),
    Backend(String),
    NoModulesProduced,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(error) => write!(f, "invalid build config: {error}"),
            Self::PackageDirMissing(path) => write!(f, "package directory does not exist: {path}"),
            Self::Backend(message) => write!(f, "bootstrap build backend failed: {message}"),
            Self::NoModulesProduced => write!(f, "compile backend produced no modules"),
        }
    }
}

impl std::error::Error for BuildError {}

pub fn parse_named_address_assignment(
    input: &str,
) -> Result<NamedAddressAssignment, BuildConfigError> {
    let (name, address) = input
        .split_once('=')
        .ok_or_else(|| BuildConfigError::MissingSeparator(input.to_string()))?;
    let parsed = AccountAddress::from_hex_literal(address.trim()).map_err(|reason| {
        BuildConfigError::InvalidAddress {
            input: address.trim().to_string(),
            reason,
        }
    })?;

    Ok(NamedAddressAssignment {
        name: name.trim().to_string(),
        address: parsed,
    })
}

pub fn parse_named_address_assignments(
    inputs: &[String],
) -> Result<Vec<NamedAddressAssignment>, BuildConfigError> {
    inputs
        .iter()
        .map(|input| parse_named_address_assignment(input))
        .collect()
}

pub fn make_build_plan(
    options: BuildOptions,
    move_toml_contents: &str,
) -> Result<BuildPlan, BuildError> {
    if !Path::new(&options.package_dir).exists() {
        return Err(BuildError::PackageDirMissing(options.package_dir.clone()));
    }

    let move_toml = inspect_move_toml(move_toml_contents);
    Ok(BuildPlan {
        artifact_layout: BuildArtifactLayout::for_package_dir(&options.package_dir),
        move_toml,
        options,
    })
}

pub fn merged_named_addresses(plan: &BuildPlan) -> Vec<NamedAddressAssignment> {
    if plan.options.named_addresses.is_empty() {
        return plan.move_toml.named_addresses.clone();
    }

    let mut merged = plan.move_toml.named_addresses.clone();
    for override_entry in &plan.options.named_addresses {
        if let Some(existing) = merged
            .iter_mut()
            .find(|entry| entry.name == override_entry.name)
        {
            *existing = override_entry.clone();
        } else {
            merged.push(override_entry.clone());
        }
    }
    merged
}

pub fn build_package_metadata(
    compiled: &CompiledPackage,
    deployer: AccountAddress,
) -> PackageMetadata {
    let package_hash = *blake3::hash(
        &compiled
            .modules
            .iter()
            .flat_map(|module| module.bytes.clone())
            .collect::<Vec<u8>>(),
    )
    .as_bytes();

    let module_hashes = compiled
        .modules
        .iter()
        .map(|module| (module.name.clone(), *blake3::hash(&module.bytes).as_bytes()))
        .collect();

    PackageMetadata {
        name: compiled.package_name.clone(),
        package_hash,
        named_addresses: compiled
            .named_addresses
            .iter()
            .map(|entry| (entry.name.clone(), entry.address))
            .collect(),
        module_hashes,
        abi_hash: [0u8; 32],
        upgrade_policy: compiled.upgrade_policy,
        deployer,
        version: 1,
    }
}

pub fn build_artifact_manifest(compiled: &CompiledPackage) -> ArtifactManifest {
    let package_hash = blake3::hash(
        &compiled
            .modules
            .iter()
            .flat_map(|module| module.bytes.clone())
            .collect::<Vec<u8>>(),
    );

    ArtifactManifest {
        package_name: compiled.package_name.clone(),
        package_hash: package_hash.to_hex().to_string(),
        module_count: compiled.modules.len(),
        total_bytecode_bytes: compiled
            .modules
            .iter()
            .map(|module| module.bytes.len())
            .sum(),
        upgrade_policy: format!("{:?}", compiled.upgrade_policy),
        modules: compiled
            .modules
            .iter()
            .map(|module| ArtifactModule {
                name: module.name.clone(),
                size_bytes: module.bytes.len(),
                blake3: blake3::hash(&module.bytes).to_hex().to_string(),
            })
            .collect(),
    }
}

pub fn encode_artifact_manifest_json(
    manifest: &ArtifactManifest,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(manifest)
}

pub fn orchestrate_build<B: CompileBackend>(
    backend: &B,
    plan: &BuildPlan,
) -> Result<(CompiledPackage, PackageMetadata, ArtifactManifest), BuildError> {
    let compiled = backend.compile(plan)?;
    if compiled.modules.is_empty() {
        return Err(BuildError::NoModulesProduced);
    }
    let metadata = build_package_metadata(&compiled, AccountAddress::ZERO);
    let manifest = build_artifact_manifest(&compiled);
    Ok((compiled, metadata, manifest))
}

pub fn inspect_move_toml(contents: &str) -> MoveTomlInfo {
    let mut current_section = String::new();
    let mut package_name = None;
    let mut package_policy = UpgradePolicy::Immutable;
    let mut addresses = Vec::new();
    let mut dev_addresses = Vec::new();

    for raw_line in contents.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim().trim_matches('"');

        match current_section.as_str() {
            "package" if key == "name" => {
                package_name = Some(value.to_string());
            }
            "package" if key == "upgrade_policy" => {
                package_policy = match value {
                    "compatible" => UpgradePolicy::Compatible,
                    "governance" | "governance_only" => UpgradePolicy::GovernanceOnly,
                    _ => UpgradePolicy::Immutable,
                };
            }
            "addresses" => push_named_address(key, value, &mut addresses),
            "dev-addresses" => push_named_address(key, value, &mut dev_addresses),
            _ => {}
        }
    }

    MoveTomlInfo {
        package_name,
        upgrade_policy: package_policy,
        named_addresses: if dev_addresses.is_empty() {
            addresses
        } else {
            dev_addresses
        },
    }
}

fn push_named_address(name: &str, value: &str, output: &mut Vec<NamedAddressAssignment>) {
    if value == "_" {
        return;
    }

    if let Ok(address) = AccountAddress::from_hex_literal(value) {
        output.push(NamedAddressAssignment {
            name: name.to_string(),
            address,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubBackend;

    impl CompileBackend for StubBackend {
        fn compile(&self, plan: &BuildPlan) -> Result<CompiledPackage, BuildError> {
            Ok(CompiledPackage {
                package_name: "counter".into(),
                upgrade_policy: plan.move_toml.upgrade_policy,
                named_addresses: merged_named_addresses(plan),
                modules: vec![CompiledModule {
                    name: "counter".into(),
                    bytes: vec![0xa1, 0x1c, 0xeb, 0x0b, 1, 0, 0, 0],
                }],
            })
        }
    }

    #[test]
    fn parses_named_address_assignment() {
        let parsed = parse_named_address_assignment("counter_addr=0xCAFE").unwrap();
        assert_eq!(parsed.name, "counter_addr");
        assert_eq!(parsed.address.0[30], 0xCA);
        assert_eq!(parsed.address.0[31], 0xFE);
    }

    #[test]
    fn build_plan_defaults_to_offline_mode() {
        let plan = PackageFrontendPlan::bootstrap("contracts/examples/counter");
        assert!(!plan.allows_remote_git);
        assert!(plan.build_options.skip_fetch_latest_git_deps);
        assert_eq!(
            plan.build_options.known_attributes,
            vec!["view".to_string()]
        );
    }

    #[test]
    fn inspect_move_toml_prefers_dev_addresses() {
        let info = inspect_move_toml(
            r#"
            [package]
            name = "counter"
            upgrade_policy = "compatible"

            [addresses]
            counter_addr = "0x1"

            [dev-addresses]
            counter_addr = "0xCAFE"
            "#,
        );

        assert_eq!(info.package_name.as_deref(), Some("counter"));
        assert_eq!(info.upgrade_policy, UpgradePolicy::Compatible);
        assert_eq!(info.named_addresses.len(), 1);
        assert_eq!(info.named_addresses[0].address.0[30], 0xCA);
        assert_eq!(info.named_addresses[0].address.0[31], 0xFE);
    }

    #[test]
    fn skips_placeholder_named_addresses() {
        let info = inspect_move_toml(
            r#"
            [addresses]
            counter_addr = "_"
            "#,
        );
        assert!(info.named_addresses.is_empty());
    }

    #[test]
    fn package_metadata_round_trips_via_bcs() {
        let meta = PackageMetadata {
            name: "counter".into(),
            package_hash: [0xAA; 32],
            named_addresses: vec![("counter_addr".into(), AccountAddress([0x11; 32]))],
            module_hashes: vec![("counter".into(), [0xBB; 32])],
            abi_hash: [0xCC; 32],
            upgrade_policy: UpgradePolicy::Compatible,
            deployer: AccountAddress([0x22; 32]),
            version: 1,
        };

        let encoded = encode_metadata(&meta).unwrap();
        let decoded = decode_metadata(&encoded).unwrap();
        assert_eq!(decoded, meta);
    }

    #[test]
    fn inspect_move_toml_supports_governance_alias() {
        let info = inspect_move_toml(
            r#"
            [package]
            upgrade_policy = "governance"
            "#,
        );
        assert_eq!(info.upgrade_policy, UpgradePolicy::GovernanceOnly);
    }

    #[test]
    fn build_plan_merges_cli_named_addresses() {
        let temp_dir = std::env::temp_dir().join("nexus-move-package-test-plan");
        let _ = std::fs::create_dir_all(&temp_dir);
        let plan = make_build_plan(
            BuildOptions {
                package_dir: temp_dir.display().to_string(),
                named_addresses: vec![NamedAddressAssignment {
                    name: "counter_addr".into(),
                    address: AccountAddress::from_hex_literal("0xCAFE").unwrap(),
                }],
                skip_fetch_latest_git_deps: true,
                known_attributes: vec![KNOWN_ATTRIBUTE_VIEW.into()],
            },
            r#"
            [addresses]
            counter_addr = "0x1"
            "#,
        )
        .unwrap();

        let merged = merged_named_addresses(&plan);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].address.0[30], 0xCA);
        assert_eq!(merged[0].address.0[31], 0xFE);
    }

    #[test]
    fn orchestrate_build_produces_metadata_and_manifest() {
        let temp_dir = std::env::temp_dir().join("nexus-move-package-test-orchestrate");
        let _ = std::fs::create_dir_all(&temp_dir);
        let plan = make_build_plan(
            BuildOptions::bootstrap(temp_dir.display().to_string()),
            r#"
            [package]
            name = "counter"
            upgrade_policy = "compatible"
            "#,
        )
        .unwrap();

        let (_compiled, metadata, manifest) = orchestrate_build(&StubBackend, &plan).unwrap();
        assert_eq!(metadata.name, "counter");
        assert_eq!(metadata.upgrade_policy, UpgradePolicy::Compatible);
        assert_eq!(manifest.module_count, 1);
        assert_eq!(manifest.package_name, "counter");
        assert!(encode_artifact_manifest_json(&manifest)
            .unwrap()
            .contains("counter"));
    }
}
