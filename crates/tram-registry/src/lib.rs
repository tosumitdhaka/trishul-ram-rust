// SPDX-License-Identifier: Apache-2.0
//! Trusted, statically compiled native plugin contract registry.
//! No dynamic ABI, WASM, runtime plugin loading, or execution is provided.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

pub const CONTRACT_VERSION: u32 = 1;
/// Frozen P1 decoded-record ceiling per source unit (resource-budgets.md).
/// Capability is an advertised maximum; admitting >=4096 does not raise the plan cap.
pub const P1_REQUIRED_BATCH_CAPACITY: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Role {
    Source,
    Sink,
    Serializer,
    Transform,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigType {
    String,
    Boolean,
    Integer,
    Mapping,
    Sequence,
    Null,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcknowledgementTier {
    None,
    LocalEphemeral,
    DurableConfirmed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Constraints {
    pub manual_batch: bool,
    pub streaming: bool,
    pub max_batch: usize,
    pub can_cancel: bool,
    pub acknowledges_source: bool,
    pub confirmation: AcknowledgementTier,
    pub needs_network: bool,
}
impl Constraints {
    pub fn p1_ephemeral() -> Self {
        Self {
            manual_batch: true,
            streaming: false,
            max_batch: P1_REQUIRED_BATCH_CAPACITY,
            can_cancel: true,
            acknowledges_source: false,
            confirmation: AcknowledgementTier::LocalEphemeral,
            needs_network: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub role: Role,
    pub name: String,
    pub version: String,
    pub engine_contract: u32,
    pub operations: BTreeSet<String>,
    pub allowed_config: BTreeMap<String, ConfigType>,
    pub required_config: BTreeSet<String>,
    pub constraints: Constraints,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    InvalidManifest,
    DuplicateIdentity {
        role: Role,
        name: String,
    },
    UnknownPlugin {
        role: Role,
        name: String,
    },
    UnsupportedOperation {
        role: Role,
        name: String,
        operation: String,
    },
    UnknownConfiguration(String),
    MissingConfiguration(String),
    WrongConfigurationType(String),
    IncompatibleEngineContract,
}
impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for RegistryError {}

impl Manifest {
    pub fn validate(&self) -> Result<(), RegistryError> {
        let semver: Vec<_> = self.version.split('.').collect();
        if self.name.is_empty()
            || !self
                .name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            || semver.len() != 3
            || semver
                .iter()
                .any(|s| s.is_empty() || s.parse::<u32>().is_err())
            || self.operations.is_empty()
            || self.constraints.max_batch == 0
            || self.engine_contract != CONTRACT_VERSION
            || !self
                .required_config
                .iter()
                .all(|k| self.allowed_config.contains_key(k))
        {
            return Err(RegistryError::InvalidManifest);
        }
        if self.constraints.needs_network
            && self.constraints.confirmation == AcknowledgementTier::LocalEphemeral
        {
            return Err(RegistryError::InvalidManifest);
        }
        Ok(())
    }
    pub fn validate_config(
        &self,
        supplied: &BTreeMap<String, ConfigType>,
    ) -> Result<(), RegistryError> {
        for (key, actual) in supplied {
            let expected = self
                .allowed_config
                .get(key)
                .ok_or_else(|| RegistryError::UnknownConfiguration(key.clone()))?;
            if actual != expected {
                return Err(RegistryError::WrongConfigurationType(key.clone()));
            }
        }
        for key in &self.required_config {
            if !supplied.contains_key(key) {
                return Err(RegistryError::MissingConfiguration(key.clone()));
            }
        }
        Ok(())
    }
}

/// Implemented only by code linked into this Rust binary at compile time.
/// Its contract is data; it does not create or run a connector in Round 1.
pub trait NativePluginContract {
    fn manifest() -> Manifest;
}
/// Compile-time/native registration is explicit and no name fallback is allowed.
#[derive(Default, Debug)]
pub struct Registry {
    entries: BTreeMap<(Role, String), Manifest>,
}
impl Registry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register_native<P: NativePluginContract>(&mut self) -> Result<(), RegistryError> {
        self.register(P::manifest())
    }
    pub fn register(&mut self, manifest: Manifest) -> Result<(), RegistryError> {
        manifest.validate()?;
        let key = (manifest.role, manifest.name.clone());
        if self.entries.contains_key(&key) {
            return Err(RegistryError::DuplicateIdentity {
                role: manifest.role,
                name: manifest.name,
            });
        }
        self.entries.insert(key, manifest);
        Ok(())
    }
    pub fn select(
        &self,
        role: Role,
        name: &str,
        operation: &str,
    ) -> Result<&Manifest, RegistryError> {
        let manifest = self.entries.get(&(role, name.to_owned())).ok_or_else(|| {
            RegistryError::UnknownPlugin {
                role,
                name: name.to_owned(),
            }
        })?;
        if !manifest.operations.contains(operation) {
            return Err(RegistryError::UnsupportedOperation {
                role,
                name: name.to_owned(),
                operation: operation.to_owned(),
            });
        }
        if !manifest.constraints.manual_batch
            || !manifest.constraints.can_cancel
            || manifest.constraints.max_batch < P1_REQUIRED_BATCH_CAPACITY
            || manifest.constraints.streaming
            || manifest.constraints.acknowledges_source
            || manifest.constraints.needs_network
            || manifest.constraints.confirmation == AcknowledgementTier::DurableConfirmed
        {
            return Err(RegistryError::UnsupportedOperation {
                role,
                name: name.to_owned(),
                operation: operation.to_owned(),
            });
        }
        Ok(manifest)
    }
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

fn builtin(
    role: Role,
    name: &str,
    operation: &str,
    keys: &[(&str, ConfigType)],
    required: &[&str],
) -> Manifest {
    Manifest {
        role,
        name: name.into(),
        version: "0.1.0".into(),
        engine_contract: CONTRACT_VERSION,
        operations: BTreeSet::from([operation.into()]),
        allowed_config: keys.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
        required_config: required.iter().map(|k| (*k).into()).collect(),
        constraints: Constraints::p1_ephemeral(),
    }
}

/// P1 data-only built-in capability declarations, not production plugins.
/// No source/sink/serializer runtime instance can be made from this registry.
pub fn p1_contracts() -> Registry {
    use ConfigType::{Boolean as B, Mapping as M, Sequence as Q, String as S};
    let mut registry = Registry::new();
    let manifests = [
        builtin(
            Role::Source,
            "local",
            "manual-readonly",
            &[
                ("type", S),
                ("path", S),
                ("file_pattern", S),
                ("recursive", B),
            ],
            &["type", "path"],
        ),
        builtin(
            Role::Sink,
            "local",
            "scratch-single",
            &[
                ("type", S),
                ("path", S),
                ("file_mode", S),
                ("filename_template", S),
                ("overwrite", B),
                ("condition", S),
                ("transforms", Q),
            ],
            &[
                "type",
                "path",
                "file_mode",
                "filename_template",
                "overwrite",
            ],
        ),
        builtin(
            Role::Serializer,
            "json",
            "decode",
            &[("type", S)],
            &["type"],
        ),
        builtin(
            Role::Transform,
            "rename",
            "stateless",
            &[("type", S), ("fields", M)],
            &["type", "fields"],
        ),
        builtin(
            Role::Transform,
            "add_field",
            "stateless",
            &[("type", S), ("fields", M)],
            &["type", "fields"],
        ),
        builtin(
            Role::Transform,
            "filter",
            "stateless",
            &[("type", S), ("condition", S)],
            &["type", "condition"],
        ),
        builtin(
            Role::Transform,
            "drop",
            "stateless",
            &[("type", S), ("fields", Q)],
            &["type", "fields"],
        ),
    ];
    for mut entry in manifests {
        if entry.role == Role::Serializer {
            entry.operations.insert("encode".into());
        }
        registry
            .register(entry)
            .expect("valid compiled-in manifest");
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake;
    impl NativePluginContract for Fake {
        fn manifest() -> Manifest {
            builtin(
                Role::Source,
                "fake",
                "read",
                &[("type", ConfigType::String)],
                &["type"],
            )
        }
    }
    #[test]
    fn static_registration_and_duplicate_identity() {
        let mut reg = Registry::new();
        reg.register_native::<Fake>().expect("native entry");
        assert_eq!(reg.count(), 1);
        assert_eq!(
            reg.register_native::<Fake>(),
            Err(RegistryError::DuplicateIdentity {
                role: Role::Source,
                name: "fake".into()
            })
        );
    }
    #[test]
    fn exact_selection_rejects_missing_and_incompatible_operations() {
        let reg = p1_contracts();
        assert!(matches!(
            reg.select(Role::Source, "nope", "manual-readonly"),
            Err(RegistryError::UnknownPlugin { .. })
        ));
        assert!(matches!(
            reg.select(Role::Source, "local", "write"),
            Err(RegistryError::UnsupportedOperation { .. })
        ));
        assert!(reg.select(Role::Source, "local", "manual-readonly").is_ok());
        assert!(matches!(
            reg.select(Role::Sink, "local", "manual-readonly"),
            Err(RegistryError::UnsupportedOperation { .. })
        ));
    }
    #[test]
    fn exact_schema_rejects_unknown_and_wrong_types() {
        let reg = p1_contracts();
        let src = reg
            .select(Role::Source, "local", "manual-readonly")
            .expect("local");
        let mut config = BTreeMap::from([
            ("type".into(), ConfigType::String),
            ("path".into(), ConfigType::String),
        ]);
        assert!(src.validate_config(&config).is_ok());
        config.insert("random".into(), ConfigType::Boolean);
        assert_eq!(
            src.validate_config(&config),
            Err(RegistryError::UnknownConfiguration("random".into()))
        );
        config.remove("random");
        config.insert("path".into(), ConfigType::Integer);
        assert_eq!(
            src.validate_config(&config),
            Err(RegistryError::WrongConfigurationType("path".into()))
        );
        config.remove("path");
        assert_eq!(
            src.validate_config(&config),
            Err(RegistryError::MissingConfiguration("path".into()))
        );
    }
    #[test]
    fn json_exact_name_supports_decode_and_encode_but_not_other_operations() {
        let registry = p1_contracts();
        assert!(registry.select(Role::Serializer, "json", "decode").is_ok());
        assert!(registry.select(Role::Serializer, "json", "encode").is_ok());
        assert!(matches!(
            registry.select(Role::Serializer, "json", "stream"),
            Err(RegistryError::UnsupportedOperation { .. })
        ));
        assert!(matches!(
            registry.select(Role::Serializer, "json_encoder", "encode"),
            Err(RegistryError::UnknownPlugin { .. })
        ));
    }
    #[test]
    fn p1_batch_capacity_and_cancellation_are_required_for_injected_manifests() {
        let mut candidate = Fake::manifest();
        assert_eq!(candidate.constraints.max_batch, P1_REQUIRED_BATCH_CAPACITY);
        let mut valid = Registry::new();
        valid
            .register(candidate.clone())
            .expect("valid P1 manifest");
        assert!(valid.select(Role::Source, "fake", "read").is_ok());
        assert!(matches!(
            valid.select(Role::Source, "fake", "wrong"),
            Err(RegistryError::UnsupportedOperation { .. })
        ));

        candidate.constraints.can_cancel = false;
        let mut no_cancel = Registry::new();
        no_cancel
            .register(candidate.clone())
            .expect("schema legal but non-P1");
        assert!(matches!(
            no_cancel.select(Role::Source, "fake", "read"),
            Err(RegistryError::UnsupportedOperation { .. })
        ));
        candidate.constraints.can_cancel = true;

        candidate.constraints.max_batch = P1_REQUIRED_BATCH_CAPACITY - 1;
        let mut too_small = Registry::new();
        too_small
            .register(candidate.clone())
            .expect("nonzero declared capacity");
        assert!(matches!(
            too_small.select(Role::Source, "fake", "read"),
            Err(RegistryError::UnsupportedOperation { .. })
        ));
        candidate.constraints.max_batch = 0;
        assert_eq!(candidate.validate(), Err(RegistryError::InvalidManifest));
        assert_eq!(
            Registry::new().register(candidate.clone()),
            Err(RegistryError::InvalidManifest)
        );
        candidate.constraints.max_batch = P1_REQUIRED_BATCH_CAPACITY;
        let mut at_boundary = Registry::new();
        at_boundary.register(candidate).expect("valid boundary");
        assert!(at_boundary.select(Role::Source, "fake", "read").is_ok());
    }

    #[test]
    fn unsupported_effectful_plugin_is_rejected() {
        let mut reg = Registry::new();
        let mut external = Fake::manifest();
        external.name = "remote".into();
        external.constraints.needs_network = true;
        external.constraints.confirmation = AcknowledgementTier::None;
        reg.register(external).expect("manifest shape");
        assert!(matches!(
            reg.select(Role::Source, "remote", "read"),
            Err(RegistryError::UnsupportedOperation { .. })
        ));
    }
}
