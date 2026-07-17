use super::epc::normalize_epc;
use super::text::sanitize_zpl_text;
use super::weight_block::build_zebra_weight_block;

pub fn build_rfid_encode_command(
    epc: &str,
    qty_text: &str,
    item_name: &str,
) -> Result<String, String> {
    build_rfid_encode_command_with_weights(epc, qty_text, "", item_name)
}

pub fn build_rfid_encode_command_with_weights(
    epc: &str,
    qty_text: &str,
    brutto_text: &str,
    item_name: &str,
) -> Result<String, String> {
    let data = normalize_job_data(epc, qty_text, brutto_text, item_name)?;
    let block = build_zebra_weight_block(&data.qty, &data.brutto);

    Ok("~PS\n".to_string()
        + "^XA\n"
        + "^LH0,0\n"
        + "^RS8,,,1,N\n"
        + &format!("^RFW,H,,,A^FD{}^FS\n", data.epc)
        + "^FO8,52^A0N,38,32^FB760,1,0,L,0\n"
        + &format!("^FDMAHSULOT: {}^FS\n", data.item)
        + &block.zpl
        + &format!("^FO8,{}^A0N,24,20^FB760,1,0,L,0\n", block.epc_y)
        + &format!("^FDEPC: {}^FS\n", sanitize_zpl_text(&data.epc))
        + &format!("^FO8,{}^BY3,2,44^BCN,44,N,N,N\n", block.barcode_y)
        + &format!("^FD{}^FS\n", sanitize_zpl_text(&data.epc))
        + "^PQ1\n"
        + "^XZ\n")
}

pub fn build_label_only_print_command(
    epc: &str,
    qty_text: &str,
    item_name: &str,
) -> Result<String, String> {
    build_label_only_print_command_with_weights(epc, qty_text, "", item_name)
}

pub fn build_qolip_cell_qr_command(epc: &str, cell_name: &str) -> Result<String, String> {
    let epc = sanitize_zpl_text(epc.trim());
    if epc.is_empty() {
        return Err("qr payload is empty".to_string());
    }
    let cell_name = {
        let value = sanitize_zpl_text(cell_name.trim());
        if value.is_empty() {
            "-".to_string()
        } else {
            value
        }
    };

    Ok("~PS\n".to_string()
        + "^XA\n"
        + "^LH0,0\n"
        + "^FO8,16^A0N,88,76^FB784,1,0,C,0\n"
        + &format!("^FD{cell_name}^FS\n")
        + "^FO120,124^BQN,2,11^FDLA,"
        + &epc
        + "^FS\n"
        + "^PQ1\n"
        + "^XZ\n")
}

pub fn build_label_only_print_command_with_weights(
    epc: &str,
    qty_text: &str,
    brutto_text: &str,
    item_name: &str,
) -> Result<String, String> {
    let data = normalize_job_data(epc, qty_text, brutto_text, item_name)?;
    let block = build_zebra_weight_block(&data.qty, &data.brutto);

    Ok("~PS\n".to_string()
        + "^XA\n"
        + "^LH0,0\n"
        + "^MMT\n"
        + "^FO8,52^A0N,38,32^FB760,1,0,L,0\n"
        + &format!("^FDMAHSULOT: {}^FS\n", data.item)
        + &block.zpl
        + &format!("^FO8,{}^A0N,24,20^FB760,1,0,L,0\n", block.epc_y)
        + &format!("^FDEPC: {}^FS\n", sanitize_zpl_text(&data.epc))
        + &format!("^FO8,{}^BY3,2,44^BCN,44,N,N,N\n", block.barcode_y)
        + &format!("^FD{}^FS\n", sanitize_zpl_text(&data.epc))
        + "^PQ1\n"
        + "^XZ\n")
}

struct NormalizedJobData {
    epc: String,
    qty: String,
    brutto: String,
    item: String,
}

fn normalize_job_data(
    epc: &str,
    qty_text: &str,
    brutto_text: &str,
    item_name: &str,
) -> Result<NormalizedJobData, String> {
    let epc = normalize_epc(epc)?;
    let mut qty = sanitize_zpl_text(qty_text.trim());
    if qty.is_empty() {
        qty = "- kg".to_string();
    }
    let brutto = sanitize_zpl_text(brutto_text.trim());
    let mut item = sanitize_zpl_text(item_name.trim());
    if item.is_empty() {
        item = "-".to_string();
    }
    Ok(NormalizedJobData {
        epc,
        qty,
        brutto,
        item,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPC: &str = "3034257BF7194E406994036B";

    #[test]
    fn builds_rfid_encode_command_with_barcode_and_text() {
        let zpl = build_rfid_encode_command(EPC, "1.2 kg", "Green Tea").unwrap();

        assert!(zpl.starts_with("~PS\n^XA\n^LH0,0\n^RS8,,,1,N\n"));
        assert!(zpl.contains(&format!("^RFW,H,,,A^FD{EPC}^FS")));
        assert!(zpl.contains("^FDMAHSULOT: Green Tea^FS"));
        assert!(zpl.contains("^FDVAZNI: 1.2 kg^FS"));
        assert!(zpl.contains(&format!("^FDEPC: {EPC}^FS")));
        assert!(zpl.contains("^FO8,236^BY3,2,44^BCN,44,N,N,N"));
        assert!(zpl.ends_with("^PQ1\n^XZ\n"));
        assert!(zpl.matches(EPC).count() >= 3);
    }

    #[test]
    fn builds_label_only_command_without_rfid_write() {
        let zpl = build_label_only_print_command(EPC, "", "").unwrap();

        assert!(zpl.contains("^MMT"));
        assert!(!zpl.contains("^RFW"));
        assert!(!zpl.contains("^RS8"));
        assert!(zpl.contains("^FDMAHSULOT: -^FS"));
        assert!(zpl.contains("^FDVAZNI: - kg^FS"));
        assert!(zpl.contains(&format!("^FD{EPC}^FS")));
    }

    #[test]
    fn builds_tare_weight_block_positions() {
        let zpl = build_rfid_encode_command_with_weights(EPC, "1.7 kg", "2.5 kg", "Tea").unwrap();

        assert!(zpl.contains("^FDNETTO: 1.7 kg^FS"));
        assert!(zpl.contains("^FDBRUTTO: 2.5 kg^FS"));
        assert!(zpl.contains("^FO8,220^A0N,24,20^FB760,1,0,L,0"));
        assert!(zpl.contains("^FO8,272^BY3,2,44^BCN,44,N,N,N"));
    }

    #[test]
    fn rejects_invalid_epc_before_building_command() {
        let err = build_rfid_encode_command("ZZZZ", "1 kg", "Tea").unwrap_err();

        assert_eq!(err, "epc faqat hex bo'lishi kerak");
    }

    #[test]
    fn builds_qolip_cell_qr_without_rfid_write() {
        let zpl = build_qolip_cell_qr_command("CELL-QR-A1", "A1").unwrap();

        assert!(zpl.contains("^FO8,16^A0N,88,76^FB784,1,0,C,0"));
        assert!(zpl.contains("^FDLA,CELL-QR-A1^FS"));
        assert!(zpl.contains("^FO120,124^BQN,2,11"));
        assert!(!zpl.contains("^RFW"));
    }
}
