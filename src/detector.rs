use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::ops::Range;

use regex::Regex;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

use crate::domain::{Confidence, DataClass, DetectorId, Finding};
use crate::policy::DetectorSettings;

pub struct Scanner {
    patterns: Vec<PatternDetector>,
    email: Regex,
    private_ipv4: Regex,
    phone: Regex,
    env_assignment: Regex,
    internal_hostname_suffixes: Vec<String>,
    custom_terms: Vec<String>,
    home_prefixes: Vec<String>,
}

struct PatternDetector {
    regex: Regex,
    data_class: DataClass,
    detector: DetectorId,
    confidence: Confidence,
    secret_only: bool,
}

impl Scanner {
    pub fn new(settings: &DetectorSettings) -> Result<Self, DetectorError> {
        let specifications = [
            (
                r"\[AV_(?:EMAIL|PHONE|IPV4|HOST|PATH|TERM)_[A-F0-9]{32}\]",
                DataClass::TokenNamespace,
                DetectorId::AgentVeilTokenNamespace,
                Confidence::High,
                false,
            ),
            (
                r"(?-u:\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{16,}\b)",
                DataClass::Credentials,
                DetectorId::OpenAiApiKey,
                Confidence::High,
                true,
            ),
            (
                r"(?-u:\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})\b)",
                DataClass::Credentials,
                DetectorId::GithubToken,
                Confidence::High,
                true,
            ),
            (
                r"(?-u:\b(?:AKIA|ASIA)[A-Z0-9]{16}\b)",
                DataClass::Credentials,
                DetectorId::AwsAccessKeyId,
                Confidence::High,
                true,
            ),
            (
                r"(?-u:\bAIza[A-Za-z0-9_-]{35}\b)",
                DataClass::Credentials,
                DetectorId::GoogleApiKey,
                Confidence::High,
                true,
            ),
            (
                r"(?-u:\b(?:sk|rk)_(?:test|live)_[A-Za-z0-9]{16,}\b)",
                DataClass::Credentials,
                DetectorId::StripeSecretKey,
                Confidence::High,
                true,
            ),
            (
                r"(?-u:\b(?:xox[bpar]-|xapp-|xwfp-)[A-Za-z0-9-]{16,}\b)",
                DataClass::Credentials,
                DetectorId::SlackToken,
                Confidence::High,
                true,
            ),
            (
                r"(?i:\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis|mssql)://[^/\s:@]+:[^@\s/]+@[^/\s]+)",
                DataClass::DatabaseCredential,
                DetectorId::CredentialedDatabaseUrl,
                Confidence::High,
                true,
            ),
            (
                r"(?i-u:\bbearer[ \t]+[A-Za-z0-9._~+/=-]{20,}\b)",
                DataClass::Credentials,
                DetectorId::BearerCredential,
                Confidence::Contextual,
                true,
            ),
            (
                r"(?-u:\beyJ[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{8,}\b)",
                DataClass::Jwt,
                DetectorId::Jwt,
                Confidence::High,
                true,
            ),
        ];
        let mut patterns = Vec::with_capacity(specifications.len());
        for (pattern, data_class, detector, confidence, secret_only) in specifications {
            patterns.push(PatternDetector {
                regex: Regex::new(pattern).map_err(DetectorError::Pattern)?,
                data_class,
                detector,
                confidence,
                secret_only,
            });
        }

        Ok(Self {
            patterns,
            email: Regex::new(
                r"(?-u:\b[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)+\b)",
            )
            .map_err(DetectorError::Pattern)?,
            private_ipv4: Regex::new(r"(?-u:\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b)")
                .map_err(DetectorError::Pattern)?,
            phone: Regex::new(
                r"(?-u:(?:\+?1[-. ]?)?\(?[2-9][0-9]{2}\)?[-. ][0-9]{3}[-. ][0-9]{4})",
            )
            .map_err(DetectorError::Pattern)?,
            env_assignment: Regex::new(
                r#"(?im)^\s*(?:export\s+)?[A-Z0-9_]*(?:API_KEY|SECRET|PASSWORD|PASSWD|TOKEN|CLIENT_SECRET|PRIVATE_KEY)[A-Z0-9_]*\s*[:=]\s*["']?([^\s#"']{12,})"#,
            )
            .map_err(DetectorError::Pattern)?,
            internal_hostname_suffixes: settings
                .internal_hostname_suffixes
                .iter()
                .map(|value| value.to_ascii_lowercase())
                .collect(),
            custom_terms: settings.custom_terms.clone(),
            home_prefixes: settings.home_prefixes.clone(),
        })
    }

    pub fn scan(&self, source: &str) -> Result<Vec<Finding>, DetectorError> {
        let normalized = ScanView::normalize(source)?;
        let mut views = vec![(normalized.clone(), false)];
        if let Some(decoded) = normalized.decode_json_escapes()? {
            views.push((decoded, false));
        }
        if let Some(decoded) = normalized.decode_percent_encoding()? {
            views.push((decoded, false));
        }
        if let Some(collapsed) = normalized.collapse_candidate_separators()? {
            views.push((collapsed, true));
        }

        let mut findings = Vec::new();
        for (view, secret_view) in &views {
            for pattern in &self.patterns {
                if *secret_view && !pattern.secret_only {
                    continue;
                }
                for candidate in pattern.regex.find_iter(&view.text) {
                    findings.push(view.finding(
                        candidate.range(),
                        pattern.data_class,
                        pattern.detector,
                        pattern.confidence,
                    )?);
                }
            }
            self.scan_private_keys(view, &mut findings)?;
            self.scan_env_assignments(view, &mut findings)?;
            if !secret_view {
                self.scan_email(view, &mut findings)?;
                self.scan_private_ipv4(view, &mut findings)?;
                self.scan_phone(view, &mut findings)?;
                self.scan_configured(view, &mut findings)?;
            }
        }

        findings.sort_by_key(|finding| {
            (
                finding.source_span.start,
                finding.source_span.end,
                finding.detector,
            )
        });
        findings.dedup_by(|left, right| {
            left.source_span == right.source_span
                && left.data_class == right.data_class
                && left.detector == right.detector
        });
        Ok(findings)
    }

    fn scan_private_keys(
        &self,
        view: &ScanView,
        findings: &mut Vec<Finding>,
    ) -> Result<(), DetectorError> {
        const BEGIN: &str = "-----BEGIN ";
        let mut cursor = 0;
        while let Some(relative) = view.text[cursor..].find(BEGIN) {
            let start = cursor + relative;
            let label_start = start + BEGIN.len();
            let Some(label_end_relative) = view.text[label_start..].find("-----") else {
                break;
            };
            let label_end = label_start + label_end_relative;
            let label = &view.text[label_start..label_end];
            if !matches!(
                label,
                "PRIVATE KEY" | "RSA PRIVATE KEY" | "EC PRIVATE KEY" | "OPENSSH PRIVATE KEY"
            ) {
                cursor = label_end + 5;
                continue;
            }
            let end_marker = format!("-----END {label}-----");
            let body_start = label_end + 5;
            let Some(end_relative) = view.text[body_start..].find(&end_marker) else {
                break;
            };
            let end = body_start + end_relative + end_marker.len();
            findings.push(view.finding(
                start..end,
                DataClass::PrivateKey,
                DetectorId::PemPrivateKey,
                Confidence::High,
            )?);
            cursor = end;
        }
        Ok(())
    }

    fn scan_env_assignments(
        &self,
        view: &ScanView,
        findings: &mut Vec<Finding>,
    ) -> Result<(), DetectorError> {
        for captures in self.env_assignment.captures_iter(&view.text) {
            let Some(candidate) = captures.get(1) else {
                continue;
            };
            if is_placeholder(candidate.as_str()) {
                continue;
            }
            let detector = if view.text[captures.get(0).map_or(0, |whole| whole.start())..]
                .to_ascii_uppercase()
                .starts_with("AWS_SECRET_ACCESS_KEY")
            {
                DetectorId::AwsSecretAssignment
            } else {
                DetectorId::EnvCredentialAssignment
            };
            findings.push(view.finding(
                candidate.range(),
                DataClass::Credentials,
                detector,
                Confidence::Contextual,
            )?);
        }
        Ok(())
    }

    fn scan_email(
        &self,
        view: &ScanView,
        findings: &mut Vec<Finding>,
    ) -> Result<(), DetectorError> {
        for candidate in self.email.find_iter(&view.text) {
            findings.push(view.finding(
                candidate.range(),
                DataClass::Email,
                DetectorId::Email,
                Confidence::High,
            )?);
        }
        Ok(())
    }

    fn scan_private_ipv4(
        &self,
        view: &ScanView,
        findings: &mut Vec<Finding>,
    ) -> Result<(), DetectorError> {
        for candidate in self.private_ipv4.find_iter(&view.text) {
            let Ok(address) = candidate.as_str().parse::<Ipv4Addr>() else {
                continue;
            };
            if !(address.is_private() || address.is_loopback() || address.is_link_local()) {
                continue;
            }
            findings.push(view.finding(
                candidate.range(),
                DataClass::PrivateIpv4,
                DetectorId::PrivateIpv4,
                Confidence::High,
            )?);
        }
        Ok(())
    }

    fn scan_phone(
        &self,
        view: &ScanView,
        findings: &mut Vec<Finding>,
    ) -> Result<(), DetectorError> {
        for candidate in self.phone.find_iter(&view.text) {
            let context_start = candidate.start().saturating_sub(32);
            let context = view.text[context_start..candidate.start()].to_ascii_lowercase();
            if !["phone", "mobile", "telephone", "tel", "contact"]
                .iter()
                .any(|label| context.contains(label))
            {
                continue;
            }
            findings.push(view.finding(
                candidate.range(),
                DataClass::Phone,
                DetectorId::Phone,
                Confidence::Contextual,
            )?);
        }
        Ok(())
    }

    fn scan_configured(
        &self,
        view: &ScanView,
        findings: &mut Vec<Finding>,
    ) -> Result<(), DetectorError> {
        let lower = view.text.to_ascii_lowercase();
        for suffix in &self.internal_hostname_suffixes {
            for (start, _) in lower.match_indices(suffix) {
                let hostname_start = view.text[..start]
                    .rfind(|character: char| !is_hostname_character(character))
                    .map_or(0, |index| index + 1);
                let end = start + suffix.len();
                if hostname_start == start || !is_boundary(&view.text, hostname_start, end) {
                    continue;
                }
                findings.push(view.finding(
                    hostname_start..end,
                    DataClass::InternalHostname,
                    DetectorId::InternalHostname,
                    Confidence::High,
                )?);
            }
        }
        for term in &self.custom_terms {
            for (start, _) in view.text.match_indices(term) {
                let end = start + term.len();
                if is_boundary(&view.text, start, end) {
                    findings.push(view.finding(
                        start..end,
                        DataClass::CustomTerm,
                        DetectorId::CustomTerm,
                        Confidence::High,
                    )?);
                }
            }
        }
        for prefix in &self.home_prefixes {
            for (start, _) in view.text.match_indices(prefix) {
                let end = view.text[start..]
                    .find(char::is_whitespace)
                    .map_or(view.text.len(), |relative| start + relative);
                findings.push(view.finding(
                    start..end,
                    DataClass::HomePath,
                    DetectorId::HomePath,
                    Confidence::High,
                )?);
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct ScanView {
    text: String,
    source_by_byte: Vec<Range<usize>>,
}

impl ScanView {
    fn normalize(source: &str) -> Result<Self, DetectorError> {
        let mut text = String::with_capacity(source.len());
        let mut source_by_byte = Vec::with_capacity(source.len());
        let mut chars = source.char_indices().peekable();
        while let Some((start, character)) = chars.next() {
            let end = chars.peek().map_or(source.len(), |(index, _)| *index);
            if is_invisible_control(character) {
                continue;
            }
            let normalized: String = character.to_string().nfkc().collect();
            append_with_source(&mut text, &mut source_by_byte, &normalized, start..end);
        }
        let view = Self {
            text,
            source_by_byte,
        };
        view.remove_structural_continuations()
    }

    fn remove_structural_continuations(self) -> Result<Self, DetectorError> {
        let bytes = self.text.as_bytes();
        let mut output = Vec::with_capacity(bytes.len());
        let mut mappings = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'\\' {
                let newline_end = if bytes.get(index + 1) == Some(&b'\n') {
                    Some(index + 2)
                } else if bytes.get(index + 1..index + 3) == Some(b"\r\n") {
                    Some(index + 3)
                } else {
                    None
                };
                if let Some(mut end) = newline_end {
                    while matches!(bytes.get(end), Some(b' ' | b'\t')) {
                        end += 1;
                    }
                    index = end;
                    continue;
                }
            }
            output.push(bytes[index]);
            mappings.push(self.source_by_byte[index].clone());
            index += 1;
        }
        Self::from_bytes(output, mappings)
    }

    fn decode_json_escapes(&self) -> Result<Option<Self>, DetectorError> {
        let bytes = self.text.as_bytes();
        let mut output = Vec::with_capacity(bytes.len());
        let mut mappings = Vec::with_capacity(bytes.len());
        let mut changed = false;
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'\\'
                && let Some((decoded, end)) = decode_escape(bytes, index)
            {
                let source = self.combined_source(index..end)?;
                for byte in decoded.as_bytes() {
                    output.push(*byte);
                    mappings.push(source.clone());
                }
                index = end;
                changed = true;
                continue;
            }
            output.push(bytes[index]);
            mappings.push(self.source_by_byte[index].clone());
            index += 1;
        }
        if !changed {
            return Ok(None);
        }
        Self::from_bytes(output, mappings).map(Some)
    }

    fn decode_percent_encoding(&self) -> Result<Option<Self>, DetectorError> {
        let bytes = self.text.as_bytes();
        let mut output = Vec::with_capacity(bytes.len());
        let mut mappings = Vec::with_capacity(bytes.len());
        let mut changed = false;
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%'
                && index + 2 < bytes.len()
                && let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2]))
            {
                output.push((high << 4) | low);
                mappings.push(self.combined_source(index..index + 3)?);
                index += 3;
                changed = true;
                continue;
            }
            output.push(bytes[index]);
            mappings.push(self.source_by_byte[index].clone());
            index += 1;
        }
        if !changed || std::str::from_utf8(&output).is_err() {
            return Ok(None);
        }
        Self::from_bytes(output, mappings).map(Some)
    }

    fn collapse_candidate_separators(&self) -> Result<Option<Self>, DetectorError> {
        let bytes = self.text.as_bytes();
        let mut output = Vec::with_capacity(bytes.len());
        let mut mappings = Vec::with_capacity(bytes.len());
        let mut changed = false;
        for (index, byte) in bytes.iter().copied().enumerate() {
            let collapse = byte == b'.'
                && index > 0
                && index + 1 < bytes.len()
                && is_token_byte(bytes[index - 1])
                && is_token_byte(bytes[index + 1]);
            if collapse {
                changed = true;
                continue;
            }
            output.push(byte);
            mappings.push(self.source_by_byte[index].clone());
        }
        if !changed {
            return Ok(None);
        }
        Self::from_bytes(output, mappings).map(Some)
    }

    fn from_bytes(bytes: Vec<u8>, mappings: Vec<Range<usize>>) -> Result<Self, DetectorError> {
        if bytes.len() != mappings.len() {
            return Err(DetectorError::InvalidSourceMap);
        }
        let text = String::from_utf8(bytes).map_err(|_| DetectorError::InvalidNormalizedUtf8)?;
        Ok(Self {
            text,
            source_by_byte: mappings,
        })
    }

    fn finding(
        &self,
        scan_span: Range<usize>,
        data_class: DataClass,
        detector: DetectorId,
        confidence: Confidence,
    ) -> Result<Finding, DetectorError> {
        Ok(Finding {
            data_class,
            detector,
            confidence,
            source_span: self.combined_source(scan_span)?,
        })
    }

    fn combined_source(&self, scan_span: Range<usize>) -> Result<Range<usize>, DetectorError> {
        if scan_span.is_empty() || scan_span.end > self.source_by_byte.len() {
            return Err(DetectorError::InvalidSourceMap);
        }
        let mappings = &self.source_by_byte[scan_span];
        let start = mappings
            .iter()
            .map(|span| span.start)
            .min()
            .ok_or(DetectorError::InvalidSourceMap)?;
        let end = mappings
            .iter()
            .map(|span| span.end)
            .max()
            .ok_or(DetectorError::InvalidSourceMap)?;
        if start >= end {
            return Err(DetectorError::InvalidSourceMap);
        }
        Ok(start..end)
    }
}

fn append_with_source(
    text: &mut String,
    mappings: &mut Vec<Range<usize>>,
    value: &str,
    source: Range<usize>,
) {
    text.push_str(value);
    mappings.extend((0..value.len()).map(|_| source.clone()));
}

fn is_invisible_control(character: char) -> bool {
    matches!(
        character,
        '\u{00AD}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}'
    )
}

fn decode_escape(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    match bytes.get(start + 1).copied()? {
        b'"' => Some(("\"".to_string(), start + 2)),
        b'\\' => Some(("\\".to_string(), start + 2)),
        b'/' => Some(("/".to_string(), start + 2)),
        b'b' => Some(("\u{0008}".to_string(), start + 2)),
        b'f' => Some(("\u{000C}".to_string(), start + 2)),
        b'n' => Some(("\n".to_string(), start + 2)),
        b'r' => Some(("\r".to_string(), start + 2)),
        b't' => Some(("\t".to_string(), start + 2)),
        b'u' => decode_unicode_escape(bytes, start),
        _ => None,
    }
}

fn decode_unicode_escape(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let first = parse_hex_quad(bytes.get(start + 2..start + 6)?)?;
    if (0xD800..=0xDBFF).contains(&first) {
        if bytes.get(start + 6..start + 8) != Some(b"\\u") {
            return None;
        }
        let second = parse_hex_quad(bytes.get(start + 8..start + 12)?)?;
        if !(0xDC00..=0xDFFF).contains(&second) {
            return None;
        }
        let scalar = 0x10000 + (((u32::from(first) - 0xD800) << 10) | (u32::from(second) - 0xDC00));
        return char::from_u32(scalar).map(|value| (value.to_string(), start + 12));
    }
    if (0xDC00..=0xDFFF).contains(&first) {
        return None;
    }
    char::from_u32(u32::from(first)).map(|value| (value.to_string(), start + 6))
}

fn parse_hex_quad(bytes: &[u8]) -> Option<u16> {
    if bytes.len() != 4 {
        return None;
    }
    let mut value = 0_u16;
    for byte in bytes {
        value = (value << 4) | u16::from(hex(*byte)?);
    }
    Some(value)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn is_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("${")
        || lower.starts_with("<")
        || [
            "changeme",
            "replace_me",
            "placeholder",
            "example_only",
            "your_secret_here",
        ]
        .iter()
        .any(|placeholder| lower == *placeholder)
}

fn is_boundary(value: &str, start: usize, end: usize) -> bool {
    let left = value[..start].chars().next_back();
    let right = value[end..].chars().next();
    !left.is_some_and(is_word_character) && !right.is_some_and(is_word_character)
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn is_hostname_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '.')
}

pub fn resolve_overlaps(
    findings: Vec<(Finding, crate::domain::Action)>,
) -> Vec<(Finding, crate::domain::Action)> {
    let mut ordered = findings;
    ordered.sort_by(
        |(left_finding, left_action), (right_finding, right_action)| {
            right_action
                .precedence()
                .cmp(&left_action.precedence())
                .then_with(|| {
                    (right_finding.source_span.end - right_finding.source_span.start)
                        .cmp(&(left_finding.source_span.end - left_finding.source_span.start))
                })
                .then_with(|| {
                    left_finding
                        .source_span
                        .start
                        .cmp(&right_finding.source_span.start)
                })
        },
    );
    let mut selected: Vec<(Finding, crate::domain::Action)> = Vec::new();
    let mut seen = HashSet::new();
    for candidate in ordered {
        let key = (
            candidate.0.source_span.start,
            candidate.0.source_span.end,
            candidate.0.data_class,
        );
        if seen.contains(&key)
            || selected
                .iter()
                .any(|(finding, _)| finding.overlaps(&candidate.0))
        {
            continue;
        }
        seen.insert(key);
        selected.push(candidate);
    }
    selected.sort_by_key(|(finding, _)| finding.source_span.start);
    selected
}

#[derive(Debug, Error)]
pub enum DetectorError {
    #[error("detector pattern failed to compile")]
    Pattern(#[source] regex::Error),
    #[error("normalized text is not valid UTF-8")]
    InvalidNormalizedUtf8,
    #[error("normalization source map is invalid")]
    InvalidSourceMap,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::Action;

    fn scanner() -> Scanner {
        Scanner::new(&DetectorSettings::default()).expect("scanner should compile")
    }

    #[test]
    fn detects_email_private_ip_and_contextual_phone() {
        let findings = scanner()
            .scan("email ava@example.test at 10.24.8.15; phone (212) 555-0134")
            .expect("scan should succeed");
        let classes: HashSet<_> = findings.iter().map(|finding| finding.data_class).collect();
        assert!(classes.contains(&DataClass::Email));
        assert!(classes.contains(&DataClass::PrivateIpv4));
        assert!(classes.contains(&DataClass::Phone));
    }

    #[test]
    fn ignores_public_ip_and_unlabelled_phone() {
        let findings = scanner()
            .scan("203.0.113.10 and 212-555-0134")
            .expect("scan should succeed");
        assert!(findings.is_empty());
    }

    #[test]
    fn detects_runtime_assembled_secret_shapes() {
        let openai = format!("{}{}", "sk-proj-", "A1b2C3d4E5f6G7h8I9j0");
        let github = format!("{}{}", "ghp_", "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5");
        let aws = format!("{}{}", "AKIA", "ABCDEFGHIJKLMNOP");
        let text = format!("{openai} {github} {aws}");
        let findings = scanner().scan(&text).expect("scan should succeed");
        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding.data_class == DataClass::Credentials)
                .count(),
            3
        );
    }

    #[test]
    fn normalization_defeats_zero_width_json_escape_and_percent_bypass() {
        let prefix = "sk-proj-";
        let body = "A1b2C3d4E5f6G7h8I9j0";
        let zero_width = format!("sk-\u{200B}proj-{body}");
        let escaped = format!(r"\u0073k-proj-{body}");
        let percent = format!("%73%6B%2D%70%72%6F%6A%2D{body}");
        for candidate in [format!("{prefix}{body}"), zero_width, escaped, percent] {
            let findings = scanner().scan(&candidate).expect("scan should succeed");
            assert!(
                findings
                    .iter()
                    .any(|finding| finding.detector == DetectorId::OpenAiApiKey),
                "candidate should be detected"
            );
        }
    }

    #[test]
    fn normalization_maps_fullwidth_compatibility_to_source() {
        let findings = scanner()
            .scan("contact ａｖａ@example.test")
            .expect("scan should succeed");
        let email = findings
            .iter()
            .find(|finding| finding.data_class == DataClass::Email)
            .expect("email should be found");
        assert_eq!(
            &"contact ａｖａ@example.test"[email.source_span.clone()],
            "ａｖａ@example.test"
        );
    }

    #[test]
    fn detects_complete_private_key_only() {
        let complete = "-----BEGIN PRIVATE KEY-----\nSYNTHETIC\n-----END PRIVATE KEY-----";
        let incomplete = "-----BEGIN PRIVATE KEY-----\nSYNTHETIC";
        assert!(
            scanner()
                .scan(complete)
                .expect("scan should succeed")
                .iter()
                .any(|finding| finding.data_class == DataClass::PrivateKey)
        );
        assert!(
            !scanner()
                .scan(incomplete)
                .expect("scan should succeed")
                .iter()
                .any(|finding| finding.data_class == DataClass::PrivateKey)
        );
    }

    #[test]
    fn restrictive_overlap_wins() {
        let broad = Finding {
            data_class: DataClass::Credentials,
            detector: DetectorId::EnvCredentialAssignment,
            confidence: Confidence::Contextual,
            source_span: 0..20,
        };
        let narrow = Finding {
            data_class: DataClass::Email,
            detector: DetectorId::Email,
            confidence: Confidence::High,
            source_span: 5..15,
        };
        let result = resolve_overlaps(vec![
            (narrow, Action::Tokenize),
            (broad.clone(), Action::Block),
        ]);
        assert_eq!(result, vec![(broad, Action::Block)]);
    }
}
