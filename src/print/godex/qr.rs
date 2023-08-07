use qrcodegen::{QrCode, QrCodeEcc};

use super::bmp::{MonoBitmap, encode_mono_bmp};

pub fn render_qr_graphic(payload: &str, box_dots: i32) -> Result<Vec<u8>, String> {
    if payload.is_empty() {
        return Err("qr payload is empty".to_string());
    }
    if box_dots <= 0 {
        return Err("qr box dots must be positive".to_string());
    }

    let qr = QrCode::encode_text(payload, QrCodeEcc::Low)
        .map_err(|error| format!("build qr: {error:?}"))?;
    let box_dots = box_dots as usize;
    let matrix_size = qr.size() as usize;
    let quiet_zone = 4_usize;
    let module_count = matrix_size + quiet_zone * 2;
    let module_dots = (box_dots / module_count).max(1);
    let drawn = module_count * module_dots;
    let offset = box_dots.saturating_sub(drawn) / 2;

    let mut bitmap = MonoBitmap::filled(box_dots, box_dots, true);
    for y in 0..matrix_size {
        for x in 0..matrix_size {
            if !qr.get_module(x as i32, y as i32) {
                continue;
            }
            let start_x = offset + (x + quiet_zone) * module_dots;
            let start_y = offset + (y + quiet_zone) * module_dots;
            for dy in 0..module_dots {
                for dx in 0..module_dots {
                    bitmap.set_light(start_x + dx, start_y + dy, false);
                }
            }
        }
    }

    Ok(encode_mono_bmp(&bitmap))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u32(data: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    #[test]
    fn rejects_invalid_qr_inputs_like_gscale() {
        assert_eq!(
            render_qr_graphic("", 144).unwrap_err(),
            "qr payload is empty"
        );
        assert_eq!(
            render_qr_graphic("payload", 0).unwrap_err(),
            "qr box dots must be positive"
        );
    }

    #[test]
    fn renders_deterministic_qr_bmp() {
        let first = render_qr_graphic("https://scan.wspace.sbs/L/A/B/1/5/EPC", 144).unwrap();
        let second = render_qr_graphic("https://scan.wspace.sbs/L/A/B/1/5/EPC", 144).unwrap();

        assert_eq!(first, second);
        assert_eq!(&first[0..2], b"BM");
        assert_eq!(read_u32(&first, 18), 144);
        assert_eq!(read_u32(&first, 22), 144);
        assert!(first[62..].iter().any(|byte| *byte != 0xff));
    }
}
