#[derive(Clone, Debug, PartialEq)]
pub struct MonoBitmap {
    width: usize,
    height: usize,
    light_pixels: Vec<bool>,
}

impl MonoBitmap {
    pub fn new(width: usize, height: usize) -> Self {
        Self::filled(width, height, false)
    }

    pub fn filled(width: usize, height: usize, light: bool) -> Self {
        Self {
            width,
            height,
            light_pixels: vec![light; width.saturating_mul(height)],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn set_light(&mut self, x: usize, y: usize, light: bool) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.light_pixels[y * self.width + x] = light;
    }

    pub fn crop_ink(&self) -> Self {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut found = false;
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.is_light(x, y) {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x + 1);
                    max_y = max_y.max(y + 1);
                    found = true;
                }
            }
        }
        if !found {
            return self.clone();
        }
        min_x = min_x.saturating_sub(1);
        max_x = (max_x + 1).min(self.width);
        let mut out = Self::filled(max_x - min_x, max_y - min_y, true);
        for y in min_y..max_y {
            for x in min_x..max_x {
                out.set_light(x - min_x, y - min_y, self.is_light(x, y));
            }
        }
        out
    }

    fn is_light(&self, x: usize, y: usize) -> bool {
        self.light_pixels[y * self.width + x]
    }
}

pub fn encode_mono_bmp(src: &MonoBitmap) -> Vec<u8> {
    let row_bytes = src.width.div_ceil(32) * 4;
    let pixel_bytes = row_bytes * src.height;
    const HEADER_BYTES: usize = 14 + 40 + 8;
    let file_bytes = HEADER_BYTES + pixel_bytes;

    let mut out = Vec::with_capacity(file_bytes);
    out.extend_from_slice(b"BM");
    write_u32(&mut out, file_bytes as u32);
    write_u16(&mut out, 0);
    write_u16(&mut out, 0);
    write_u32(&mut out, HEADER_BYTES as u32);

    write_u32(&mut out, 40);
    write_i32(&mut out, src.width as i32);
    write_i32(&mut out, src.height as i32);
    write_u16(&mut out, 1);
    write_u16(&mut out, 1);
    write_u32(&mut out, 0);
    write_u32(&mut out, pixel_bytes as u32);
    write_i32(&mut out, 0);
    write_i32(&mut out, 0);
    write_u32(&mut out, 2);
    write_u32(&mut out, 2);

    out.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    out.extend_from_slice(&[0xff, 0xff, 0xff, 0x00]);

    for y in (0..src.height).rev() {
        let mut row = vec![0_u8; row_bytes];
        for x in 0..src.width {
            if src.is_light(x, y) {
                row[x / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend_from_slice(&row);
    }
    out
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u16(data: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([data[offset], data[offset + 1]])
    }

    fn read_u32(data: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    #[test]
    fn encodes_one_bit_bmp_header_like_gscale() {
        let img = MonoBitmap::new(9, 2);
        let bmp = encode_mono_bmp(&img);

        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(read_u32(&bmp, 2), 70);
        assert_eq!(read_u32(&bmp, 10), 62);
        assert_eq!(read_u32(&bmp, 14), 40);
        assert_eq!(read_u32(&bmp, 18), 9);
        assert_eq!(read_u32(&bmp, 22), 2);
        assert_eq!(read_u16(&bmp, 26), 1);
        assert_eq!(read_u16(&bmp, 28), 1);
        assert_eq!(read_u32(&bmp, 34), 8);
        assert_eq!(&bmp[54..62], &[0, 0, 0, 0, 255, 255, 255, 0]);
    }

    #[test]
    fn stores_rows_bottom_up_and_packs_light_pixels_like_gscale() {
        let mut img = MonoBitmap::new(9, 2);
        img.set_light(0, 0, true);
        img.set_light(8, 1, true);

        let bmp = encode_mono_bmp(&img);

        assert_eq!(&bmp[62..66], &[0x00, 0x80, 0x00, 0x00]);
        assert_eq!(&bmp[66..70], &[0x80, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn ignores_out_of_bounds_pixels() {
        let mut img = MonoBitmap::new(1, 1);
        img.set_light(1, 0, true);
        img.set_light(0, 1, true);

        let bmp = encode_mono_bmp(&img);

        assert_eq!(&bmp[62..66], &[0, 0, 0, 0]);
    }
}
