use super::options::{LabelOptions, mm_dots};
use super::text::{normalize_kg_value, sanitize_label_text};
use super::wrap::wrap_text_for_ezpl;

pub fn build_direct_pack_label(
    company_name: &str,
    product_name: &str,
    qty_text: &str,
    barcode: &str,
    batch_code: &str,
    qr_payload: &str,
    options: LabelOptions,
) -> Vec<String> {
    let options = options.normalized_simple();
    let company_name = sanitize_label_text(company_name);
    let product_name = sanitize_label_text(product_name);
    let qty_text = normalize_kg_value(qty_text);
    let barcode = sanitize_label_text(barcode);
    let batch_code = sanitize_label_text(batch_code);
    let mut qr_payload = sanitize_label_text(qr_payload);
    if qr_payload.is_empty() {
        qr_payload = batch_code.clone();
    }

    let label_width_dots = mm_dots(f64::from(options.label_width_mm), options.dpi);
    let label_length_dots = mm_dots(f64::from(options.label_length_mm), options.dpi);
    let safe_margin_dots = mm_dots(options.safe_margin_mm, options.dpi);
    let left_x = safe_margin_dots;
    let gap_dots = mm_dots(3.0, options.dpi);
    let line_step = mm_dots(5.0, options.dpi);
    let company_y = safe_margin_dots;
    let item_y = company_y + line_step;
    let qr_box_mm = 16.0_f64.max(20.0_f64.min(f64::from(options.label_width_mm) * 0.30));
    let qr_box_dots = mm_dots(qr_box_mm, options.dpi);
    let mut qr_x = left_x.max(label_width_dots - safe_margin_dots - qr_box_dots);
    qr_x = (label_width_dots - qr_box_dots).min(qr_x + mm_dots(1.0, options.dpi));
    let barcode_y = (item_y + line_step * 3)
        .max(label_length_dots - safe_margin_dots - mm_dots(12.0, options.dpi));
    let qr_mul = 5;
    let text_width_dots = 1.max(qr_x - left_x - gap_dots);
    let product_lines = wrap_text_for_ezpl(&product_name, text_width_dots, 1, 14, 8);
    let qty_y = item_y + product_lines.len() as i32 * line_step;
    let qr_y = (safe_margin_dots + line_step * 2).max(qty_y + line_step);
    let barcode_text_y = barcode_y + mm_dots(8.0, options.dpi);
    let barcode_text_x_mul = 2;
    let barcode_text_width_dots = 1.max(barcode.len() as i32 * 14 * barcode_text_x_mul);
    let barcode_text_x = left_x.max(
        left_x + ((label_width_dots - left_x - safe_margin_dots) - barcode_text_width_dots) / 2,
    );

    let mut commands = vec![
        "~S,ESG".to_string(),
        "^AD".to_string(),
        "^XSET,IMMEDIATE,1".to_string(),
        "^XSET,ACTIVERESPONSE,1".to_string(),
        "^XSET,CODEPAGE,16".to_string(),
        format!("^Q{},{}", options.label_length_mm, options.label_gap_mm),
        format!("^W{}", options.label_width_mm),
        "^H10".to_string(),
        "^P1".to_string(),
        "^L".to_string(),
        format!("AC,{left_x},{company_y},1,1,0,0,company name: {company_name}"),
        format!(
            "AC,{},{},1,1,0,0,company name: {}",
            left_x + 1,
            company_y + 1,
            company_name
        ),
        format!(
            "AC,{left_x},{item_y},1,1,0,0,item name: {}",
            product_lines[0]
        ),
        format!("AC,{left_x},{qty_y},1,1,0,0,kg: {qty_text}"),
    ];
    for (idx, line) in product_lines.iter().skip(1).enumerate() {
        commands.push(format!(
            "AC,{},{},1,1,0,0,{}",
            left_x,
            item_y + (idx as i32 + 1) * line_step,
            line
        ));
    }
    commands.push(format!("BA,{left_x},{barcode_y},1,2,42,0,0,{barcode}"));
    commands.push(format!(
        "AC,{barcode_text_x},{barcode_text_y},{barcode_text_x_mul},1,0,0,{barcode}"
    ));
    commands.push(format!(
        "W{qr_x},{qr_y},2,1,L,8,{qr_mul},{},0",
        qr_payload.len()
    ));
    commands.push(qr_payload);
    if !batch_code.is_empty() {
        let batch_y = (label_length_dots - safe_margin_dots).min(barcode_text_y + line_step);
        commands.push(format!("AC,{left_x},{batch_y},1,1,0,0,{batch_code}"));
    }
    commands.push("E".to_string());
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_direct_pack_label_like_gscale() {
        let commands = build_direct_pack_label(
            "Accord LLC",
            "Green Tea Premium Long Leaf",
            "kg: 1,26",
            "3034257BF7194E406994036B",
            "BATCH-001",
            "",
            LabelOptions::default(),
        );

        assert_eq!(
            commands,
            vec![
                "~S,ESG",
                "^AD",
                "^XSET,IMMEDIATE,1",
                "^XSET,ACTIVERESPONSE,1",
                "^XSET,CODEPAGE,16",
                "^Q25,3",
                "^W50",
                "^H10",
                "^P1",
                "^L",
                "AC,32,32,1,1,0,0,company name: Accord LLC",
                "AC,33,33,1,1,0,0,company name: Accord LLC",
                "AC,32,72,1,1,0,0,item name: Green Tea",
                "AC,32,192,1,1,0,0,kg: 1.3",
                "AC,32,112,1,1,0,0,Premium Long",
                "AC,32,152,1,1,0,0,Leaf",
                "BA,32,192,1,2,42,0,0,3034257BF7194E406994036B",
                "AC,32,256,2,1,0,0,3034257BF7194E406994036B",
                "W248,232,2,1,L,8,5,9,0",
                "BATCH-001",
                "AC,32,168,1,1,0,0,BATCH-001",
                "E",
            ]
        );
    }

    #[test]
    fn builds_wrapped_direct_pack_label_like_gscale() {
        let commands = build_direct_pack_label(
            "AC",
            "Super Extra Very Long Product Name For Wrap Test",
            "2.05kg",
            "ABC123",
            "",
            "QR PAYLOAD",
            LabelOptions::default(),
        );

        assert_eq!(commands[12], "AC,32,72,1,1,0,0,item name: Super Extra");
        assert_eq!(commands[13], "AC,32,232,1,1,0,0,kg: 2.1");
        assert_eq!(commands[17], "BA,32,192,1,2,42,0,0,ABC123");
        assert_eq!(commands[18], "AC,116,256,2,1,0,0,ABC123");
        assert_eq!(commands[19], "W248,272,2,1,L,8,5,10,0");
        assert_eq!(commands.last().unwrap(), "E");
    }
}
