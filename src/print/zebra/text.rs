pub fn sanitize_zpl_text(value: &str) -> String {
    value
        .replace(['\n', '\r', '^', '~'], " ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_zpl_control_characters() {
        assert_eq!(sanitize_zpl_text(" A^B~C\r\n "), "A B C");
    }
}
