//! AgentVeil's local privacy engine.
//!
//! The engine is intentionally independent from HTTP I/O: callers must fully
//! buffer a bounded Responses request, parse it, and obtain a sanitized body
//! before opening an upstream connection.

#![forbid(unsafe_code)]

pub mod audit;
pub mod detector;
pub mod domain;
pub mod engine;
pub mod ledger;
pub mod payload;
pub mod policy;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
