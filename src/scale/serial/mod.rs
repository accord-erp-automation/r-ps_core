pub mod detect;

pub use detect::{
    DetectError, DetectedScalePort, ProbeOutcome, ScaleProbe, collect_serial_candidates,
    detect_scale_port_with_probe, is_busy_error, list_serial_candidates,
};
