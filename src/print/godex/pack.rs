use crate::core::PackLabelContent;

use super::options::{LabelOptions, mm_dots};
use super::qr::render_qr_graphic;
use super::text_graphic::render_pack_text_graphic;

const TEXT_GRAPHIC_NAME: &str = "TEXTLBL";
const QR_GRAPHIC_NAME: &str = "QRLBL";

#[derive(Clone, Debug, PartialEq)]
pub struct GodexPackRender {
    pub commands: Vec<String>,
    pub qr_payload: String,
    pub text_graphic_bmp: Vec<u8>,
    pub qr_graphic_bmp: Vec<u8>,
    pub text_graphic_name: String,
    pub qr_graphic_name: String,
    pub qr_box_dots: i32,
}

pub fn build_pack_render(
    content: &PackLabelContent,
    options: LabelOptions,
) -> Result<GodexPackRender, String> {
    let options = options.normalized_pack();
    let layout = compute_pack_layout(&options);
    let text_graphic_bmp = render_pack_text_graphic(content, &options);
    let qr_graphic_bmp = render_qr_graphic(&content.qr_payload, layout.qr_box_dots)?;

    let commands = vec![
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
        format!("Y0,0,{TEXT_GRAPHIC_NAME}"),
        format!(
            "BA,{},{},1,2,42,0,0,{}",
            layout.barcode_x, layout.barcode_y, content.epc
        ),
        format!("Y{},{},{}", layout.qr_x, layout.qr_y, QR_GRAPHIC_NAME),
        "E".to_string(),
    ];

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

#[derive(Clone, Copy, Debug, PartialEq)]
struct PackLayout {
    qr_x: i32,
    qr_y: i32,
    barcode_x: i32,
    barcode_y: i32,
    qr_box_dots: i32,
}

fn compute_pack_layout(options: &LabelOptions) -> PackLayout {
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

    PackLayout {
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
    use crate::core::{PrintSelection, QuantitySource, build_pack_label_content};
    use crate::print::mode::PrintMode;

    fn content() -> PackLabelContent {
        let job = crate::core::CorePrintJob::from_selection(
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
                tare_enabled: false,
                tare_kg: 0.0,
            },
        );
        build_pack_label_content(&job, "Accord LLC", "5kg").unwrap()
    }

    #[test]
    fn builds_pack_commands_like_gscale_godex_layout() {
        let render = build_pack_render(&content(), LabelOptions::default_pack()).unwrap();

        assert_eq!(
            render.commands,
            vec![
                "~S,ESG",
                "^AD",
                "^XSET,UNICODE,1",
                "^XSET,IMMEDIATE,1",
                "^XSET,ACTIVERESPONSE,1",
                "^XSET,CODEPAGE,16",
                "^Q50,3",
                "^W50",
                "^H10",
                "^P1",
                "^L",
                "Y0,0,TEXTLBL",
                "BA,0,24,1,2,42,0,0,3034257BF7194E406994036B",
                "Y224,224,QRLBL",
                "E",
            ]
        );
        assert_eq!(
            render.qr_payload,
            "https://scan.wspace.sbs/L/ACCORD+LLC/GREEN+TEA/1.3/5/3034257BF7194E406994036B"
        );
        assert_eq!(render.qr_box_dots, 144);
        assert_eq!(&render.text_graphic_bmp[0..2], b"BM");
        assert_eq!(&render.qr_graphic_bmp[0..2], b"BM");
    }

    #[test]
    fn custom_options_follow_gscale_coordinate_math() {
        let options = LabelOptions {
            label_length_mm: 40,
            label_gap_mm: 2,
            label_width_mm: 60,
            dpi: 203,
            safe_margin_mm: 5.0,
            qr_box_mm: 16.0,
        };
        let render = build_pack_render(&content(), options).unwrap();

        assert_eq!(render.commands[6], "^Q40,2");
        assert_eq!(render.commands[7], "^W60");
        assert_eq!(
            render.commands[12],
            "BA,8,24,1,2,42,0,0,3034257BF7194E406994036B"
        );
        assert_eq!(render.commands[13], "Y320,136,QRLBL");
        assert_eq!(render.qr_box_dots, 128);
    }
}
