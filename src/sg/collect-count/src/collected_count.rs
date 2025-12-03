//! Data structures for representing collected count statistics.
//!
//! This module provides the `CollectedCount` type and related structures
//! that represent the output of the collect-count command in a structured,
//! type-safe way before JSON serialization.

use crate::word_count::WordCount;
use serde::{Deserialize, Serialize};

/// Represents a single file's count statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
    pub bytes: usize,
    pub max_line_length: usize,
}

impl FileCount {
    /// Create a FileCount from a WordCount and optional filename
    pub fn from_word_count(title: Option<String>, wc: &WordCount) -> Self {
        Self {
            file: title,
            lines: wc.lines,
            words: wc.words,
            chars: wc.chars,
            bytes: wc.bytes,
            max_line_length: wc.max_line_length,
        }
    }
}

/// Total count statistics across all files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalCount {
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
    pub bytes: usize,
    pub max_line_length: usize,
}

impl From<&WordCount> for TotalCount {
    fn from(wc: &WordCount) -> Self {
        Self {
            lines: wc.lines,
            words: wc.words,
            chars: wc.chars,
            bytes: wc.bytes,
            max_line_length: wc.max_line_length,
        }
    }
}

/// Metadata about the counting operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountMetadata {
    pub json_input: bool,
}

/// Metadata about the input source (internal representation)
#[derive(Debug, Default)]
pub struct InputMetadata {
    pub json_input_detected: bool,
}

impl From<&InputMetadata> for CountMetadata {
    fn from(input_meta: &InputMetadata) -> Self {
        Self {
            json_input: input_meta.json_input_detected,
        }
    }
}

/// The complete output object for collect-count
///
/// This structure represents all the data collected by the collect-count
/// command in a structured format. It can be serialized to JSON for output
/// in pipeline contexts.
///
/// # Example
///
/// ```rust,ignore
/// let collected = CollectedCount::new(
///     file_results,
///     &total_word_count,
///     num_inputs,
///     &input_metadata,
/// );
///
/// let json = collected.to_json();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedCount {
    pub files: Vec<FileCount>,
    pub total: TotalCount,
    pub files_counted: usize,
    pub metadata: CountMetadata,
}

impl CollectedCount {
    /// Create a new CollectedCount from counting results
    pub fn new(
        file_results: Vec<(Option<String>, WordCount)>,
        total: &WordCount,
        files_counted: usize,
        input_metadata: &InputMetadata,
    ) -> Self {
        let files = file_results
            .iter()
            .map(|(title, wc)| FileCount::from_word_count(title.clone(), wc))
            .collect();

        Self {
            files,
            total: TotalCount::from(total),
            files_counted,
            metadata: CountMetadata::from(input_metadata),
        }
    }

    /// Convert to JSON value
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}
