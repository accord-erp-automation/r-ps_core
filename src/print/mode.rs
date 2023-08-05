#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintMode {
    Rfid,
    LabelOnly,
}

impl PrintMode {
    pub fn normalize(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "rfid" | "rfid-label" | "rfid_label" | "rfidprint" => Self::Rfid,
            "label" | "label-only" | "label_only" | "plain" | "plain-label" | "plain_label"
            | "simple" => Self::LabelOnly,
            _ => Self::Rfid,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rfid => "rfid",
            Self::LabelOnly => "label",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_gscale_print_modes() {
        assert_eq!(PrintMode::normalize(""), PrintMode::Rfid);
        assert_eq!(PrintMode::normalize("rfid_label"), PrintMode::Rfid);
        assert_eq!(PrintMode::normalize("label-only"), PrintMode::LabelOnly);
        assert_eq!(PrintMode::normalize("plain_label"), PrintMode::LabelOnly);
        assert_eq!(PrintMode::normalize("unknown"), PrintMode::Rfid);
    }
}
