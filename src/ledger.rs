use std::collections::HashMap;
use std::fmt;

use thiserror::Error;
use zeroize::Zeroizing;

use crate::domain::DataClass;

const TOKEN_RANDOM_BYTES: usize = 16;
const MAX_TOKEN_ATTEMPTS: usize = 8;

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct SessionScope(String);

impl SessionScope {
    pub fn parse(value: impl Into<String>) -> Result<Self, LedgerError> {
        let value = value.into();
        if !(12..=64).contains(&value.len())
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(LedgerError::InvalidSessionScope);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionScope([redacted])")
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Token(String);

impl Token {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Token([redacted])")
    }
}

pub struct IssuedToken {
    pub token: Token,
    pub created: bool,
}

pub trait RandomSource {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), LedgerError>;
}

#[derive(Default)]
pub struct OsRandom;

impl RandomSource for OsRandom {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), LedgerError> {
        getrandom::fill(destination).map_err(|_| LedgerError::RandomUnavailable)
    }
}

struct Entry {
    scope: SessionScope,
    data_class: DataClass,
    value: Zeroizing<String>,
    expires_at_ms: u64,
    last_used_sequence: u64,
}

pub struct TokenLedger<R = OsRandom> {
    entries: HashMap<String, Entry>,
    max_entries: usize,
    sequence: u64,
    random: R,
}

impl TokenLedger<OsRandom> {
    pub fn new(max_entries: usize) -> Result<Self, LedgerError> {
        Self::with_random(max_entries, OsRandom)
    }
}

impl<R> TokenLedger<R>
where
    R: RandomSource,
{
    pub fn with_random(max_entries: usize, random: R) -> Result<Self, LedgerError> {
        if max_entries == 0 || max_entries > 16_384 {
            return Err(LedgerError::InvalidCapacity);
        }
        Ok(Self {
            entries: HashMap::new(),
            max_entries,
            sequence: 0,
            random,
        })
    }

    pub fn issue(
        &mut self,
        scope: &SessionScope,
        data_class: DataClass,
        value: &str,
        now_ms: u64,
        ttl_seconds: u64,
    ) -> Result<Token, LedgerError> {
        self.issue_tracked(scope, data_class, value, now_ms, ttl_seconds)
            .map(|issued| issued.token)
    }

    pub fn issue_tracked(
        &mut self,
        scope: &SessionScope,
        data_class: DataClass,
        value: &str,
        now_ms: u64,
        ttl_seconds: u64,
    ) -> Result<IssuedToken, LedgerError> {
        if value.is_empty() {
            return Err(LedgerError::EmptyValue);
        }
        if data_class.hard_block() {
            return Err(LedgerError::NonRestorableClass(data_class));
        }
        let ttl_ms = ttl_seconds
            .checked_mul(1_000)
            .ok_or(LedgerError::ExpiryOverflow)?;
        let expires_at_ms = now_ms
            .checked_add(ttl_ms)
            .ok_or(LedgerError::ExpiryOverflow)?;
        self.purge_expired(now_ms);

        self.sequence = self.sequence.saturating_add(1);
        let sequence = self.sequence;
        if let Some((token, entry)) = self.entries.iter_mut().find(|(_, entry)| {
            entry.scope == *scope
                && entry.data_class == data_class
                && entry.value.as_str() == value
                && entry.expires_at_ms > now_ms
        }) {
            entry.last_used_sequence = sequence;
            entry.expires_at_ms = expires_at_ms;
            return Ok(IssuedToken {
                token: Token(token.clone()),
                created: false,
            });
        }

        if self.entries.len() >= self.max_entries {
            self.evict_lru();
        }

        for _ in 0..MAX_TOKEN_ATTEMPTS {
            let token = self.generate_token(data_class)?;
            if self.entries.contains_key(token.as_str()) {
                continue;
            }
            self.entries.insert(
                token.as_str().to_string(),
                Entry {
                    scope: scope.clone(),
                    data_class,
                    value: Zeroizing::new(value.to_string()),
                    expires_at_ms,
                    last_used_sequence: sequence,
                },
            );
            return Ok(IssuedToken {
                token,
                created: true,
            });
        }
        Err(LedgerError::TokenCollision)
    }

    pub fn rollback_created(&mut self, scope: &SessionScope, tokens: &[Token]) {
        for token in tokens {
            if self
                .entries
                .get(token.as_str())
                .is_some_and(|entry| entry.scope == *scope)
            {
                self.entries.remove(token.as_str());
            }
        }
    }

    pub fn restore_exact(
        &mut self,
        scope: &SessionScope,
        token: &str,
        now_ms: u64,
    ) -> Option<Zeroizing<String>> {
        self.purge_expired(now_ms);
        self.sequence = self.sequence.saturating_add(1);
        let sequence = self.sequence;
        let entry = self.entries.get_mut(token)?;
        if entry.scope != *scope || entry.expires_at_ms <= now_ms {
            return None;
        }
        entry.last_used_sequence = sequence;
        Some(Zeroizing::new(entry.value.to_string()))
    }

    pub fn owns_token(&mut self, scope: &SessionScope, token: &str, now_ms: u64) -> bool {
        self.purge_expired(now_ms);
        self.entries
            .get(token)
            .is_some_and(|entry| entry.scope == *scope && entry.expires_at_ms > now_ms)
    }

    pub fn clear_scope(&mut self, scope: &SessionScope) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, entry| entry.scope != *scope);
        before - self.entries.len()
    }

    pub fn clear_all(&mut self) -> usize {
        let count = self.entries.len();
        self.entries.clear();
        count
    }

    pub fn entry_count(&mut self, now_ms: u64) -> usize {
        self.purge_expired(now_ms);
        self.entries.len()
    }

    fn generate_token(&mut self, data_class: DataClass) -> Result<Token, LedgerError> {
        let mut random = [0_u8; TOKEN_RANDOM_BYTES];
        self.random.fill(&mut random)?;
        let mut encoded = String::with_capacity(TOKEN_RANDOM_BYTES * 2);
        for byte in random {
            use std::fmt::Write as _;
            write!(&mut encoded, "{byte:02X}").map_err(|_| LedgerError::TokenEncoding)?;
        }
        Ok(Token(format!(
            "[AV_{}_{encoded}]",
            data_class.token_label()
        )))
    }

    fn purge_expired(&mut self, now_ms: u64) {
        self.entries.retain(|_, entry| entry.expires_at_ms > now_ms);
    }

    fn evict_lru(&mut self) {
        let victim = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used_sequence)
            .map(|(token, _)| token.clone());
        if let Some(victim) = victim {
            self.entries.remove(&victim);
        }
    }
}

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("session scope is invalid")]
    InvalidSessionScope,
    #[error("ledger capacity is invalid")]
    InvalidCapacity,
    #[error("protected value cannot be empty")]
    EmptyValue,
    #[error("{0} can never enter the token ledger")]
    NonRestorableClass(DataClass),
    #[error("secure random generation is unavailable")]
    RandomUnavailable,
    #[error("token encoding failed")]
    TokenEncoding,
    #[error("token generation repeatedly collided")]
    TokenCollision,
    #[error("token expiry overflowed")]
    ExpiryOverflow,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    struct CounterRandom(u8);

    impl RandomSource for CounterRandom {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), LedgerError> {
            self.0 = self.0.wrapping_add(1);
            destination.fill(self.0);
            Ok(())
        }
    }

    fn scope(name: &str) -> SessionScope {
        SessionScope::parse(name).expect("scope fixture should be valid")
    }

    #[test]
    fn token_is_stable_within_scope_and_distinct_across_scopes() {
        let mut ledger =
            TokenLedger::with_random(8, CounterRandom(0)).expect("ledger fixture should be valid");
        let a = scope("scope_A_123456");
        let b = scope("scope_B_123456");
        let token_a1 = ledger
            .issue(&a, DataClass::Email, "fixture@example.test", 0, 60)
            .expect("token should issue");
        let token_a2 = ledger
            .issue(&a, DataClass::Email, "fixture@example.test", 10, 60)
            .expect("token should reuse");
        let token_b = ledger
            .issue(&b, DataClass::Email, "fixture@example.test", 10, 60)
            .expect("token should issue in second scope");
        assert_eq!(token_a1, token_a2);
        assert_ne!(token_a1, token_b);
    }

    #[test]
    fn restoration_is_scope_and_ttl_bound() {
        let mut ledger =
            TokenLedger::with_random(8, CounterRandom(0)).expect("ledger fixture should be valid");
        let a = scope("scope_A_123456");
        let b = scope("scope_B_123456");
        let token = ledger
            .issue(&a, DataClass::Email, "fixture@example.test", 0, 1)
            .expect("token should issue");
        assert!(ledger.restore_exact(&b, token.as_str(), 999).is_none());
        let restored = ledger.restore_exact(&a, token.as_str(), 999);
        assert_eq!(
            restored.as_ref().map(|value| value.as_str()),
            Some("fixture@example.test")
        );
        assert!(ledger.restore_exact(&a, token.as_str(), 1_000).is_none());
    }

    #[test]
    fn capacity_uses_lru_and_clear_scope_isolated() {
        let mut ledger =
            TokenLedger::with_random(2, CounterRandom(0)).expect("ledger fixture should be valid");
        let a = scope("scope_A_123456");
        let b = scope("scope_B_123456");
        let first = ledger
            .issue(&a, DataClass::Email, "one@example.test", 0, 60)
            .expect("first token should issue");
        let second = ledger
            .issue(&a, DataClass::Email, "two@example.test", 0, 60)
            .expect("second token should issue");
        assert!(ledger.restore_exact(&a, first.as_str(), 1).is_some());
        let third = ledger
            .issue(&b, DataClass::Email, "three@example.test", 2, 60)
            .expect("third token should issue");
        assert!(ledger.restore_exact(&a, second.as_str(), 3).is_none());
        assert!(ledger.restore_exact(&a, first.as_str(), 3).is_some());
        assert!(ledger.restore_exact(&b, third.as_str(), 3).is_some());
        assert_eq!(ledger.clear_scope(&a), 1);
        assert!(ledger.restore_exact(&b, third.as_str(), 4).is_some());
    }

    #[test]
    fn hard_secrets_never_enter_ledger() {
        let mut ledger =
            TokenLedger::with_random(8, CounterRandom(0)).expect("ledger fixture should be valid");
        let result = ledger.issue(
            &scope("scope_A_123456"),
            DataClass::Credentials,
            "synthetic",
            0,
            60,
        );
        assert!(matches!(
            result,
            Err(LedgerError::NonRestorableClass(DataClass::Credentials))
        ));
    }
}
