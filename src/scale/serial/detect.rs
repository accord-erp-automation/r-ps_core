use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DetectedScalePort {
    pub device: String,
    pub baud: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProbeOutcome {
    pub parsed_weight: bool,
    pub has_data: bool,
}

impl ProbeOutcome {
    pub fn empty() -> Self {
        Self {
            parsed_weight: false,
            has_data: false,
        }
    }

    pub fn data() -> Self {
        Self {
            parsed_weight: false,
            has_data: true,
        }
    }

    pub fn parsed_weight() -> Self {
        Self {
            parsed_weight: true,
            has_data: true,
        }
    }
}

pub trait ScaleProbe {
    fn probe(&mut self, device: &str, baud: u32) -> Result<ProbeOutcome, String>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DetectError {
    EmptyBaudList,
    NoCandidates,
    Busy(String),
}

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBaudList => write!(f, "empty baud list"),
            Self::NoCandidates => {
                write!(
                    f,
                    "serial device topilmadi (/dev/ttyUSB* yoki /dev/ttyACM*)"
                )
            }
            Self::Busy(err) => write!(f, "serial port band: {err}"),
        }
    }
}

impl std::error::Error for DetectError {}

pub fn detect_scale_port_with_probe(
    explicit_device: &str,
    bauds: &[u32],
    candidates: &[String],
    probe: &mut impl ScaleProbe,
) -> Result<DetectedScalePort, DetectError> {
    let Some(first_baud) = bauds.first().copied() else {
        return Err(DetectError::EmptyBaudList);
    };

    let explicit_device = explicit_device.trim();
    if !explicit_device.is_empty() {
        return Ok(DetectedScalePort {
            device: explicit_device.to_string(),
            baud: first_baud,
        });
    }

    if candidates.is_empty() {
        return Err(DetectError::NoCandidates);
    }

    let mut last_busy: Option<String> = None;
    for device in candidates {
        for &baud in bauds {
            match probe.probe(device, baud) {
                Ok(outcome) if outcome.parsed_weight || outcome.has_data => {
                    return Ok(DetectedScalePort {
                        device: device.clone(),
                        baud,
                    });
                }
                Ok(_) => {}
                Err(err) if is_busy_error(&err) => {
                    last_busy = Some(format!("{device} band: {err}"));
                }
                Err(_) => {}
            }
        }
    }

    if let Some(err) = last_busy {
        return Err(DetectError::Busy(err));
    }

    Ok(DetectedScalePort {
        device: candidates[0].clone(),
        baud: first_baud,
    })
}

pub fn list_serial_candidates() -> Vec<String> {
    collect_serial_candidates(Path::new("/dev"))
}

pub fn collect_serial_candidates(dev_root: &Path) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(16);

    for path in sorted_dir_entries(&dev_root.join("serial").join("by-id")) {
        let resolved = fs::canonicalize(&path).unwrap_or(path);
        push_unique(&mut out, &mut seen, resolved);
    }

    for prefix in ["ttyUSB", "ttyACM"] {
        let mut matches: Vec<PathBuf> = sorted_dir_entries(dev_root)
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix))
            })
            .collect();
        matches.sort();

        for path in matches {
            push_unique(&mut out, &mut seen, path);
        }
    }

    out
}

pub fn is_busy_error(err: &str) -> bool {
    let msg = err.to_ascii_lowercase();
    msg.contains("resource busy")
        || msg.contains("device or resource busy")
        || msg.contains("permission denied")
}

fn sorted_dir_entries(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .collect();
    paths.sort();
    paths
}

fn push_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, path: PathBuf) {
    let value = path.to_string_lossy().trim().to_string();
    if value.is_empty() || seen.contains(&value) {
        return;
    }
    seen.insert(value.clone());
    out.push(value);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        DetectError, ProbeOutcome, ScaleProbe, collect_serial_candidates,
        detect_scale_port_with_probe, is_busy_error,
    };

    #[derive(Default)]
    struct FakeProbe {
        outcomes: HashMap<(String, u32), Result<ProbeOutcome, String>>,
        calls: Vec<(String, u32)>,
    }

    impl FakeProbe {
        fn with(mut self, device: &str, baud: u32, result: Result<ProbeOutcome, &str>) -> Self {
            self.outcomes.insert(
                (device.to_string(), baud),
                result.map_err(|err| err.to_string()),
            );
            self
        }
    }

    impl ScaleProbe for FakeProbe {
        fn probe(&mut self, device: &str, baud: u32) -> Result<ProbeOutcome, String> {
            self.calls.push((device.to_string(), baud));
            self.outcomes
                .get(&(device.to_string(), baud))
                .cloned()
                .unwrap_or_else(|| Ok(ProbeOutcome::empty()))
        }
    }

    #[test]
    fn explicit_device_wins_and_uses_first_baud_like_go() {
        let mut probe = FakeProbe::default();
        let detected =
            detect_scale_port_with_probe(" /dev/custom ", &[19200, 9600], &[], &mut probe).unwrap();

        assert_eq!(detected.device, "/dev/custom");
        assert_eq!(detected.baud, 19200);
        assert!(probe.calls.is_empty());
    }

    #[test]
    fn parsed_weight_or_any_data_selects_candidate_like_go() {
        let mut probe = FakeProbe::default()
            .with("/dev/a", 9600, Ok(ProbeOutcome::empty()))
            .with("/dev/a", 19200, Ok(ProbeOutcome::data()))
            .with("/dev/b", 9600, Ok(ProbeOutcome::parsed_weight()));
        let candidates = vec!["/dev/a".to_string(), "/dev/b".to_string()];

        let detected =
            detect_scale_port_with_probe("", &[9600, 19200], &candidates, &mut probe).unwrap();

        assert_eq!(detected.device, "/dev/a");
        assert_eq!(detected.baud, 19200);
    }

    #[test]
    fn fallback_uses_first_candidate_and_first_baud_like_go() {
        let mut probe = FakeProbe::default();
        let candidates = vec!["/dev/ttyUSB0".to_string(), "/dev/ttyACM0".to_string()];

        let detected = detect_scale_port_with_probe("", &[9600], &candidates, &mut probe).unwrap();

        assert_eq!(detected.device, "/dev/ttyUSB0");
        assert_eq!(detected.baud, 9600);
    }

    #[test]
    fn busy_error_wins_after_all_candidates_fail_like_go() {
        let mut probe = FakeProbe::default()
            .with("/dev/ttyUSB0", 9600, Err("permission denied"))
            .with("/dev/ttyACM0", 9600, Err("temporary decode error"));
        let candidates = vec!["/dev/ttyUSB0".to_string(), "/dev/ttyACM0".to_string()];

        let err = detect_scale_port_with_probe("", &[9600], &candidates, &mut probe).unwrap_err();

        assert_eq!(
            err,
            DetectError::Busy("/dev/ttyUSB0 band: permission denied".to_string())
        );
        assert_eq!(
            err.to_string(),
            "serial port band: /dev/ttyUSB0 band: permission denied"
        );
    }

    #[test]
    fn no_candidates_matches_production_error_text() {
        let mut probe = FakeProbe::default();

        let err = detect_scale_port_with_probe("", &[9600], &[], &mut probe).unwrap_err();

        assert_eq!(
            err.to_string(),
            "serial device topilmadi (/dev/ttyUSB* yoki /dev/ttyACM*)"
        );
    }

    #[test]
    fn busy_error_matching_is_case_insensitive_like_go() {
        assert!(is_busy_error("RESOURCE BUSY"));
        assert!(is_busy_error("device or resource busy"));
        assert!(is_busy_error("Permission Denied"));
        assert!(!is_busy_error("temporary decode error"));
    }

    #[test]
    fn candidate_scan_orders_by_id_usb_acm_and_removes_duplicates() {
        let root = temp_root("rp-scale-detect");
        fs::create_dir_all(root.join("serial/by-id")).unwrap();
        fs::write(root.join("ttyUSB0"), "").unwrap();
        fs::write(root.join("ttyACM0"), "").unwrap();
        fs::write(root.join("ttyACM1"), "").unwrap();

        let by_id = root.join("serial/by-id/scale-a");
        make_symlink_or_file(&root.join("ttyUSB0"), &by_id);

        let candidates = collect_serial_candidates(&root);

        assert_eq!(candidates[0], canonical_or_self(root.join("ttyUSB0")));
        assert_eq!(candidates[1], root.join("ttyACM0").to_string_lossy());
        assert_eq!(candidates[2], root.join("ttyACM1").to_string_lossy());
        assert_eq!(candidates.len(), 3);

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
        base.join(format!("{name}-{}-{nanos}", std::process::id()))
    }

    fn canonical_or_self(path: PathBuf) -> String {
        fs::canonicalize(&path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }

    #[cfg(unix)]
    fn make_symlink_or_file(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[cfg(not(unix))]
    fn make_symlink_or_file(_target: &Path, link: &Path) {
        fs::write(link, "").unwrap();
    }
}
