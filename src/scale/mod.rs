pub mod capabilities;
pub mod core;
pub mod driver;
pub mod frame;
pub mod parser;
pub mod reading;
pub mod serial;
pub mod stable;
pub mod stream;

pub use capabilities::{ScaleCapabilities, ScaleTransport};
pub use core::ScaleCoreState;
pub use driver::{ScaleChunkDecoder, ScaleDriver};
pub use frame::{append_raw, pop_serial_frame, sanitize_inline};
pub use parser::{ParsedWeight, parse_weight, stable_text};
pub use reading::Reading;
pub use serial::{
    DetectError, DetectedScalePort, ProbeOutcome, ScaleProbe, SerialPortProbe, SerialReader,
    SerialReaderConfig, collect_serial_candidates, detect_scale_port_with_probe, is_busy_error,
    list_serial_candidates,
};
pub use stable::{StableConfig, StableSnapshot, StableState, StableTracker};
pub use stream::SerialStreamDecoder;
