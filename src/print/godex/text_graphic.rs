use crate::core::PackLabelContent;

use super::bmp::{MonoBitmap, encode_mono_bmp};
use super::options::{LabelOptions, mm_dots};

pub fn render_pack_text_graphic(content: &PackLabelContent, options: &LabelOptions) -> Vec<u8> {
    let options = options.clone().normalized_pack();
    let label_width = mm_dots(f64::from(options.label_width_mm), options.dpi).max(1) as usize;
    let label_length = mm_dots(f64::from(options.label_length_mm), options.dpi).max(1) as usize;
    let safe_margin = mm_dots(options.safe_margin_mm, options.dpi);
    let line_step = mm_dots(5.0, options.dpi);
    let left_x = 0.max(safe_margin - mm_dots(2.0, options.dpi));

    let mut company_y = safe_margin + line_step * 2;
    let mut item_y = company_y + line_step;
    let mut qty_y = mm_dots(33.0, options.dpi);
    let epc_y = 0.max(safe_margin - line_step * 5);
    company_y = 0.max(company_y - mm_dots(5.0, options.dpi));
    item_y = 0.max(item_y - mm_dots(5.0, options.dpi));
    qty_y = 0.max(qty_y - mm_dots(3.0, options.dpi));
    let brutto_y = 0.max(qty_y + line_step);

    let mut canvas = MonoBitmap::filled(label_width, label_length, true);
    draw_text(
        &mut canvas,
        left_x,
        epc_y,
        2,
        &format!("EPC: {}", content.epc),
    );
    draw_text(
        &mut canvas,
        left_x,
        company_y,
        3,
        &format!("COMPANY: {}", content.company_name),
    );
    draw_wrapped_product(&mut canvas, left_x, item_y, &content.product_name);
    draw_text(
        &mut canvas,
        left_x,
        qty_y,
        3,
        &format!("NETTO: {} KG", content.kg_text),
    );
    draw_text(
        &mut canvas,
        left_x,
        brutto_y,
        3,
        &format!("BRUTTO: {} KG", content.brutto_text),
    );

    encode_mono_bmp(&canvas.crop_ink())
}

pub fn render_pack_epc_graphic(content: &PackLabelContent, options: &LabelOptions) -> Vec<u8> {
    let options = options.clone().normalized_pack();
    let label_width = mm_dots(f64::from(options.label_width_mm), options.dpi).max(1) as usize;
    let label_length = mm_dots(f64::from(options.label_length_mm), options.dpi).max(1) as usize;
    let safe_margin = mm_dots(options.safe_margin_mm, options.dpi);
    let line_step = mm_dots(5.0, options.dpi);
    let left_x = 0.max(safe_margin - mm_dots(2.0, options.dpi));
    let epc_y = 0.max(safe_margin - line_step * 5);

    let mut canvas = MonoBitmap::filled(label_width, label_length, true);
    draw_text(
        &mut canvas,
        left_x,
        epc_y,
        2,
        &format!("EPC: {}", content.epc),
    );
    encode_mono_bmp(&canvas.crop_ink())
}

pub fn render_qolip_cell_name_graphic(name: &str, options: &LabelOptions) -> Vec<u8> {
    let options = options.clone().normalized_pack();
    let label_width = mm_dots(f64::from(options.label_width_mm), options.dpi).max(1) as usize;
    let mut canvas = MonoBitmap::filled(label_width, 96, true);
    let name = sanitize_label_text(name).to_ascii_uppercase();
    let scale = 8;
    let text_width = name.chars().count() as i32 * 6 * scale;
    let x = ((label_width as i32 - text_width) / 2).max(0);
    draw_text(&mut canvas, x, 16, scale as usize, &name);
    encode_mono_bmp(&canvas.crop_ink())
}

pub fn render_qolip_code_text_graphic(
    name: &str,
    code: &str,
    options: &LabelOptions,
) -> Vec<u8> {
    let options = options.clone().normalized_pack();
    let label_width = mm_dots(f64::from(options.label_width_mm), options.dpi).max(1) as usize;
    let mut canvas = MonoBitmap::filled(label_width, 400, true);
    let name = sanitize_label_text(name).to_ascii_uppercase();
    let code = sanitize_label_text(code).to_ascii_uppercase();
    draw_centered_text(&mut canvas, &name, 8, 3);
    draw_centered_text(&mut canvas, &code, 352, 4);
    encode_mono_bmp(&canvas.crop_ink())
}

fn draw_centered_text(canvas: &mut MonoBitmap, text: &str, y: i32, scale: usize) {
    let text_width = text.chars().count() as i32 * 6 * scale as i32;
    let x = ((canvas.width() as i32 - text_width) / 2).max(0);
    draw_text(canvas, x, y, scale, text);
}

fn draw_wrapped_product(canvas: &mut MonoBitmap, x: i32, y: i32, product_name: &str) {
    let prefix = "MAHSULOT NOMI:";
    let mut line = format!("{prefix} {product_name}");
    let max_chars = 24;
    let mut line_idx = 0_i32;
    while !line.is_empty() {
        let split = split_line(&line, max_chars);
        draw_text(canvas, x, y + line_idx * 28, 3, split.0);
        line = split.1.trim_start().to_string();
        line_idx += 1;
        if line_idx > 6 {
            break;
        }
    }
}

fn split_line(text: &str, max_chars: usize) -> (&str, &str) {
    if text.chars().count() <= max_chars {
        return (text, "");
    }
    let mut end = 0;
    for (idx, _) in text.char_indices().take(max_chars + 1) {
        end = idx;
    }
    if let Some(space) = text[..end].rfind(' ') {
        (&text[..space], &text[space + 1..])
    } else {
        (&text[..end], &text[end..])
    }
}

fn draw_text(canvas: &mut MonoBitmap, x: i32, y: i32, scale: usize, text: &str) {
    let mut cursor = x;
    for ch in text.chars() {
        draw_char(canvas, cursor, y, scale, ch);
        cursor += (6 * scale) as i32;
    }
}

fn draw_char(canvas: &mut MonoBitmap, x: i32, y: i32, scale: usize, ch: char) {
    if ch == ' ' {
        return;
    }
    let glyph = glyph_rows(ch.to_ascii_uppercase());
    for (row_idx, row) in glyph.iter().enumerate() {
        for col in 0..5 {
            if row & (0b10000 >> col) == 0 {
                continue;
            }
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x + (col * scale + dx) as i32;
                    let py = y + (row_idx * scale + dy) as i32;
                    if px >= 0 && py >= 0 {
                        canvas.set_light(px as usize, py as usize, false);
                    }
                }
            }
        }
    }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '\'' => [4, 4, 8, 0, 0, 0, 0],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        _ => [31, 17, 21, 21, 21, 17, 31],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> PackLabelContent {
        PackLabelContent {
            company_name: "ACCORD".to_string(),
            product_name: "GREEN TEA".to_string(),
            kg_text: "1.3".to_string(),
            brutto_text: "5".to_string(),
            epc: "3034257BF7194E406994036B".to_string(),
            qr_payload: "payload".to_string(),
        }
    }

    #[test]
    fn renders_pack_text_graphic_as_bmp() {
        let bmp = render_pack_text_graphic(&content(), &LabelOptions::default_pack());

        assert_eq!(&bmp[0..2], b"BM");
        assert!(bmp.len() > 62);
        assert!(bmp[62..].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn renders_pack_epc_graphic_as_bmp() {
        let bmp = render_pack_epc_graphic(&content(), &LabelOptions::default_pack());

        assert_eq!(&bmp[0..2], b"BM");
        assert!(bmp.len() > 62);
        assert!(bmp[62..].iter().any(|byte| *byte != 0));
    }
}
