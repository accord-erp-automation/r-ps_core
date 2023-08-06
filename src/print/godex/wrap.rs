use super::text::sanitize_label_text;

pub fn wrap_text_for_ezpl(
    text: &str,
    width_dots: i32,
    x_mul: i32,
    pitch_dots: i32,
    min_chars: usize,
) -> Vec<String> {
    let text = sanitize_label_text(text);
    if text.is_empty() {
        return vec![String::new()];
    }
    let char_width = (pitch_dots * x_mul.max(1)).max(1);
    let width_chars = min_chars.max((width_dots / char_width).max(0) as usize);
    let lines = wrap_words_by_char_count(&text, width_chars, false);
    if lines.iter().any(|line| line.chars().count() > width_chars) {
        return wrap_words_by_char_count(&text, width_chars, true);
    }
    lines
}

fn wrap_words_by_char_count(text: &str, width: usize, break_long: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if candidate.chars().count() <= width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            lines.push(current);
        }
        if !break_long || word.chars().count() <= width {
            current = word.to_string();
            continue;
        }
        let mut chars = word.chars().collect::<Vec<_>>();
        while chars.len() > width {
            lines.push(chars[..width].iter().collect());
            chars = chars[width..].to_vec();
        }
        current = chars.iter().collect();
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        return vec![text.to_string()];
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words_like_gscale_ezpl() {
        assert_eq!(
            wrap_text_for_ezpl("Green Tea Premium Long Leaf", 192, 1, 14, 8),
            vec!["Green Tea", "Premium Long", "Leaf"]
        );
        assert_eq!(
            wrap_text_for_ezpl(
                "Super Extra Very Long Product Name For Wrap Test",
                192,
                1,
                14,
                8
            ),
            vec!["Super Extra", "Very Long", "Product Name", "For Wrap Test"]
        );
    }

    #[test]
    fn breaks_long_words_when_needed_like_gscale() {
        assert_eq!(
            wrap_text_for_ezpl("ABCDEFGHIJK", 70, 1, 14, 2),
            vec!["ABCDE", "FGHIJ", "K"]
        );
    }
}
