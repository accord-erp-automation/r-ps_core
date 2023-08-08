use super::mode::PrintMode;
use super::printer::PrinterKind;

#[derive(Clone, Debug, PartialEq)]
pub struct PrinterCapabilities {
    pub id: &'static str,
    pub name: &'static str,
    pub thermal_label: bool,
    pub rfid_epc_write: bool,
    pub barcode: bool,
    pub qr: bool,
    pub verify_after_print: bool,
    pub required_fields: &'static [&'static str],
    pub unsupported_modes: &'static [&'static str],
}

impl PrinterCapabilities {
    pub fn supports_mode(&self, mode: PrintMode) -> bool {
        match mode {
            PrintMode::Rfid => self.rfid_epc_write,
            PrintMode::LabelOnly => self.thermal_label,
        }
    }
}

pub fn capabilities_for(kind: PrinterKind) -> PrinterCapabilities {
    match kind {
        PrinterKind::Zebra => zebra_capabilities(),
        PrinterKind::Godex => godex_capabilities(),
    }
}

pub fn zebra_capabilities() -> PrinterCapabilities {
    PrinterCapabilities {
        id: "zebra",
        name: "Zebra RFID",
        thermal_label: true,
        rfid_epc_write: true,
        barcode: true,
        qr: false,
        verify_after_print: true,
        required_fields: &["epc", "item_name", "weight"],
        unsupported_modes: &[],
    }
}

pub fn godex_capabilities() -> PrinterCapabilities {
    PrinterCapabilities {
        id: "godex",
        name: "GoDEX G500",
        thermal_label: true,
        rfid_epc_write: false,
        barcode: true,
        qr: true,
        verify_after_print: false,
        required_fields: &["epc", "item_name", "weight"],
        unsupported_modes: &["rfid_epc_write"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_zebra_capabilities() {
        let caps = capabilities_for(PrinterKind::Zebra);

        assert!(caps.supports_mode(PrintMode::Rfid));
        assert!(caps.supports_mode(PrintMode::LabelOnly));
        assert!(caps.rfid_epc_write);
        assert!(caps.verify_after_print);
    }

    #[test]
    fn exposes_godex_capabilities_and_blocks_rfid() {
        let caps = capabilities_for(PrinterKind::Godex);

        assert!(!caps.supports_mode(PrintMode::Rfid));
        assert!(caps.supports_mode(PrintMode::LabelOnly));
        assert_eq!(caps.unsupported_modes, &["rfid_epc_write"]);
        assert!(caps.qr);
        assert!(caps.barcode);
    }
}
