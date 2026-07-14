use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Credentials,
    PrivateKey,
    Jwt,
    DatabaseCredential,
    Email,
    Phone,
    PrivateIpv4,
    InternalHostname,
    HomePath,
    CustomTerm,
    TokenNamespace,
}

impl DataClass {
    pub const fn policy_key(self) -> &'static str {
        match self {
            Self::Credentials => "credentials",
            Self::PrivateKey => "private_keys",
            Self::Jwt => "security.jwt",
            Self::DatabaseCredential => "network.database_url",
            Self::Email => "pii.email",
            Self::Phone => "pii.phone",
            Self::PrivateIpv4 => "infrastructure.private_ip",
            Self::InternalHostname => "infrastructure.hostname",
            Self::HomePath => "filesystem.home_path",
            Self::CustomTerm => "custom.term",
            Self::TokenNamespace => "agentveil.token_namespace",
        }
    }

    pub const fn token_label(self) -> &'static str {
        match self {
            Self::Email => "EMAIL",
            Self::Phone => "PHONE",
            Self::PrivateIpv4 => "IPV4",
            Self::InternalHostname => "HOST",
            Self::HomePath => "PATH",
            Self::CustomTerm => "TERM",
            Self::Credentials
            | Self::PrivateKey
            | Self::Jwt
            | Self::DatabaseCredential
            | Self::TokenNamespace => "BLOCKED",
        }
    }

    pub const fn hard_block(self) -> bool {
        matches!(
            self,
            Self::Credentials
                | Self::PrivateKey
                | Self::Jwt
                | Self::DatabaseCredential
                | Self::TokenNamespace
        )
    }
}

impl fmt::Display for DataClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.policy_key())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Block,
    Mask,
    Tokenize,
    Allow,
}

impl Action {
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Block => 4,
            Self::Mask => 3,
            Self::Tokenize => 2,
            Self::Allow => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectorId {
    AgentVeilTokenNamespace,
    OpenAiApiKey,
    GithubToken,
    AwsAccessKeyId,
    AwsSecretAssignment,
    GoogleApiKey,
    StripeSecretKey,
    SlackToken,
    PemPrivateKey,
    CredentialedDatabaseUrl,
    BearerCredential,
    Jwt,
    EnvCredentialAssignment,
    Email,
    Phone,
    PrivateIpv4,
    InternalHostname,
    HomePath,
    CustomTerm,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Contextual,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldClass {
    Instructions,
    MessageText,
    AgentMessageText,
    FunctionArguments,
    FunctionOutput,
    CustomToolInput,
    CustomToolOutput,
    ToolDescription,
    ToolSchemaText,
    ReasoningSummary,
    ClientMetadata,
    StructuralMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub data_class: DataClass,
    pub detector: DetectorId,
    pub confidence: Confidence,
    pub source_span: Range<usize>,
}

impl Finding {
    pub fn overlaps(&self, other: &Self) -> bool {
        self.source_span.start < other.source_span.end
            && other.source_span.start < self.source_span.end
    }
}
