use std::io::{ErrorKind, Read};
use std::time::{Duration, Instant};

use crate::scale::{Reading, SerialStreamDecoder};

#[derive(Debug, Clone)]
pub struct SerialReaderConfig {
    pub device: String,
    pub baud: u32,
    pub unit: String,
    pub read_timeout: Duration,
    pub reconnect_delay: Duration,
}

impl SerialReaderConfig {
    pub fn new(device: &str, baud: u32, unit: &str) -> Self {
        Self {
            device: device.trim().to_string(),
            baud,
            unit: normalize_unit(unit),
            read_timeout: Duration::from_millis(250),
            reconnect_delay: Duration::from_millis(400),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerialReader {
    config: SerialReaderConfig,
}

impl SerialReader {
    pub fn new(config: SerialReaderConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SerialReaderConfig {
        &self.config
    }

    pub fn run_with_limits(&self, max_readings: usize, max_runtime: Duration) -> Vec<Reading> {
        let deadline = Instant::now() + max_runtime;
        let mut out = Vec::new();

        while out.len() < max_readings && Instant::now() < deadline {
            let mut source = match open_read_source(&self.config) {
                Ok(source) => source,
                Err(err) => {
                    out.push(
                        self.base_reading()
                            .with_error(&format!("open error: {err}")),
                    );
                    sleep_until_or_deadline(self.config.reconnect_delay, deadline);
                    continue;
                }
            };

            out.push(self.base_reading());
            let mut decoder =
                SerialStreamDecoder::new(&self.config.device, self.config.baud, &self.config.unit);

            match self.stream_until(&mut source, &mut decoder, max_readings, deadline, &mut out) {
                Ok(()) => {}
                Err(err) if Instant::now() < deadline => {
                    out.push(
                        self.base_reading()
                            .with_error(&format!("read error: {err}")),
                    );
                    sleep_until_or_deadline(self.config.reconnect_delay, deadline);
                }
                Err(_) => {}
            }
        }

        out.truncate(max_readings);
        out
    }

    pub fn run_forever<F>(&self, mut on_reading: F)
    where
        F: FnMut(Reading),
    {
        loop {
            let mut source = match open_read_source(&self.config) {
                Ok(source) => source,
                Err(err) => {
                    on_reading(
                        self.base_reading()
                            .with_error(&format!("open error: {err}")),
                    );
                    std::thread::sleep(self.config.reconnect_delay);
                    continue;
                }
            };

            on_reading(self.base_reading());
            let mut decoder =
                SerialStreamDecoder::new(&self.config.device, self.config.baud, &self.config.unit);
            let mut buf = [0_u8; 256];

            loop {
                match source.read(&mut buf) {
                    Ok(0) => {}
                    Ok(n) => {
                        for reading in decoder.push_chunk(&String::from_utf8_lossy(&buf[..n])) {
                            on_reading(reading);
                        }
                    }
                    Err(err) if err.kind() == ErrorKind::TimedOut => {}
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(err) => {
                        on_reading(
                            self.base_reading()
                                .with_error(&format!("read error: {err}")),
                        );
                        std::thread::sleep(self.config.reconnect_delay);
                        break;
                    }
                }
            }
        }
    }

    fn stream_until(
        &self,
        source: &mut dyn Read,
        decoder: &mut SerialStreamDecoder,
        max_readings: usize,
        deadline: Instant,
        out: &mut Vec<Reading>,
    ) -> Result<(), String> {
        let mut buf = [0_u8; 256];
        while out.len() < max_readings && Instant::now() < deadline {
            match source.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => out.extend(decoder.push_chunk(&String::from_utf8_lossy(&buf[..n]))),
                Err(err) if err.kind() == ErrorKind::TimedOut => {}
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(err) => return Err(err.to_string()),
            }
        }
        Ok(())
    }

    fn base_reading(&self) -> Reading {
        Reading::serial(&self.config.device, self.config.baud, &self.config.unit)
    }
}

fn open_read_source(config: &SerialReaderConfig) -> Result<Box<dyn Read>, String> {
    let port = serialport::new(&config.device, config.baud)
        .timeout(config.read_timeout)
        .open()
        .map_err(|err| err.to_string());

    match port {
        Ok(port) => Ok(port),
        Err(err) if should_try_unix_pty_fallback(&err) => open_unix_pty_source(&config.device),
        Err(err) => Err(err),
    }
}

#[cfg(unix)]
fn open_unix_pty_source(device: &str) -> Result<Box<dyn Read>, String> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(device)
        .map_err(|err| err.to_string())?;
    Ok(Box::new(file))
}

#[cfg(not(unix))]
fn open_unix_pty_source(_device: &str) -> Result<Box<dyn Read>, String> {
    Err("unix pty fallback is unavailable".to_string())
}

fn sleep_until_or_deadline(duration: Duration, deadline: Instant) {
    let now = Instant::now();
    if now >= deadline {
        return;
    }
    std::thread::sleep(duration.min(deadline - now));
}

fn normalize_unit(unit: &str) -> String {
    let unit = unit.trim().to_ascii_lowercase();
    if unit.is_empty() {
        "kg".to_string()
    } else {
        unit
    }
}

fn should_try_unix_pty_fallback(err: &str) -> bool {
    let err = err.to_ascii_lowercase();
    err.contains("not a typewriter") || err.contains("inappropriate ioctl for device")
}

#[cfg(test)]
mod tests {
    use super::{SerialReaderConfig, normalize_unit, should_try_unix_pty_fallback};

    #[test]
    fn config_defaults_match_go_runtime_intent() {
        let config = SerialReaderConfig::new(" /dev/ttyUSB0 ", 9600, "");

        assert_eq!(config.device, "/dev/ttyUSB0");
        assert_eq!(config.baud, 9600);
        assert_eq!(config.unit, "kg");
        assert_eq!(config.read_timeout.as_millis(), 250);
        assert_eq!(config.reconnect_delay.as_millis(), 400);
    }

    #[test]
    fn normalizes_unit() {
        assert_eq!(normalize_unit(" KG "), "kg");
        assert_eq!(normalize_unit(""), "kg");
    }

    #[test]
    fn detects_pty_fallback_errors() {
        assert!(should_try_unix_pty_fallback("Not a typewriter"));
        assert!(should_try_unix_pty_fallback(
            "inappropriate ioctl for device"
        ));
        assert!(!should_try_unix_pty_fallback("permission denied"));
    }

    #[test]
    fn run_forever_api_is_available_for_runtime_monitor() {
        fn assert_callback<F: FnMut(crate::scale::Reading)>(_callback: F) {}
        assert_callback(|_reading| {});
    }
}
