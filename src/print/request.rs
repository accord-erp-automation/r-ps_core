use super::mode::PrintMode;
use super::printer::PrinterKind;

#[derive(Clone, Debug, PartialEq)]
pub struct PrintRequest {
    pub epc: String,
    pub qty: Option<f64>,
    pub gross_qty: Option<f64>,
    pub unit: String,
    pub item_code: String,
    pub item_name: String,
    pub mode: PrintMode,
    pub printer: Option<PrinterKind>,
    pub tare: bool,
    pub tare_kg: f64,
}

impl PrintRequest {
    pub fn normalized(mut self) -> Self {
        self.epc = self.epc.trim().to_ascii_uppercase();
        self.unit = self.unit.trim().to_string();
        if self.unit.is_empty() {
            self.unit = "kg".to_string();
        }
        self.item_code = self.item_code.trim().to_string();
        self.item_name = self.item_name.trim().to_string();
        if self.item_name.is_empty() {
            self.item_name = self.item_code.clone();
        }
        if !self.tare || self.tare_kg <= 0.0 {
            self.tare = false;
            self.tare_kg = 0.0;
        }
        self
    }

    pub fn item_label(&self) -> &str {
        if self.item_name.trim().is_empty() {
            self.item_code.trim()
        } else {
            self.item_name.trim()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_print_request_fields() {
        let request = PrintRequest {
            epc: " abc123 ".to_string(),
            qty: Some(1.0),
            gross_qty: Some(2.0),
            unit: "".to_string(),
            item_code: " ITM-1 ".to_string(),
            item_name: " ".to_string(),
            mode: PrintMode::Rfid,
            printer: PrinterKind::normalize_request("zebra"),
            tare: true,
            tare_kg: 0.0,
        }
        .normalized();

        assert_eq!(request.epc, "ABC123");
        assert_eq!(request.unit, "kg");
        assert_eq!(request.item_name, "ITM-1");
        assert!(!request.tare);
        assert_eq!(request.tare_kg, 0.0);
    }
}
