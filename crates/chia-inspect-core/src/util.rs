use anyhow::{Result, anyhow, bail};

pub fn strip_0x(s: &str) -> &str {
    s.strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s)
}

pub fn decode_hex(s: &str) -> Result<Vec<u8>> {
    let raw = strip_0x(s).trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    if raw.len() % 2 != 0 {
        bail!("hex string has odd length: {raw}");
    }
    Ok(hex::decode(raw)?)
}

pub fn encode_hex_prefixed(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

pub fn normalize_hex_no_prefix(s: &str) -> Result<String> {
    let bytes = decode_hex(s)?;
    Ok(hex::encode(bytes))
}

pub fn read_text_input(path_or_stdin: &str, stdin_fallback: Option<String>) -> Result<String> {
    if path_or_stdin == "-" {
        return stdin_fallback.ok_or_else(|| anyhow!("stdin was requested but no stdin was provided"));
    }
    Ok(std::fs::read_to_string(path_or_stdin)?)
}
