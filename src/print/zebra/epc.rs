pub fn normalize_epc(epc: &str) -> Result<String, String> {
    let mut value = epc.trim().to_ascii_uppercase();
    if let Some(stripped) = value.strip_prefix("0X") {
        value = stripped.to_string();
    }
    value = value.replace([' ', '-'], "");

    if value.is_empty() {
        return Err("epc bo'sh".to_string());
    }
    if !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("epc faqat hex bo'lishi kerak".to_string());
    }
    if !value.len().is_multiple_of(2) {
        return Err("epc uzunligi juft bo'lishi kerak".to_string());
    }
    if !value.len().is_multiple_of(4) {
        return Err("epc uzunligi 16-bit word (4 hex belgi) ga bo'linishi kerak".to_string());
    }
    if value.len() < 8 || value.len() > 64 {
        return Err("epc uzunligi 8..64 oralig'ida bo'lsin".to_string());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_epc_for_zebra() {
        assert_eq!(
            normalize_epc(" 0x3034 257b-f7194e406994036b ").unwrap(),
            "3034257BF7194E406994036B"
        );
    }

    #[test]
    fn rejects_invalid_epc_like_gscale() {
        assert_eq!(normalize_epc("").unwrap_err(), "epc bo'sh");
        assert_eq!(
            normalize_epc("3034257BF7194E40699403").unwrap_err(),
            "epc uzunligi 16-bit word (4 hex belgi) ga bo'linishi kerak"
        );
        assert_eq!(
            normalize_epc("3034257BF7194E40699403ZZ").unwrap_err(),
            "epc faqat hex bo'lishi kerak"
        );
    }
}
