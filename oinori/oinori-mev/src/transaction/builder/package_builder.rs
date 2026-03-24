use anyhow::{anyhow, Error, Result};
use aptos_types::account_address::AccountAddress;

use aptos_framework::{BuildOptions, BuiltPackage};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::transaction::builder::script::*;
use aptos_types::transaction::TransactionPayload;

use move_model::metadata::{CompilerVersion, LanguageVersion};

const BYTECODE_VERSION: u32 = 6;
#[derive(Debug)]
pub struct BuildPackageInfo {
    package_dir: PathBuf,
    named_addresses: Option<NamedAddreses>,
    fat_package: bool,
}

impl BuildPackageInfo {
    pub fn new(
        package_dir: PathBuf,
        named_addresses: Option<NamedAddreses>,
        fat_package: bool,
    ) -> Self {
        Self {
            named_addresses,
            package_dir,
            fat_package,
        }
    }

    pub fn build_package(self) -> Result<BuiltPackage> {
        validate_module_package_dir(&self.package_dir)?;

        let BuildPackageInfo {
            package_dir,
            named_addresses,
            fat_package,
        } = self;

        let build_option = if fat_package {
            fat_build_option(named_addresses)
        } else {
            compact_build_option(named_addresses)
        };

        let package = BuiltPackage::build(package_dir, build_option)?;
        Ok(package)
    }

    pub fn build_script_call(self) -> Result<ScriptCall> {
        Ok(ScriptCall::build_script(self)?)
    }

    pub fn build_module_publish_transaction(self) -> Result<TransactionPayload> {
        // Send the compiled module and metadata using the code::publish_package_txn.
        let package = self.build_package()?;
        let compiled_units = package.extract_code();
        let metadata = package.extract_metadata()?;
        let payload = aptos_cached_packages::aptos_stdlib::code_publish_package_txn(
            bcs::to_bytes(&metadata).expect("PackageMetadata has BCS"),
            compiled_units,
        );

        Ok(payload)
    }
}

fn validate_module_package_dir(module_package_dir: &Path) -> Result<()> {
    let mut pb = PathBuf::new();
    pb.push(module_package_dir);
    pb.push("Move.toml");

    if pb.exists() {
        Ok(())
    } else {
        Err(anyhow!(
            "Move.toml not found in {}",
            module_package_dir.display()
        ))
    }
}

#[derive(Debug, Default)]
pub struct NamedAddreses(BTreeMap<String, AccountAddress>);
//TODO(tacogip) use HexAddress
impl TryFrom<Vec<(String, String)>> for NamedAddreses {
    type Error = Error;
    fn try_from(values: Vec<(String, String)>) -> Result<Self> {
        let mut named_addresses = BTreeMap::new();
        for (name, address) in values.into_iter() {
            let address = AccountAddress::from_hex_literal(&address)?;
            named_addresses.insert(name, address);
        }
        Ok(NamedAddreses(named_addresses))
    }
}

impl NamedAddreses {
    fn as_inner(&self) -> BTreeMap<String, AccountAddress> {
        self.0.clone()
    }
}

fn compact_build_option(named_addresses: Option<NamedAddreses>) -> BuildOptions {
    BuildOptions {
        dev: false,
        with_srcs: false,
        with_docs: false,
        with_abis: false,
        language_version: Some(LanguageVersion::V1),
        compiler_version: Some(CompilerVersion::V1),
        docgen_options: None,
        with_source_maps: false,
        // Always enable error map bytecode injection
        with_error_map: true,
        named_addresses: named_addresses.unwrap_or_default().as_inner(),
        install_dir: Option::None,
        skip_fetch_latest_git_deps: false,
        bytecode_version: Some(BYTECODE_VERSION),

        override_std: None,
        known_attributes: BTreeSet::new(),
        skip_attribute_checks: true,
        check_test_code: false,
        experiments: vec![],
        move_2: false,
    }
}

fn fat_build_option(named_addresses: Option<NamedAddreses>) -> BuildOptions {
    BuildOptions {
        dev: false,
        with_srcs: true,
        with_abis: true,
        with_docs: true,
        language_version: Some(LanguageVersion::V1),
        compiler_version: Some(CompilerVersion::V1),
        docgen_options: None,
        with_source_maps: true,
        // Always enable error map bytecode injection
        with_error_map: true,
        named_addresses: named_addresses.unwrap_or_default().as_inner(),
        install_dir: Option::None,
        skip_fetch_latest_git_deps: false,
        bytecode_version: Some(BYTECODE_VERSION),
        override_std: None,
        known_attributes: BTreeSet::new(),
        skip_attribute_checks: true,
        check_test_code: false,
        experiments: vec![],
        move_2: false,
    }
}
