use serde::Deserialize;

/// Options that control how the scrubber behaves. Values are merged with sensible defaults.
#[derive(Debug, Default, Deserialize)]
pub struct ScrubberConfig {
    /// Additional person names to scrub (case-insensitive).
    #[serde(default)]
    pub names: Vec<String>,
    /// Additional keywords or facility names to scrub (case-insensitive).
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Overrides the minimum length for MRN detection (default: 6).
    #[serde(default)]
    pub mrn_min_length: Option<usize>,
    /// Overrides the maximum length for MRN detection (default: 10).
    #[serde(default)]
    pub mrn_max_length: Option<usize>,
}
