use unicode_normalization::UnicodeNormalization;

pub fn sanitize_label_text(value: &str) -> String {
    let normalized: String = value.nfkc().collect();
    let replaced = normalized.replace(['\r', '\n', '^', '~'], " ");
    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_kg_value(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_label_text_like_gscale() {
        assert_eq!(sanitize_label_text("  A^B~C\r\n  D   E  "), "A B C D E");
    }

    #[test]
    fn normalizes_kg_value_like_gscale() {
        assert_eq!(normalize_kg_value("kg: 1,26"), "1.3");
        assert_eq!(normalize_kg_value("1.24kg"), "1.2");
        assert_eq!(normalize_kg_value("2.05"), "2.1");
        assert_eq!(normalize_kg_value("abc kg"), "abc");
        assert_eq!(normalize_kg_value(""), "");
    }
}
