use super::request::PrintRequest;

#[derive(Clone, Debug, PartialEq)]
pub struct PrintWeightLabels {
    pub netto: String,
    pub brutto: String,
    pub has_tare: bool,
}

pub fn format_print_weight_labels(request: &PrintRequest) -> PrintWeightLabels {
    let net_qty = request.qty;
    let gross_qty = request.gross_qty.or(request.qty);

    if !request.tare || request.tare_kg <= 0.0 {
        let label = format_label_qty(gross_qty, &request.unit);
        return PrintWeightLabels {
            netto: label.clone(),
            brutto: label,
            has_tare: false,
        };
    }

    let Some(gross) = gross_qty else {
        let label = format_label_qty(net_qty, &request.unit);
        return PrintWeightLabels {
            netto: label.clone(),
            brutto: label,
            has_tare: false,
        };
    };
    let net = net_qty.unwrap_or_else(|| (gross - request.tare_kg).max(0.0));

    PrintWeightLabels {
        netto: format_trimmed_qty(net, &request.unit),
        brutto: format_trimmed_qty(gross, &request.unit),
        has_tare: true,
    }
}

pub fn format_label_qty(qty: Option<f64>, unit: &str) -> String {
    let normalized_unit = normalized_unit(unit);
    match qty {
        Some(value) => format!("{} {}", format_rounded_qty(value), normalized_unit),
        None => format!("- {}", normalized_unit),
    }
}

fn format_trimmed_qty(qty: f64, unit: &str) -> String {
    format!("{} {}", format_rounded_qty(qty), normalized_unit(unit))
}

fn format_rounded_qty(qty: f64) -> String {
    trim_float((qty * 10.0).round() / 10.0)
}

fn normalized_unit(unit: &str) -> String {
    let unit = unit.trim();
    if unit.is_empty() {
        "kg".to_string()
    } else {
        unit.to_string()
    }
}

fn trim_float(value: f64) -> String {
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
    use crate::print::mode::PrintMode;

    fn request(qty: Option<f64>, gross_qty: Option<f64>, tare: bool, tare_kg: f64) -> PrintRequest {
        PrintRequest {
            epc: "EPC".to_string(),
            qty,
            gross_qty,
            unit: "kg".to_string(),
            item_code: "ITM".to_string(),
            item_name: "Item".to_string(),
            mode: PrintMode::Rfid,
            printer: None,
            tare,
            tare_kg,
        }
    }

    #[test]
    fn formats_label_qty_like_gscale() {
        assert_eq!(format_label_qty(Some(1.72), "kg"), "1.7 kg");
        assert_eq!(format_label_qty(Some(2.0), "kg"), "2 kg");
        assert_eq!(format_label_qty(None, ""), "- kg");
    }

    #[test]
    fn formats_no_tare_as_same_netto_and_brutto() {
        let labels = format_print_weight_labels(&request(Some(1.72), Some(2.5), false, 0.78));

        assert_eq!(labels.netto, "2.5 kg");
        assert_eq!(labels.brutto, "2.5 kg");
        assert!(!labels.has_tare);
    }

    #[test]
    fn formats_tare_netto_and_brutto() {
        let labels = format_print_weight_labels(&request(Some(1.72), Some(2.5), true, 0.78));

        assert_eq!(labels.netto, "1.7 kg");
        assert_eq!(labels.brutto, "2.5 kg");
        assert!(labels.has_tare);
    }
}
