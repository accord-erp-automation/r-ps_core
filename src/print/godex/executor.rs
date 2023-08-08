use std::fmt;
use std::time::Duration;

use super::pack::GodexPackRender;

pub trait GodexTransport {
    fn send(
        &mut self,
        command: &str,
        read: bool,
        pause: Duration,
    ) -> Result<String, GodexExecutionError>;

    fn write_raw(&mut self, payload: &[u8]) -> Result<(), GodexExecutionError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct GodexExecutionError {
    message: String,
}

impl GodexExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GodexExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GodexExecutionError {}

pub fn execute_pack_render<T: GodexTransport>(
    transport: &mut T,
    render: &GodexPackRender,
) -> Result<String, GodexExecutionError> {
    transport
        .send("^XSET,BUZZER,0", false, Duration::from_millis(120))
        .map_err(|err| GodexExecutionError::new(format!("disable buzzer: {err}")))?;

    download_graphic(
        transport,
        &render.text_graphic_name,
        &render.text_graphic_bmp,
        "text",
    )?;
    download_graphic(
        transport,
        &render.qr_graphic_name,
        &render.qr_graphic_bmp,
        "qr",
    )?;

    for (idx, command) in render.commands.iter().enumerate() {
        transport
            .send(command, false, Duration::from_millis(120))
            .map_err(|err| {
                GodexExecutionError::new(format!("send print command {}: {err}", idx + 1))
            })?;
    }

    transport
        .send("~S,STATUS", true, Duration::from_millis(120))
        .map_err(|err| GodexExecutionError::new(format!("final status: {err}")))
}

fn download_graphic<T: GodexTransport>(
    transport: &mut T,
    name: &str,
    graphic: &[u8],
    label: &str,
) -> Result<(), GodexExecutionError> {
    let _ = transport.send(&format!("~MDELG,{name}"), false, Duration::from_millis(100));
    transport
        .send(
            &format!("~EB,{name},{}", graphic.len()),
            false,
            Duration::from_millis(50),
        )
        .map_err(|err| GodexExecutionError::new(format!("download {label} graphic: {err}")))?;
    transport
        .write_raw(graphic)
        .map_err(|err| GodexExecutionError::new(format!("download {label} graphic: {err}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CorePrintJob, PrintSelection, QuantitySource, build_pack_label_content};
    use crate::print::godex::{LabelOptions, build_pack_render};
    use crate::print::mode::PrintMode;

    #[derive(Default)]
    struct MockTransport {
        calls: Vec<String>,
        delete_errors: bool,
        fail_on: Option<String>,
    }

    impl GodexTransport for MockTransport {
        fn send(
            &mut self,
            command: &str,
            read: bool,
            _pause: Duration,
        ) -> Result<String, GodexExecutionError> {
            self.calls.push(format!("send:{command}:read={read}"));
            if self.delete_errors && command.starts_with("~MDELG,") {
                return Err(GodexExecutionError::new("delete missing graphic"));
            }
            if self.fail_on.as_deref() == Some(command) {
                return Err(GodexExecutionError::new("forced error"));
            }
            if command == "~S,STATUS" {
                return Ok("00,OK".to_string());
            }
            Ok(String::new())
        }

        fn write_raw(&mut self, payload: &[u8]) -> Result<(), GodexExecutionError> {
            self.calls.push(format!("raw:{}", payload.len()));
            Ok(())
        }
    }

    fn render() -> GodexPackRender {
        let job = CorePrintJob::from_selection(
            "3034257BF7194E406994036B",
            1.72,
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
                tare_enabled: true,
                tare_kg: 0.78,
            },
        );
        let content = build_pack_label_content(&job, "Accord", "5kg").unwrap();
        build_pack_render(&content, LabelOptions::default_pack()).unwrap()
    }

    #[test]
    fn executes_pack_render_like_gscale_print_pack() {
        let render = render();
        let mut transport = MockTransport::default();
        let status = execute_pack_render(&mut transport, &render).unwrap();

        assert_eq!(status, "00,OK");
        assert_eq!(transport.calls[0], "send:^XSET,BUZZER,0:read=false");
        assert_eq!(transport.calls[1], "send:~MDELG,TEXTLBL:read=false");
        assert_eq!(
            transport.calls[2],
            format!(
                "send:~EB,TEXTLBL,{}:read=false",
                render.text_graphic_bmp.len()
            )
        );
        assert_eq!(
            transport.calls[3],
            format!("raw:{}", render.text_graphic_bmp.len())
        );
        assert_eq!(transport.calls[4], "send:~MDELG,QRLBL:read=false");
        assert_eq!(
            transport.calls[5],
            format!("send:~EB,QRLBL,{}:read=false", render.qr_graphic_bmp.len())
        );
        assert_eq!(
            transport.calls[6],
            format!("raw:{}", render.qr_graphic_bmp.len())
        );
        assert_eq!(transport.calls[7], "send:~S,ESG:read=false");
        assert_eq!(transport.calls.last().unwrap(), "send:~S,STATUS:read=true");
    }

    #[test]
    fn ignores_delete_graphic_errors_like_gscale() {
        let mut transport = MockTransport {
            delete_errors: true,
            ..Default::default()
        };

        assert_eq!(
            execute_pack_render(&mut transport, &render()).unwrap(),
            "00,OK"
        );
    }

    #[test]
    fn maps_command_errors_with_gscale_context() {
        let mut transport = MockTransport {
            fail_on: Some("^AD".to_string()),
            ..Default::default()
        };

        let err = execute_pack_render(&mut transport, &render()).unwrap_err();

        assert_eq!(err.to_string(), "send print command 2: forced error");
    }
}
