use std::fs::File;
use std::io::Read;
use std::process;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct EpcGenerator {
    state: Mutex<EpcState>,
}

#[derive(Debug)]
struct EpcState {
    last_ns: i64,
    seq: u32,
    salt: u32,
}

impl EpcGenerator {
    pub fn new() -> Self {
        Self::with_salt(new_epc_salt())
    }

    pub fn with_salt(salt: u32) -> Self {
        Self {
            state: Mutex::new(EpcState {
                last_ns: 0,
                seq: 0,
                salt: salt | 1,
            }),
        }
    }

    pub fn next(&self) -> String {
        self.next_at_unix_ns(now_unix_ns())
    }

    pub fn next_at_unix_ns(&self, ns: i64) -> String {
        let mut state = self.state.lock().expect("epc generator mutex poisoned");
        if ns != state.last_ns {
            state.last_ns = ns;
            state.seq = 0;
        } else {
            state.seq = state.seq.wrapping_add(1);
        }
        format_epc_24(ns, state.seq, state.salt)
    }
}

impl Default for EpcGenerator {
    fn default() -> Self {
        Self::new()
    }
}

pub fn format_epc_24(ns: i64, seq: u32, salt: u32) -> String {
    let ns_bits = ns as u64;
    let atom = ((ns_bits / 1_000) & 0xFFFF_FFFF) as u32;
    let mut tail = atom ^ (ns as u32).rotate_left(13) ^ seq.rotate_left(7) ^ salt;
    tail |= 1;
    format!("30{:014X}{:08X}", ns_bits & 0x00FF_FFFF_FFFF_FFFF, tail)
}

fn new_epc_salt() -> u32 {
    read_os_random_u32().unwrap_or_else(|| {
        let fallback = now_unix_ns() as u32 ^ ((process::id() as u32) << 16);
        fallback | 1
    }) | 1
}

fn read_os_random_u32() -> Option<u32> {
    let mut bytes = [0_u8; 4];
    let mut file = File::open("/dev/urandom").ok()?;
    file.read_exact(&mut bytes).ok()?;
    Some(u32::from_be_bytes(bytes))
}

fn now_unix_ns() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_nanos().min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epc_like_gscale_go_formula() {
        assert_eq!(
            format_epc_24(1_691_139_600_123_456_789, 0, 0x1357_9BDF),
            "307822819AF46D1581D4AEC1"
        );
        assert_eq!(
            format_epc_24(1_691_139_600_123_456_789, 1, 0x1357_9BDF),
            "307822819AF46D1581D4AE41"
        );
    }

    #[test]
    fn generator_increments_sequence_for_same_timestamp() {
        let generator = EpcGenerator::with_salt(0x1357_9BDE);
        let ns = 1_691_139_600_123_456_789;

        let first = generator.next_at_unix_ns(ns);
        let second = generator.next_at_unix_ns(ns);

        assert_eq!(first, "307822819AF46D1581D4AEC1");
        assert_eq!(second, "307822819AF46D1581D4AE41");
        assert_ne!(first, second);
    }

    #[test]
    fn generator_resets_sequence_for_new_timestamp() {
        let generator = EpcGenerator::with_salt(0x1357_9BDF);

        let first = generator.next_at_unix_ns(1_691_139_600_123_456_789);
        let second = generator.next_at_unix_ns(1_691_139_600_123_456_790);

        assert_eq!(first, "307822819AF46D1581D4AEC1");
        assert_eq!(second, "307822819AF46D1681D4CEC1");
    }

    #[test]
    fn generated_epc_is_24_upper_hex_chars() {
        let epc = EpcGenerator::new().next();

        assert_eq!(epc.len(), 24);
        assert!(epc.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(epc, epc.to_ascii_uppercase());
    }
}
