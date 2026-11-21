//! Shared value representation for persisted and product-authored settings.
//!
//! This type lives outside both the settings schema and desired-state storage
//! so those modules have a one-way dependency rather than a cycle.

use serde::{Deserialize, Serialize};

/// A user-set setting value stored only when the user changes that setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    /// A string or authored-choice value.
    Str(String),
    /// An enabled/disabled value.
    Bool(bool),
}
