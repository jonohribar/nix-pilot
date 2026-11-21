//! System-operation adapters grouped by their responsibility.
//!
//! Candidate builds, privileged actions, storage upkeep, and generation
//! listing remain separate contracts. Shared process lifecycle is owned by
//! `crate::process`.

pub mod activation;
pub mod build;
pub mod generation;
mod helper;
pub mod storage;
