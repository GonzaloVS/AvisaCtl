use chrono::{DateTime, Utc};
use rasn::types::GeneralizedTime;
use rasn::Decoder;

#[derive(Debug, rasn::AsnType, rasn::Decode)]
struct TstInfo {
    #[rasn(tag(0))]
    version: i32,
    #[rasn(tag(2))]
    gen_time: GeneralizedTime,
    // ... (puedes ampliar si necesitas más campos)
}

/// Extrae la fecha de emisión desde la firma TSA (.tsr) si es posible
pub fn extract_tsa_date(tsr_data: &[u8]) -> Result<String, String> {
    use rasn::der;

    let tst_info = der::decode::<TstInfo>(tsr_data)
        .map_err(|e| format!("Falló la decodificación DER: {}", e))?;

    let dt = DateTime::parse_from_rfc3339(&tst_info.gen_time.to_string())
        .map_err(|e| format!("Fecha inválida: {}", e))?;

    Ok(dt.with_timezone(&Utc).format("%Y-%m-%dT%H-%M-%SZ").to_string())
}