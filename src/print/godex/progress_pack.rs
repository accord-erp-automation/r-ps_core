use crate::core::{PackLabelContent, ProgressLabelContent};

use super::options::{LabelOptions, mm_dots};
use super::pack::GodexPackRender;
use super::qr::render_qr_graphic;
use super::text::sanitize_label_text;
use super::text_graphic::render_pack_epc_graphic;
use super::wrap::wrap_text_for_ezpl;

const TEXT_GRAPHIC_NAME: &str = "TEXTLBL";
const QR_GRAPHIC_NAME: &str = "QRLBL";

pub fn build_progress_pack_render(
    content: &ProgressLabelContent,
    options: LabelOptions,
) -> Result<GodexPackRender, String> {
    let options = options.normalized_pack();
    let layout = compute_progress_layout(&options);
    let text_graphic_bmp = render_progress_epc_graphic(content, &options);
    let qr_graphic_bmp = render_qr_graphic(&content.qr_payload, layout.qr_box_dots)?;

    let mut commands = vec![
        "~S,ESG".to_string(),
        "^AD".to_string(),
        "^XSET,UNICODE,1".to_string(),
        "^XSET,IMMEDIATE,1".to_string(),
        "^XSET,ACTIVERESPONSE,1".to_string(),
        "^XSET,CODEPAGE,16".to_string(),
        format!("^Q{},{}", options.label_length_mm, options.label_gap_mm),
        format!("^W{}", options.label_width_mm),
        "^H10".to_string(),
        "^P1".to_string(),
        "^L".to_string(),
    ];
    commands.push(format!("Y0,0,{TEXT_GRAPHIC_NAME}"));
    commands.extend(build_native_text_commands(content, &options, &layout));
    commands.extend([
        format!(
            "BA,{},{},1,2,42,0,0,{}",
            layout.barcode_x, layout.barcode_y, content.epc
        ),
        format!("Y{},{},{}", layout.qr_x, layout.qr_y, QR_GRAPHIC_NAME),
        "E".to_string(),
    ]);

    Ok(GodexPackRender {
        commands,
        qr_payload: content.qr_payload.clone(),
        text_graphic_bmp,
        qr_graphic_bmp,
        text_graphic_name: TEXT_GRAPHIC_NAME.to_string(),
        qr_graphic_name: QR_GRAPHIC_NAME.to_string(),
        qr_box_dots: layout.qr_box_dots,
    })
}

fn render_progress_epc_graphic(content: &ProgressLabelContent, options: &LabelOptions) -> Vec<u8> {
    render_pack_epc_graphic(
        &PackLabelContent {
            company_name: content.company_name.clone(),
            product_name: content.product_name.clone(),
            kg_text: content.kg_text.clone(),
            brutto_text: content.qty_text.clone(),
            epc: content.epc.clone(),
            qr_payload: content.qr_payload.clone(),
        },
        options,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ProgressLayout {
    qr_x: i32,
    qr_y: i32,
    barcode_x: i32,
    barcode_y: i32,
    qr_box_dots: i32,
}

fn build_native_text_commands(
    content: &ProgressLabelContent,
    options: &LabelOptions,
    layout: &ProgressLayout,
) -> Vec<String> {
    let safe_margin = mm_dots(options.safe_margin_mm, options.dpi);
    let line_step = mm_dots(5.0, options.dpi);
    let left_x = 0.max(safe_margin - mm_dots(2.0, options.dpi));
    let text_right_gap = mm_dots(3.0, options.dpi);
    let text_width = 1.max(layout.qr_x - left_x - text_right_gap);
    let company_y = safe_margin + line_step;
    let item_y = company_y + line_step;
    let qty_y = mm_dots(33.0, options.dpi);

    let mut commands = Vec::new();
    commands.push(native_text(
        left_x,
        company_y,
        &format!("COMPANY: {}", content.company_name),
    ));

    let product = format!("MAHSULOT NOMI: {}", content.product_name);
    let product_lines = wrap_text_for_ezpl(&product, text_width, 1, 8, 8);
    for (idx, line) in product_lines.iter().take(4).enumerate() {
        commands.push(native_text(left_x, item_y + idx as i32 * line_step, line));
    }

    commands.push(native_text(
        left_x,
        qty_y,
        &format!("KG: {}", content.kg_text),
    ));
    commands.push(native_text(
        left_x,
        qty_y + line_step,
        &format!("METRAJ: {}", content.qty_text),
    ));
    commands
}

fn native_text(x: i32, y: i32, value: &str) -> String {
    format!("AB,{x},{y},1,1,0,0,{}", sanitize_label_text(value))
}

fn compute_progress_layout(options: &LabelOptions) -> ProgressLayout {
    let label_width_dots = mm_dots(f64::from(options.label_width_mm), options.dpi);
    let label_length_dots = mm_dots(f64::from(options.label_length_mm), options.dpi);
    let safe_margin_dots = mm_dots(options.safe_margin_mm, options.dpi);
    let left_x = 0.max(safe_margin_dots - mm_dots(2.0, options.dpi));
    let line_step = mm_dots(5.0, options.dpi);

    let qr_box_dots = mm_dots(options.qr_box_mm, options.dpi);
    let qr_right_gap_dots = mm_dots(4.0, options.dpi);
    let base_qr_x = label_width_dots - qr_box_dots - qr_right_gap_dots;
    let qr_x = (label_width_dots - qr_box_dots).min(left_x.max(base_qr_x));

    let qty_y = mm_dots(33.0, options.dpi);
    let mut qr_y = (safe_margin_dots + line_step * 2).max(qty_y + line_step);
    qr_y = (label_length_dots - safe_margin_dots - mm_dots(18.0, options.dpi))
        .min(qr_y + mm_dots(8.0, options.dpi));
    let epc_y = 0.max(safe_margin_dots - line_step * 5);
    let barcode_y = 0.max(epc_y + mm_dots(3.0, options.dpi));
    let barcode_x = 0.max(left_x - mm_dots(2.0, options.dpi));

    ProgressLayout {
        qr_x,
        qr_y,
        barcode_x,
        barcode_y,
        qr_box_dots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> ProgressLabelContent {
        ProgressLabelContent {
            company_name: "ACCORD".to_string(),
            product_name: "VESTA YARIM TAYYOR, PECHAT HOLATDA, PAUZA".to_string(),
            qty_text: "120 M".to_string(),
            kg_text: "17".to_string(),
            executor_name: "ALI".to_string(),
            epc: "400100000000000000000001".to_string(),
            qr_payload: "400100000000000000000001".to_string(),
        }
    }

    #[test]
    fn builds_progress_commands_like_gscale_pack_layout_with_meter_and_kg_labels() {
        let render = build_progress_pack_render(&content(), LabelOptions::default_pack()).unwrap();

        assert!(
            render
                .commands
                .iter()
                .any(|command| command == "Y0,0,TEXTLBL")
        );
        assert!(
            !render
                .commands
                .iter()
                .any(|command| command.starts_with("AB,") && command.contains("EPC:")),
            "EPC text must stay on the top bitmap graphic like standard pack"
        );
        assert!(
            render
                .commands
                .iter()
                .any(|command| command.contains("KG: 17"))
        );
        assert!(
            render
                .commands
                .iter()
                .any(|command| command.contains("METRAJ: 120 M"))
        );
        assert!(
            !render
                .commands
                .iter()
                .any(|command| command.contains("NETTO:") || command.contains("BRUTTO:"))
        );
        assert_eq!(
            render
                .commands
                .iter()
                .find(|command| command.starts_with("BA,"))
                .unwrap(),
            "BA,0,24,1,2,42,0,0,400100000000000000000001"
        );
        assert!(
            render
                .commands
                .iter()
                .any(|command| command == "Y224,224,QRLBL")
        );
        assert_eq!(render.qr_payload, "400100000000000000000001");
        assert_eq!(&render.text_graphic_bmp[0..2], b"BM");
        assert_eq!(&render.qr_graphic_bmp[0..2], b"BM");
    }
}
