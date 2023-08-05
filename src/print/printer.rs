#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrinterKind {
    Zebra,
    Godex,
}

impl PrinterKind {
    pub fn normalize_request(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "zebra" | "zpl" | "rfid" => Some(Self::Zebra),
            "godex" | "go-dex" | "g500" => Some(Self::Godex),
            _ => None,
        }
    }

    pub fn normalize_backend(value: &str) -> Self {
        Self::normalize_request(value).unwrap_or(Self::Zebra)
    }

    pub fn resolve(request_printer: &str, default_backend: &str) -> Self {
        Self::normalize_request(request_printer)
            .unwrap_or_else(|| Self::normalize_backend(default_backend))
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zebra => "zebra",
            Self::Godex => "godex",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_request_printer_like_gscale() {
        assert_eq!(
            PrinterKind::normalize_request("zpl"),
            Some(PrinterKind::Zebra)
        );
        assert_eq!(
            PrinterKind::normalize_request("rfid"),
            Some(PrinterKind::Zebra)
        );
        assert_eq!(
            PrinterKind::normalize_request("g500"),
            Some(PrinterKind::Godex)
        );
        assert_eq!(PrinterKind::normalize_request("unknown"), None);
    }

    #[test]
    fn resolves_request_printer_over_default_backend() {
        assert_eq!(PrinterKind::resolve("godex", "zebra"), PrinterKind::Godex);
        assert_eq!(PrinterKind::resolve("", "go-dex"), PrinterKind::Godex);
        assert_eq!(PrinterKind::resolve("", "unknown"), PrinterKind::Zebra);
    }
}
