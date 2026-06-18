use unicode_normalization::UnicodeNormalization;

use super::job::CorePrintJob;

#[derive(Clone, Debug, PartialEq)]
pub struct ProgressLabelContent {
    pub company_name: String,
    pub product_name: String,
    pub qty_text: String,
    pub executor_name: String,
    pub epc: String,
    pub qr_payload: String,
}

pub fn build_progress_label_content(
    job: &CorePrintJob,
    company_name: &str,
) -> Result<ProgressLabelContent, String> {
    let company_name = uppercase_clean(company_name);
    let product_name = uppercase_clean(product_name_or_epc(job));
    let qty_text = progress_qty_text(job);
    let executor_name = uppercase_clean(&job.executor_name);
    let epc = uppercase_clean(&job.epc);

    if company_name.is_empty() || product_name.is_empty() || qty_text.is_empty() || epc.is_empty() {
        return Err("company, product, qty, and epc are required".to_string());
    }

    let qr_payload = epc.clone();
    Ok(ProgressLabelContent {
        company_name,
        product_name,
        qty_text,
        executor_name,
        epc,
        qr_payload,
    })
}

fn product_name_or_epc(job: &CorePrintJob) -> &str {
    let item = job.item_name.trim();
    if item.is_empty() {
        job.epc.trim()
    } else {
        item
    }
}

fn progress_qty_text(job: &CorePrintJob) -> String {
    let qty = normalize_qty_value(&job.net_qty.to_string());
    let unit = uppercase_clean(&job.unit);
    if unit.is_empty() {
        qty
    } else {
        format!("{qty} {unit}")
    }
}

fn uppercase_clean(value: &str) -> String {
    sanitize_label_text(value).to_ascii_uppercase()
}

fn sanitize_label_text(value: &str) -> String {
    let normalized = value.nfkc().collect::<String>();
    let replaced = normalized.replace(['\r', '\n', '^', '~'], " ");
    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_qty_value(text: &str) -> String {
    let value = sanitize_label_text(text);
    round_qty_text(&value).unwrap_or(value)
}

fn round_qty_text(text: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{PrintSelection, QuantitySource};
    use crate::print::mode::PrintMode;

    fn job() -> CorePrintJob {
        let mut job = CorePrintJob::from_selection(
            "400100000000000000000001",
            120.0,
            120.0,
            "m",
            PrintSelection {
                item_code: "ORDER-1202".to_string(),
                item_name: "Vesta yarim tayyor, pechat holatda, pauza".to_string(),
                warehouse: "Ijrochi: Ali".to_string(),
                print_mode: PrintMode::LabelOnly,
                printer: "godex".to_string(),
                quantity_source: QuantitySource::Scale,
                manual_qty_kg: 0.0,
                tare_enabled: false,
                tare_kg: 0.0,
            },
        );
        job.executor_name = "Ali".to_string();
        job.label_kind = "progress".to_string();
        job
    }

    #[test]
    fn builds_progress_label_content_with_epc_only_qr_payload() {
        let content = build_progress_label_content(&job(), "Accord").unwrap();

        assert_eq!(content.company_name, "ACCORD");
        assert_eq!(
            content.product_name,
            "VESTA YARIM TAYYOR, PECHAT HOLATDA, PAUZA"
        );
        assert_eq!(content.qty_text, "120 M");
        assert_eq!(content.executor_name, "ALI");
        assert_eq!(content.epc, "400100000000000000000001");
        assert_eq!(content.qr_payload, "400100000000000000000001");
    }
}
