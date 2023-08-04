use super::frame::{append_raw, pop_serial_frame};
use super::parser::parse_weight;
use super::reading::Reading;
use super::{ScaleCapabilities, ScaleChunkDecoder, ScaleDriver};

#[derive(Debug, Clone)]
pub struct SerialStreamDecoder {
    pending: String,
    last_unit: String,
    seen_parsed_value: bool,
    port: String,
    baud: u32,
    default_unit: String,
}

impl SerialStreamDecoder {
    pub fn new(port: &str, baud: u32, default_unit: &str) -> Self {
        let mut last_unit = default_unit.trim().to_ascii_lowercase();
        if last_unit.is_empty() {
            last_unit = "kg".to_string();
        }

        Self {
            pending: String::new(),
            last_unit,
            seen_parsed_value: false,
            port: port.trim().to_string(),
            baud,
            default_unit: default_unit.to_string(),
        }
    }

    pub fn push_chunk(&mut self, chunk: &str) -> Vec<Reading> {
        self.pending = append_raw(&self.pending, chunk, 1024);
        let mut out = Vec::new();

        while let Some((frame, rest)) = pop_serial_frame(&self.pending) {
            self.pending = rest;
            if let Some(reading) = self.frame_to_reading(&frame) {
                out.push(reading);
            }
        }

        out
    }

    fn frame_to_reading(&mut self, frame: &str) -> Option<Reading> {
        let trimmed = frame.trim();
        if trimmed.is_empty() {
            if !self.seen_parsed_value {
                return None;
            }
            return Some(self.base_reading().with_weight(0.0, None, "<empty-frame>"));
        }

        let Some(parsed) = parse_weight(trimmed, &self.default_unit) else {
            return Some(self.base_reading().with_raw(trimmed));
        };

        if !parsed.unit.trim().is_empty() {
            self.last_unit = parsed.unit.clone();
        }
        self.seen_parsed_value = true;
        Some(
            self.base_reading()
                .with_weight(parsed.weight, parsed.stable, trimmed),
        )
    }

    fn base_reading(&self) -> Reading {
        Reading::serial(&self.port, self.baud, &self.last_unit)
    }
}

impl ScaleDriver for SerialStreamDecoder {
    fn capabilities(&self) -> ScaleCapabilities {
        ScaleCapabilities::serial(&self.port, self.baud, &self.default_unit)
    }
}

impl ScaleChunkDecoder for SerialStreamDecoder {
    fn push_chunk(&mut self, chunk: &str) -> Vec<Reading> {
        SerialStreamDecoder::push_chunk(self, chunk)
    }
}

#[cfg(test)]
mod tests {
    use crate::scale::{ScaleDriver, ScaleTransport};

    use super::SerialStreamDecoder;

    #[test]
    fn decodes_multiple_frames_from_stream() {
        let mut decoder = SerialStreamDecoder::new("/dev/ttyUSB0", 9600, "kg");
        let readings = decoder.push_chunk("ST, 2.50kg\rUS, -1.00kg\n");

        assert_eq!(readings.len(), 2);
        assert_eq!(readings[0].weight, Some(2.5));
        assert_eq!(readings[0].stable, Some(true));
        assert_eq!(readings[1].weight, Some(-1.0));
        assert_eq!(readings[1].stable, Some(false));
    }

    #[test]
    fn skips_empty_frames_until_first_parsed_value_like_go() {
        let mut decoder = SerialStreamDecoder::new("/dev/ttyUSB0", 9600, "kg");
        assert!(decoder.push_chunk("\r\n").is_empty());

        let readings = decoder.push_chunk("1kg\r \r");
        assert_eq!(readings.len(), 2);
        assert_eq!(readings[0].weight, Some(1.0));
        assert_eq!(readings[1].weight, Some(0.0));
        assert_eq!(readings[1].raw, "<empty-frame>");
    }

    #[test]
    fn parse_miss_keeps_stream_alive_like_go() {
        let mut decoder = SerialStreamDecoder::new("/dev/ttyUSB0", 9600, "kg");
        let readings = decoder.push_chunk("bad frame\r");

        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].weight, None);
        assert_eq!(readings[0].unit, "kg");
        assert_eq!(readings[0].raw, "bad frame");
    }

    #[test]
    fn serial_decoder_exposes_driver_capabilities() {
        let decoder = SerialStreamDecoder::new("/dev/ttyUSB0", 9600, "KG");
        let caps = decoder.capabilities();

        assert_eq!(caps.transport, ScaleTransport::Serial);
        assert_eq!(caps.connection, "/dev/ttyUSB0@9600");
        assert_eq!(caps.default_unit, "kg");
        assert!(caps.realtime_weight);
    }

    #[test]
    fn decodes_polygon_simulator_raw_frames() {
        let mut decoder = SerialStreamDecoder::new("polygon://scale", 0, "kg");
        let readings = decoder.push_chunk("1.250 kg ST\n2.750 kg US\n0.000 kg ST\n");

        assert_eq!(readings.len(), 3);
        assert_eq!(readings[0].source, "serial");
        assert_eq!(readings[0].port, "polygon://scale");
        assert_eq!(readings[0].weight, Some(1.25));
        assert_eq!(readings[0].stable, Some(true));
        assert_eq!(readings[0].raw, "1.250 kg ST");
        assert_eq!(readings[1].weight, Some(2.75));
        assert_eq!(readings[1].stable, Some(false));
        assert_eq!(readings[1].raw, "2.750 kg US");
        assert_eq!(readings[2].weight, Some(0.0));
        assert_eq!(readings[2].stable, Some(true));
        assert_eq!(readings[2].raw, "0.000 kg ST");
    }
}
