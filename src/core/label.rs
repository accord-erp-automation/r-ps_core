use unicode_normalization::UnicodeNormalization;

use super::job::CorePrintJob;

pub const DEFAULT_QR_BASE_URL: &str = "https://scan.wspace.sbs/L/";

#[derive(Clone, Debug, PartialEq)]
pub struct PackLabelContent {
    pub company_name: String,
    pub product_name: String,
    pub kg_text: String,
    pub brutto_text: String,
    pub epc: String,
    pub qr_payload: String,
}

pub fn build_pack_label_content(
    job: &CorePrintJob,
    company_name: &str,
    _default_brutto_text: &str,
) -> Result<PackLabelContent, String> {
    let company_name = uppercase_clean(company_name);
    let product_name = uppercase_clean(product_name_or_epc(job));
    let kg_text = normalize_kg_value(&job.net_qty.to_string());
    let brutto_text = normalize_brutto_text(job);
    let epc = uppercase_clean(&job.epc);

    if company_name.is_empty() || product_name.is_empty() || kg_text.is_empty() || epc.is_empty() {
        return Err("company, product, kg, and epc are required".to_string());
    }

    let qr_payload = epc.clone();
    Ok(PackLabelContent {
        company_name,
        product_name,
        kg_text,
        brutto_text,
        epc,
        qr_payload,
    })
}

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

fn product_name_or_epc(job: &CorePrintJob) -> &str {
    let item = job.item_name.trim();
    if item.is_empty() {
        job.epc.trim()
    } else {
        item
    }
}

fn normalize_brutto_text(job: &CorePrintJob) -> String {
    if job.tare {
        return normalize_kg_value(&job.gross_qty.to_string());
    }
    normalize_kg_value(&job.net_qty.to_string())
}

fn uppercase_clean(value: &str) -> String {
    sanitize_label_text(value).to_ascii_uppercase()
}

fn sanitize_label_text(value: &str) -> String {
    let normalized = value.nfkc().collect::<String>();
    let replaced = normalized.replace(['\r', '\n', '^', '~'], " ");
    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_kg_value(text: &str) -> String {
    let mut value = sanitize_label_text(text);
    let lowered = value.to_ascii_lowercase();
    if let Some(rest) = lowered.strip_prefix("kg:") {
        let start = value.len() - rest.len();
        value = value[start..].trim().to_string();
    } else if lowered.ends_with("kg") {
        value = value[..value.len() - 2].trim().to_string();
    }

    round_kg_text(&value).unwrap_or(value)
}

fn round_kg_text(text: &str) -> Option<String> {
    let value = text.trim().replace(',', ".");
    if value.is_empty() {
        return None;
    }
    let parsed = value.parse::<f64>().ok()?;
    Some(format_go_float((parsed * 10.0).round() / 10.0))
}

fn format_go_float(value: f64) -> String {
    let mut text = format!("{value:.1}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
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
    use crate::core::{PrintSelection, QuantitySource};
    use crate::print::mode::PrintMode;

    fn job(tare: bool) -> CorePrintJob {
        CorePrintJob::from_selection(
            "3034257BF7194E406994036B",
            1.26,
            2.5,
            "kg",
            PrintSelection {
                item_code: "ITEM-1".to_string(),
                item_name: "Green Tea".to_string(),
                warehouse: "Stores - A".to_string(),
                print_mode: PrintMode::LabelOnly,
                printer: "godex".to_string(),
                quantity_source: QuantitySource::Scale,
                manual_qty_kg: 0.0,
                tare_enabled: tare,
                tare_kg: 1.24,
            },
        )
    }

    #[test]
    fn builds_pack_label_content_like_godex_pack_input() {
        let content = build_pack_label_content(&job(false), "Accord LLC", "5kg").unwrap();

        assert_eq!(content.company_name, "ACCORD LLC");
        assert_eq!(content.product_name, "GREEN TEA");
        assert_eq!(content.kg_text, "1.3");
        assert_eq!(content.brutto_text, content.kg_text);
        assert_eq!(content.epc, "3034257BF7194E406994036B");
        assert_eq!(content.qr_payload, "3034257BF7194E406994036B");
    }

    #[test]
    fn uses_gross_qty_as_brutto_when_job_has_tare() {
        let content = build_pack_label_content(&job(true), "Accord", "").unwrap();

        assert_eq!(content.kg_text, "1.3");
        assert_eq!(content.brutto_text, "2.5");
    }

    #[test]
    fn falls_back_product_name_to_epc_like_godex_action() {
        let mut job = job(false);
        job.item_name = " ".to_string();

        let content = build_pack_label_content(&job, "Accord", "").unwrap();

        assert_eq!(content.product_name, "3034257BF7194E406994036B");
        assert_eq!(content.brutto_text, content.kg_text);
    }

    #[test]
    fn query_escapes_scan_payload_like_go() {
        assert_eq!(
            encode_scan_payload("A+B", "O'zbek чой", "1,2", "5 kg", "ABC/123"),
            "https://scan.wspace.sbs/L/A%2BB/O%27zbek+%D1%87%D0%BE%D0%B9/1%2C2/5+kg/ABC%2F123"
        );
    }
}
