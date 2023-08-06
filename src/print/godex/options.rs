#[derive(Clone, Debug, PartialEq)]
pub struct LabelOptions {
    pub label_length_mm: i32,
    pub label_gap_mm: i32,
    pub label_width_mm: i32,
    pub dpi: i32,
    pub safe_margin_mm: f64,
    pub qr_box_mm: f64,
}

impl LabelOptions {
    pub fn default_pack() -> Self {
        Self {
            label_length_mm: 50,
            label_gap_mm: 3,
            label_width_mm: 50,
            dpi: 203,
            safe_margin_mm: 4.0,
            qr_box_mm: 18.0,
        }
    }

    pub fn default_simple() -> Self {
        Self {
            label_length_mm: 25,
            label_gap_mm: 3,
            label_width_mm: 50,
            dpi: 203,
            safe_margin_mm: 0.0,
            qr_box_mm: 35.0,
        }
    }

    pub fn normalized_pack(mut self) -> Self {
        let defaults = Self::default_pack();
        if self.label_length_mm <= 0 {
            self.label_length_mm = defaults.label_length_mm;
        }
        if self.label_gap_mm <= 0 {
            self.label_gap_mm = defaults.label_gap_mm;
        }
        if self.label_width_mm <= 0 {
            self.label_width_mm = defaults.label_width_mm;
        }
        if self.dpi <= 0 {
            self.dpi = defaults.dpi;
        }
        if self.safe_margin_mm <= 0.0 {
            self.safe_margin_mm = defaults.safe_margin_mm;
        }
        if self.qr_box_mm <= 0.0 {
            self.qr_box_mm = defaults.qr_box_mm;
        }
        self
    }

    pub fn normalized_simple(mut self) -> Self {
        let defaults = Self::default_simple();
        if self.label_length_mm <= 0 {
            self.label_length_mm = defaults.label_length_mm;
        }
        if self.label_gap_mm <= 0 {
            self.label_gap_mm = defaults.label_gap_mm;
        }
        if self.label_width_mm <= 0 {
            self.label_width_mm = defaults.label_width_mm;
        }
        if self.dpi <= 0 {
            self.dpi = defaults.dpi;
        }
        if self.safe_margin_mm <= 0.0 {
            self.safe_margin_mm = 4.0;
        }
        self
    }
}

impl Default for LabelOptions {
    fn default() -> Self {
        Self::default_simple()
    }
}

pub fn mm_dots(mm: f64, dpi: i32) -> i32 {
    (mm * f64::from(dpi) / 25.4 + 0.5) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_simple_options_like_gscale() {
        let options = LabelOptions::default_simple().normalized_simple();

        assert_eq!(options.label_length_mm, 25);
        assert_eq!(options.label_gap_mm, 3);
        assert_eq!(options.label_width_mm, 50);
        assert_eq!(options.dpi, 203);
        assert_eq!(options.safe_margin_mm, 4.0);
    }

    #[test]
    fn normalizes_pack_options_like_gscale() {
        let options = LabelOptions::default_pack().normalized_pack();

        assert_eq!(options.label_length_mm, 50);
        assert_eq!(options.label_gap_mm, 3);
        assert_eq!(options.label_width_mm, 50);
        assert_eq!(options.dpi, 203);
        assert_eq!(options.safe_margin_mm, 4.0);
        assert_eq!(options.qr_box_mm, 18.0);
    }

    #[test]
    fn converts_mm_to_dots_like_gscale() {
        assert_eq!(mm_dots(4.0, 203), 32);
        assert_eq!(mm_dots(3.0, 203), 24);
        assert_eq!(mm_dots(12.0, 203), 96);
    }
}
