use std::fs;
use std::path::{Path, PathBuf};

pub fn resolve_usblp_device_by_serial(serial: &str) -> Option<PathBuf> {
    resolve_usblp_device_by_serial_in(
        serial,
        Path::new("/sys/class/usbmisc"),
        Path::new("/dev/usb"),
    )
}

fn resolve_usblp_device_by_serial_in(
    serial: &str,
    sysfs_root: &Path,
    device_root: &Path,
) -> Option<PathBuf> {
    let serial = normalize_serial(serial)?;
    let entries = fs::read_dir(sysfs_root).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("lp") {
            continue;
        }
        let entry_path = entry.path();
        let device = entry_path.join("device");
        if !godex_vendor_product_match(&device) {
            continue;
        }
        let Some(candidate_serial) = read_trimmed(device.join("../serial")) else {
            continue;
        };
        if normalize_serial(&candidate_serial).as_deref() == Some(serial.as_str()) {
            return Some(device_root.join(name.as_ref()));
        }
    }
    None
}

fn godex_vendor_product_match(device: &Path) -> bool {
    let vendor = read_trimmed(device.join("../idVendor"));
    let product = read_trimmed(device.join("../idProduct"));
    matches!(
        (vendor.as_deref(), product.as_deref()),
        (Some("195f"), Some("0001"))
    )
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_serial(serial: &str) -> Option<String> {
    let serial = serial.trim();
    if serial.is_empty() {
        None
    } else {
        Some(serial.to_ascii_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolves_usblp_device_by_usb_serial_after_lp_order_changes() {
        let root =
            std::env::temp_dir().join(format!("rp-scale-usblp-serial-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let sysfs = root.join("sys/class/usbmisc");
        let devices = root.join("devices");
        fs::create_dir_all(&sysfs).unwrap();
        fs::create_dir_all(&devices).unwrap();
        add_lp(&sysfs, &devices, "lp0", "NEW-SERIAL");
        add_lp(&sysfs, &devices, "lp1", "OLD-SERIAL");

        let resolved =
            resolve_usblp_device_by_serial_in("OLD-SERIAL", &sysfs, Path::new("/dev/usb"));

        let _ = fs::remove_dir_all(&root);
        assert_eq!(resolved, Some(PathBuf::from("/dev/usb/lp1")));
    }

    #[test]
    fn resolves_usblp_device_by_usb_serial_when_case_differs() {
        let root =
            std::env::temp_dir().join(format!("rp-scale-usblp-serial-case-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let sysfs = root.join("sys/class/usbmisc");
        let devices = root.join("devices");
        fs::create_dir_all(&sysfs).unwrap();
        fs::create_dir_all(&devices).unwrap();
        add_lp(&sysfs, &devices, "lp0", "255109E1");

        let resolved = resolve_usblp_device_by_serial_in("255109e1", &sysfs, Path::new("/dev/usb"));

        let _ = fs::remove_dir_all(&root);
        assert_eq!(resolved, Some(PathBuf::from("/dev/usb/lp0")));
    }

    fn add_lp(sysfs: &Path, devices: &Path, lp: &str, serial: &str) {
        let usb = devices.join(format!("{lp}-usb"));
        let interface = usb.join("interface");
        fs::create_dir_all(&interface).unwrap();
        fs::write(usb.join("serial"), serial).unwrap();
        fs::write(usb.join("idVendor"), "195f").unwrap();
        fs::write(usb.join("idProduct"), "0001").unwrap();
        fs::create_dir_all(sysfs.join(lp)).unwrap();
        std::os::unix::fs::symlink(&interface, sysfs.join(lp).join("device")).unwrap();
    }
}
