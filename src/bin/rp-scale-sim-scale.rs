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
    let pty = open_pty()?;
    println!("device={}", pty.slave_name);
    io::stdout().flush().map_err(|err| err.to_string())?;

    let mut master = pty.master;
    let _keep_slave_open = pty.slave;
    let mut index = 0_usize;
    let mut sent = 0_u64;

    loop {
        if cfg.count > 0 && sent >= cfg.count {
            break;
        }

        let sample = cfg.sample(index);
        let frame = format_frame(sample.weight, &cfg.unit, sample.stable);
        if let Err(err) = master.write_all(frame.as_bytes())
            && err.raw_os_error() != Some(libc::EIO)
        {
            return Err(err.to_string());
        }
        let _ = master.flush();

        sent += 1;
        index = index.wrapping_add(1);
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

    fn sample(&self, index: usize) -> ScaleSample {
        if let Some(weight) = self.weight {
            return ScaleSample {
                weight,
                stable: self.stable.unwrap_or(true),
            };
        }

        let samples = scenario_samples(&self.scenario);
        samples[index % samples.len()]
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct ScaleSample {
    weight: f64,
    stable: bool,
}

#[cfg(unix)]
struct Pty {
    master: std::fs::File,
    slave: std::fs::File,
    slave_name: String,
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
fn scenario_samples(name: &str) -> &'static [ScaleSample] {
    match name.trim().to_ascii_lowercase().as_str() {
        "idle" => &[
            ScaleSample {
                weight: 0.0,
                stable: true,
            },
            ScaleSample {
                weight: 0.002,
                stable: true,
            },
        ],
        "stress" => &[
            ScaleSample {
                weight: 0.0,
                stable: true,
            },
            ScaleSample {
                weight: 0.320,
                stable: false,
            },
            ScaleSample {
                weight: 1.870,
                stable: false,
            },
            ScaleSample {
                weight: 1.842,
                stable: true,
            },
        ],
        _ => &[
            ScaleSample {
                weight: 0.0,
                stable: true,
            },
            ScaleSample {
                weight: 1.250,
                stable: false,
            },
            ScaleSample {
                weight: 1.250,
                stable: true,
            },
            ScaleSample {
                weight: 0.0,
                stable: true,
            },
        ],
    }
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
