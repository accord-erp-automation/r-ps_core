pub const DEFAULT_QR_BASE_URL: &str = "https://scan.wspace.sbs/L/";

pub fn encode_scan_payload(
    company_name: &str,
    product_name: &str,
    kg_text: &str,
    brutto_text: &str,
    epc: &str,
) -> String {
    let parts = [company_name, product_name, kg_text, brutto_text, epc]
        .into_iter()
        .map(query_escape)
        .collect::<Vec<_>>();
    DEFAULT_QR_BASE_URL.to_string() + &parts.join("/")
}

fn query_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_scan_payload_like_gscale() {
        assert_eq!(
            encode_scan_payload(
                "Accord LLC",
                "Green Tea 1kg",
                "1.2",
                "5",
                "3034257BF7194E406994036B"
            ),
            "https://scan.wspace.sbs/L/Accord+LLC/Green+Tea+1kg/1.2/5/3034257BF7194E406994036B"
        );
    }

    #[test]
    fn query_escapes_utf8_and_reserved_chars_like_go() {
        assert_eq!(
            encode_scan_payload("A+B", "O'zbek чой", "1,2", "5 kg", "ABC/123"),
            "https://scan.wspace.sbs/L/A%2BB/O%27zbek+%D1%87%D0%BE%D0%B9/1%2C2/5+kg/ABC%2F123"
        );
    }
}
