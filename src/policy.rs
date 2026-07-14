use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{Action, DataClass};

pub const MAX_RESTORATION_TTL_SECONDS: u64 = 30 * 60;
pub const DEFAULT_RESTORATION_TTL_SECONDS: u64 = 15 * 60;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u8,
    pub name: String,
    pub description: String,
    pub defaults: PolicyDefaults,
    pub categories: BTreeMap<String, CategoryRule>,
    #[serde(default)]
    pub detectors: DetectorSettings,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyDefaults {
    pub unknown_sensitive: Action,
    pub raw_logging: bool,
    pub restoration: RestorationDefault,
    pub max_request_bytes: usize,
    pub ledger_max_entries: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RestorationDefault {
    Deny,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryRule {
    pub action: Action,
    #[serde(default)]
    pub restorable: bool,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DetectorSettings {
    pub internal_hostname_suffixes: Vec<String>,
    pub custom_terms: Vec<String>,
    pub home_prefixes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPolicy {
    inner: Policy,
    hash: String,
}

impl ValidatedPolicy {
    pub fn from_yaml(bytes: &[u8]) -> Result<Self, PolicyError> {
        let policy: Policy = serde_yaml::from_slice(bytes).map_err(PolicyError::Parse)?;
        Self::validate(policy)
    }

    pub fn validate(policy: Policy) -> Result<Self, PolicyError> {
        if policy.version != 1 {
            return Err(PolicyError::UnsupportedVersion(policy.version));
        }
        if !is_safe_slug(&policy.name) {
            return Err(PolicyError::UnsafePolicyName);
        }
        if policy.defaults.raw_logging {
            return Err(PolicyError::RawLoggingForbidden);
        }
        if policy.defaults.unknown_sensitive != Action::Block {
            return Err(PolicyError::UnknownSensitiveMustBlock);
        }
        if !(1_024..=8 * 1_024 * 1_024).contains(&policy.defaults.max_request_bytes) {
            return Err(PolicyError::InvalidMaxRequestBytes);
        }
        if !(1..=16_384).contains(&policy.defaults.ledger_max_entries) {
            return Err(PolicyError::InvalidLedgerCapacity);
        }

        for key in policy.categories.keys() {
            if !KNOWN_CATEGORY_KEYS.contains(&key.as_str()) {
                return Err(PolicyError::UnknownCategory(key.clone()));
            }
        }

        for data_class in ALL_DATA_CLASSES {
            let key = data_class.policy_key();
            let rule = policy
                .categories
                .get(key)
                .ok_or_else(|| PolicyError::MissingCategory(key.to_string()))?;
            validate_rule(data_class, rule)?;
        }

        validate_detector_settings(&policy.detectors)?;
        let canonical = serde_json::to_vec(&policy).map_err(PolicyError::Canonicalize)?;
        let hash = blake3::hash(&canonical).to_hex().to_string();
        Ok(Self {
            inner: policy,
            hash,
        })
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn defaults(&self) -> &PolicyDefaults {
        &self.inner.defaults
    }

    pub fn detectors(&self) -> &DetectorSettings {
        &self.inner.detectors
    }

    pub fn rule_for(&self, data_class: DataClass) -> &CategoryRule {
        // Validation proves every key is present. Indexing avoids a policy
        // fallback that could silently weaken enforcement.
        &self.inner.categories[data_class.policy_key()]
    }
}

const ALL_DATA_CLASSES: [DataClass; 11] = [
    DataClass::Credentials,
    DataClass::PrivateKey,
    DataClass::Jwt,
    DataClass::DatabaseCredential,
    DataClass::Email,
    DataClass::Phone,
    DataClass::PrivateIpv4,
    DataClass::InternalHostname,
    DataClass::HomePath,
    DataClass::CustomTerm,
    DataClass::TokenNamespace,
];

const KNOWN_CATEGORY_KEYS: [&str; 11] = [
    "credentials",
    "private_keys",
    "security.jwt",
    "network.database_url",
    "pii.email",
    "pii.phone",
    "infrastructure.private_ip",
    "infrastructure.hostname",
    "filesystem.home_path",
    "custom.term",
    "agentveil.token_namespace",
];

fn validate_rule(data_class: DataClass, rule: &CategoryRule) -> Result<(), PolicyError> {
    if data_class.hard_block() && rule.action != Action::Block {
        return Err(PolicyError::HardBlockInvariant(data_class));
    }
    if data_class.hard_block() && rule.restorable {
        return Err(PolicyError::HardBlockRestoration(data_class));
    }
    if rule.restorable && rule.action != Action::Tokenize {
        return Err(PolicyError::RestorationRequiresTokenize(data_class));
    }
    if rule.action == Action::Tokenize && !rule.restorable {
        return Err(PolicyError::TokenizationRequiresRestoration(data_class));
    }
    if rule.action != Action::Tokenize && rule.ttl_seconds.is_some() {
        return Err(PolicyError::TtlRequiresTokenize(data_class));
    }
    if rule.action == Action::Tokenize {
        let ttl = rule.ttl_seconds.unwrap_or(DEFAULT_RESTORATION_TTL_SECONDS);
        if ttl == 0 || ttl > MAX_RESTORATION_TTL_SECONDS {
            return Err(PolicyError::InvalidTtl(data_class));
        }
    }
    Ok(())
}

fn validate_detector_settings(settings: &DetectorSettings) -> Result<(), PolicyError> {
    if settings.internal_hostname_suffixes.len() > 64
        || settings.custom_terms.len() > 128
        || settings.home_prefixes.len() > 16
    {
        return Err(PolicyError::TooManyDetectorTerms);
    }
    for term in settings
        .internal_hostname_suffixes
        .iter()
        .chain(settings.custom_terms.iter())
        .chain(settings.home_prefixes.iter())
    {
        if term.len() < 3 || term.len() > 128 || term.chars().any(char::is_control) {
            return Err(PolicyError::UnsafeDetectorTerm);
        }
    }
    Ok(())
}

fn is_safe_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("policy YAML is invalid")]
    Parse(#[source] serde_yaml::Error),
    #[error("policy cannot be canonicalized")]
    Canonicalize(#[source] serde_json::Error),
    #[error("unsupported policy version {0}")]
    UnsupportedVersion(u8),
    #[error("policy name must be a short lowercase slug")]
    UnsafePolicyName,
    #[error("raw logging is forbidden")]
    RawLoggingForbidden,
    #[error("unknown sensitive data must block")]
    UnknownSensitiveMustBlock,
    #[error("request-size limit is outside the supported bounds")]
    InvalidMaxRequestBytes,
    #[error("ledger capacity is outside the supported bounds")]
    InvalidLedgerCapacity,
    #[error("unknown category {0}")]
    UnknownCategory(String),
    #[error("missing category {0}")]
    MissingCategory(String),
    #[error("{0} must always block")]
    HardBlockInvariant(DataClass),
    #[error("{0} can never be restorable")]
    HardBlockRestoration(DataClass),
    #[error("restoration for {0} requires tokenization")]
    RestorationRequiresTokenize(DataClass),
    #[error("tokenization for {0} requires explicit restoration in this version")]
    TokenizationRequiresRestoration(DataClass),
    #[error("TTL for {0} requires tokenization")]
    TtlRequiresTokenize(DataClass),
    #[error("TTL for {0} is outside the supported bounds")]
    InvalidTtl(DataClass),
    #[error("too many configured detector terms")]
    TooManyDetectorTerms,
    #[error("configured detector term is unsafe")]
    UnsafeDetectorTerm,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const DEFAULT: &[u8] = include_bytes!("../policies/default.yaml");

    #[test]
    fn default_policy_is_valid_and_stable() {
        let policy = ValidatedPolicy::from_yaml(DEFAULT).expect("default policy should validate");
        assert_eq!(policy.name(), "default");
        assert_eq!(policy.hash().len(), 64);
        assert_eq!(
            policy.rule_for(DataClass::Credentials).action,
            Action::Block
        );
    }

    #[test]
    fn s0_cannot_be_tokenized_or_restored() {
        let mut policy: Policy = serde_yaml::from_slice(DEFAULT).expect("fixture should parse");
        let rule = policy
            .categories
            .get_mut("credentials")
            .expect("fixture category should exist");
        rule.action = Action::Tokenize;
        rule.restorable = true;
        rule.ttl_seconds = Some(30);
        assert!(matches!(
            ValidatedPolicy::validate(policy),
            Err(PolicyError::HardBlockInvariant(DataClass::Credentials))
        ));
    }

    #[test]
    fn raw_logging_cannot_be_enabled() {
        let mut policy: Policy = serde_yaml::from_slice(DEFAULT).expect("fixture should parse");
        policy.defaults.raw_logging = true;
        assert!(matches!(
            ValidatedPolicy::validate(policy),
            Err(PolicyError::RawLoggingForbidden)
        ));
    }
}
