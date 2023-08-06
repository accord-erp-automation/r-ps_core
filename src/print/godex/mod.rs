pub mod options;
pub mod payload;
pub mod text;

pub use options::LabelOptions;
pub use payload::{DEFAULT_QR_BASE_URL, encode_scan_payload};
pub use text::{normalize_kg_value, sanitize_label_text};
