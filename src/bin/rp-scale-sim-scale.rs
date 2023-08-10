#[cfg(unix)]
fn main() {
    if let Err(err) = unix_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("rp-scale-sim-scale requires a Unix pseudo terminal");
    std::process::exit(1);
}

#[cfg(unix)]
fn unix_main() -> Result<(), String> {
    use std::io::{self, Write};
    use std::thread;
    use std::time::Duration;

    let cfg = SimConfig::from_args(std::env::args().skip(1))?;
    let pty = open_simulator_pty()?;
    println!("device={}", pty.slave_name);
    io::stdout().flush().map_err(|err| err.to_string())?;

    let mut master = pty.master;
    let mut sequencer = SampleSequencer::new(cfg.samples());
    let mut sent = 0_u64;

    loop {
        if cfg.count > 0 && sent >= cfg.count {
            break;
        }

        let sample = sequencer.next();
        let frame = format_frame(sample.weight, &cfg.unit, sample.stable);
        if let Err(err) = master.write_all(frame.as_bytes())
            && err.raw_os_error() != Some(libc::EIO)
        {
            return Err(err.to_string());
        }
        let _ = master.flush();

        sent += 1;
        thread::sleep(Duration::from_millis(cfg.interval_ms));
    }

    Ok(())
}

#[cfg(unix)]
#[derive(Debug, Clone)]
struct SimConfig {
    scenario: String,
    weight: Option<f64>,
    stable: Option<bool>,
    unit: String,
    interval_ms: u64,
    count: u64,
}

#[cfg(unix)]
impl SimConfig {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut cfg = Self {
            scenario: "batch".to_string(),
            weight: None,
            stable: None,
            unit: "kg".to_string(),
            interval_ms: 120,
            count: 0,
        };

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--scenario" => cfg.scenario = next_value(&mut args, "--scenario")?,
                "--weight" => cfg.weight = Some(parse_f64(&next_value(&mut args, "--weight")?)?),
                "--stable" => cfg.stable = Some(parse_bool(&next_value(&mut args, "--stable")?)?),
                "--unit" => cfg.unit = next_value(&mut args, "--unit")?,
                "--interval-ms" => {
                    cfg.interval_ms = parse_u64(&next_value(&mut args, "--interval-ms")?)?
                }
                "--count" => cfg.count = parse_u64(&next_value(&mut args, "--count")?)?,
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument: {arg}\n{}", usage())),
            }
        }

        if cfg.unit.trim().is_empty() {
            cfg.unit = "kg".to_string();
        }
        if cfg.interval_ms == 0 {
            cfg.interval_ms = 1;
        }

        Ok(cfg)
    }

    fn samples(&self) -> Vec<ScaleSample> {
        if let Some(weight) = self.weight {
            return vec![ScaleSample {
                weight,
                stable: self.stable.unwrap_or(true),
                repeat: stable_repeat(self.interval_ms, 1_500),
            }];
        }

        scenario_samples(&self.scenario, self.interval_ms)
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct ScaleSample {
    weight: f64,
    stable: bool,
    repeat: u16,
}

#[cfg(unix)]
struct SampleSequencer {
    samples: Vec<ScaleSample>,
    index: usize,
    remaining: u16,
}

#[cfg(unix)]
impl SampleSequencer {
    fn new(samples: Vec<ScaleSample>) -> Self {
        Self {
            samples,
            index: 0,
            remaining: 0,
        }
    }

    fn next(&mut self) -> ScaleSample {
        if self.samples.is_empty() {
            return ScaleSample {
                weight: 0.0,
                stable: true,
                repeat: 1,
            };
        }
        if self.remaining == 0 {
            self.remaining = self.samples[self.index].repeat.max(1);
        }

        let sample = self.samples[self.index];
        self.remaining -= 1;
        if self.remaining == 0 {
            self.index = (self.index + 1) % self.samples.len();
        }
        sample
    }
}

#[cfg(unix)]
struct Pty {
    master: std::fs::File,
    slave: std::fs::File,
    slave_name: String,
}

#[cfg(unix)]
struct SimulatorPty {
    master: std::fs::File,
    slave_name: String,
}

#[cfg(unix)]
fn open_simulator_pty() -> Result<SimulatorPty, String> {
    let Pty {
        master,
        slave,
        slave_name,
    } = open_pty()?;
    drop(slave);
    Ok(SimulatorPty { master, slave_name })
}

#[cfg(unix)]
fn open_pty() -> Result<Pty, String> {
    use std::os::fd::FromRawFd;

    let mut master_fd = 0;
    let mut slave_fd = 0;
    let mut name = [0_i8; 128];

    let rc = unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            name.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }

    let slave_name = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
        .to_string_lossy()
        .to_string();
    if slave_name.trim().is_empty() {
        return Err("openpty returned empty slave name".to_string());
    }

    let master = unsafe { std::fs::File::from_raw_fd(master_fd) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave_fd) };
    Ok(Pty {
        master,
        slave,
        slave_name,
    })
}

#[cfg(unix)]
fn scenario_samples(name: &str, interval_ms: u64) -> Vec<ScaleSample> {
    match name.trim().to_ascii_lowercase().as_str() {
        "idle" => idle_samples(interval_ms),
        "stress" => stress_samples(interval_ms),
        _ => batch_samples(interval_ms),
    }
}

#[cfg(unix)]
fn batch_samples(interval_ms: u64) -> Vec<ScaleSample> {
    let stable = stable_repeat(interval_ms, 1_600);
    let zero = stable_repeat(interval_ms, 1_200);
    let mut out = Vec::new();
    push_hold(&mut out, 0.0, true, zero);
    push_ramp(&mut out, 0.0, 1.250, 5);
    push_hold(&mut out, 1.250, false, 2);
    push_hold(&mut out, 1.250, true, stable);
    push_ramp(&mut out, 1.250, 0.0, 4);
    push_hold(&mut out, 0.0, true, zero);
    push_ramp(&mut out, 0.0, 2.750, 7);
    push_hold(&mut out, 2.750, false, 2);
    push_hold(&mut out, 2.750, true, stable_repeat(interval_ms, 2_000));
    push_ramp(&mut out, 2.750, 0.0, 5);
    push_hold(&mut out, 0.0, true, zero);
    push_ramp(&mut out, 0.0, 0.640, 3);
    push_hold(&mut out, 0.640, true, stable);
    push_ramp(&mut out, 0.640, 3.180, 6);
    push_hold(&mut out, 3.180, false, 3);
    push_hold(&mut out, 3.180, true, stable_repeat(interval_ms, 1_800));
    out
}

#[cfg(unix)]
fn stress_samples(interval_ms: u64) -> Vec<ScaleSample> {
    let mut out = Vec::new();
    push_hold(&mut out, 0.0, true, stable_repeat(interval_ms, 700));
    push_ramp(&mut out, 0.0, 1.870, 5);
    push_hold(&mut out, 1.930, false, 1);
    push_hold(&mut out, 1.810, false, 1);
    push_hold(&mut out, 1.842, true, stable_repeat(interval_ms, 1_200));
    push_ramp(&mut out, 1.842, 0.280, 4);
    push_ramp(&mut out, 0.280, 2.420, 4);
    push_hold(&mut out, 2.420, true, stable_repeat(interval_ms, 1_000));
    push_ramp(&mut out, 2.420, 0.0, 5);
    push_hold(&mut out, 0.0, true, stable_repeat(interval_ms, 800));
    out
}

#[cfg(unix)]
fn idle_samples(interval_ms: u64) -> Vec<ScaleSample> {
    vec![
        ScaleSample {
            weight: 0.0,
            stable: true,
            repeat: stable_repeat(interval_ms, 2_000),
        },
        ScaleSample {
            weight: 0.002,
            stable: true,
            repeat: stable_repeat(interval_ms, 1_500),
        },
    ]
}

#[cfg(unix)]
fn push_ramp(out: &mut Vec<ScaleSample>, from: f64, to: f64, steps: u16) {
    for step in 1..=steps {
        let p = f64::from(step) / f64::from(steps);
        let weight = round3(from + (to - from) * p);
        push_hold(out, weight, false, 1);
    }
}

#[cfg(unix)]
fn push_hold(out: &mut Vec<ScaleSample>, weight: f64, stable: bool, repeat: u16) {
    out.push(ScaleSample {
        weight: round3(weight),
        stable,
        repeat: repeat.max(1),
    });
}

#[cfg(unix)]
fn stable_repeat(interval_ms: u64, duration_ms: u64) -> u16 {
    let interval = interval_ms.max(1);
    let repeat = duration_ms.div_ceil(interval).max(1);
    repeat.min(u64::from(u16::MAX)) as u16
}

#[cfg(unix)]
fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(unix)]
fn format_frame(weight: f64, unit: &str, stable: bool) -> String {
    let marker = if stable { "ST" } else { "US" };
    format!("{weight:.3} {} {marker}\r", unit.trim())
}

#[cfg(unix)]
fn next_value(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    name: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value\n{}", usage()))
}

#[cfg(unix)]
fn parse_f64(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("invalid float: {value}"))
}

#[cfg(unix)]
fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("invalid u64: {value}"))
}

#[cfg(unix)]
fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "stable" => Ok(true),
        "0" | "false" | "no" | "unstable" => Ok(false),
        _ => Err(format!("invalid bool: {value}")),
    }
}

#[cfg(unix)]
fn usage() -> String {
    "usage: rp-scale-sim-scale [--scenario batch|idle|stress] [--weight N] [--stable true|false] [--unit kg] [--interval-ms N] [--count N]".to_string()
}

#[cfg(all(test, unix))]
mod tests {
    use super::open_simulator_pty;

    #[test]
    fn simulator_pty_slave_is_available_for_scale_reader() {
        let pty = open_simulator_pty().unwrap();
        if let Err(err) = serialport::new(&pty.slave_name, 9600).open() {
            let detail = err.to_string().to_ascii_lowercase();
            assert!(
                !detail.contains("resource busy"),
                "simulator PTY must not be busy for scale reader: {err}"
            );
            std::fs::OpenOptions::new()
                .read(true)
                .open(&pty.slave_name)
                .unwrap();
        }
    }
}
