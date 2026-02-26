use anyhow::{Context, Result, bail};
use chia_protocol::{CoinSpend, SpendBundle};
use chia_traits::Streamable;
use serde_json::{Map, Value, json};

use crate::util::{decode_hex, normalize_hex_no_prefix};

#[derive(Debug, Clone)]
pub enum InputSource {
    Mempool,
    Block,
    Coin,
}

impl InputSource {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Mempool => "mempool_item",
            Self::Block => "block",
            Self::Coin => "coin",
        }
    }
}

pub fn load_mempool_blob_input(blob_json: &str) -> Result<(InputSource, SpendBundle, Vec<String>)> {
    let value: Value = serde_json::from_str(blob_json)?;
    let mut notes = Vec::new();
    let scope = if let Some(wrapper) = value.get("mempool_item") {
        notes.push("input contained mempool_item wrapper; using nested payload".to_string());
        wrapper
    } else {
        &value
    };

    let bundle = if let Some(sb) = scope.get("spend_bundle") {
        parse_spend_bundle_object(sb)?
    } else if let Some(sb_bytes) = scope.get("spend_bundle_bytes").and_then(Value::as_str) {
        notes.push("input contained spend_bundle_bytes; decoded with streamable parser".to_string());
        parse_spend_bundle_bytes(sb_bytes)?
    } else if scope.get("coin_spends").is_some() {
        parse_spend_bundle_object(scope)?
    } else {
        bail!(
            "mempool blob must contain spend_bundle, spend_bundle_bytes, or coin_spends (top-level or inside mempool_item)"
        );
    };

    Ok((InputSource::Mempool, bundle, notes))
}

pub fn load_block_spends_input(spends_json: &str) -> Result<(InputSource, SpendBundle, Vec<String>)> {
    let value: Value = serde_json::from_str(spends_json)?;
    let mut notes = Vec::new();

    let spends = if let Some(items) = value.get("coin_spends") {
        parse_coin_spend_list(items)?
    } else if let Some(items) = value.get("block_spends") {
        parse_coin_spend_list(items)?
    } else if value.is_array() {
        parse_coin_spend_list(&value)?
    } else {
        bail!("block input must be an array or object containing coin_spends/block_spends");
    };

    notes.push("block input normalized to SpendBundle with default aggregate signature".to_string());
    Ok((
        InputSource::Block,
        SpendBundle::new(spends, Default::default()),
        notes,
    ))
}

pub fn load_coin_spend_input(coin_spend_json: &str) -> Result<(InputSource, SpendBundle, Vec<String>)> {
    let value: Value = serde_json::from_str(coin_spend_json)?;
    let mut notes = Vec::new();
    let spend_value = value.get("coin_spend").unwrap_or(&value);
    let spend = parse_coin_spend(spend_value)?;
    notes.push("coin input normalized to single-spend SpendBundle".to_string());
    Ok((
        InputSource::Coin,
        SpendBundle::new(vec![spend], Default::default()),
        notes,
    ))
}

fn parse_spend_bundle_object(value: &Value) -> Result<SpendBundle> {
    let normalized = normalize_spend_bundle_value(value)?;
    serde_json::from_value(normalized).context("failed to parse spend bundle JSON")
}

fn parse_spend_bundle_bytes(hex_value: &str) -> Result<SpendBundle> {
    let bytes = decode_hex(hex_value)?;
    SpendBundle::from_bytes(&bytes).context("failed to parse spend bundle bytes")
}

fn parse_coin_spend_list(value: &Value) -> Result<Vec<CoinSpend>> {
    let arr = value.as_array().context("coin spend list must be an array")?;
    let mut ret = Vec::with_capacity(arr.len());
    for item in arr {
        ret.push(parse_coin_spend(item)?);
    }
    Ok(ret)
}

fn parse_coin_spend(value: &Value) -> Result<CoinSpend> {
    let normalized = normalize_coin_spend_value(value)?;
    serde_json::from_value(normalized).context("failed to parse coin spend")
}

fn normalize_spend_bundle_value(value: &Value) -> Result<Value> {
    let mut obj = value
        .as_object()
        .context("spend bundle JSON must be an object")?
        .clone();
    let coin_spends = obj
        .get("coin_spends")
        .cloned()
        .context("spend bundle is missing coin_spends")?;
    let normalized_spends = normalize_coin_spend_array(&coin_spends)?;
    obj.insert("coin_spends".to_string(), normalized_spends);
    Ok(Value::Object(obj))
}

fn normalize_coin_spend_array(value: &Value) -> Result<Value> {
    let arr = value
        .as_array()
        .context("coin_spends must be an array of coin spend objects")?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(normalize_coin_spend_value(item)?);
    }
    Ok(Value::Array(out))
}

fn normalize_coin_spend_value(value: &Value) -> Result<Value> {
    let obj = value
        .as_object()
        .context("coin spend entry must be an object")?;
    let mut out = Map::new();
    for (k, v) in obj {
        if k == "puzzle_reveal" || k == "solution" {
            let s = v
                .as_str()
                .with_context(|| format!("{k} must be a hex string"))?;
            out.insert(k.clone(), json!(normalize_hex_no_prefix(s)?));
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chia_protocol::{Coin, CoinSpend, Program};
    use clvm_utils::tree_hash_from_bytes;
    use serde_json::json;

    fn sample_spend_bundle() -> SpendBundle {
        let parent = [0x11_u8; 32];
        let puzzle = Program::from(vec![0x01_u8]);
        let puzzle_hash = tree_hash_from_bytes(puzzle.as_ref()).expect("tree hash");
        let coin = Coin::new(parent.into(), puzzle_hash.into(), 1);
        let solution = Program::from(
            hex::decode(format!("ffff33ffa0{}ff018080", "22".repeat(32))).expect("solution hex"),
        );
        let spend = CoinSpend::new(coin, puzzle, solution);
        SpendBundle::new(vec![spend], Default::default())
    }

    #[test]
    fn mempool_wrapper_spend_bundle_parses() {
        let bundle = sample_spend_bundle();
        let blob = json!({ "spend_bundle": bundle });
        let (source, parsed, _notes) =
            load_mempool_blob_input(&serde_json::to_string(&blob).expect("json")).expect("parse");
        assert_eq!(source.kind(), "mempool_item");
        assert_eq!(parsed.coin_spends.len(), 1);
    }

    #[test]
    fn mempool_item_wrapper_spend_bundle_parses() {
        let bundle = sample_spend_bundle();
        let blob = json!({ "mempool_item": { "spend_bundle": bundle } });
        let (_source, parsed, notes) =
            load_mempool_blob_input(&serde_json::to_string(&blob).expect("json")).expect("parse");
        assert_eq!(parsed.coin_spends.len(), 1);
        assert!(notes.iter().any(|n| n.contains("mempool_item wrapper")));
    }

    #[test]
    fn mempool_bytes_shape_parses() {
        let bundle = sample_spend_bundle();
        let bytes = bundle.to_bytes().expect("to bytes");
        let blob = json!({ "spend_bundle_bytes": format!("0x{}", hex::encode(bytes)) });
        let (_source, parsed, _notes) =
            load_mempool_blob_input(&serde_json::to_string(&blob).expect("json")).expect("parse");
        assert_eq!(parsed.coin_spends.len(), 1);
    }

    #[test]
    fn mempool_item_wrapper_bytes_parses() {
        let bundle = sample_spend_bundle();
        let bytes = bundle.to_bytes().expect("to bytes");
        let blob = json!({
            "mempool_item": { "spend_bundle_bytes": format!("0x{}", hex::encode(bytes)) }
        });
        let (_source, parsed, notes) =
            load_mempool_blob_input(&serde_json::to_string(&blob).expect("json")).expect("parse");
        assert_eq!(parsed.coin_spends.len(), 1);
        assert!(notes.iter().any(|n| n.contains("mempool_item wrapper")));
    }

    #[test]
    fn block_coin_spend_array_parses() {
        let bundle = sample_spend_bundle();
        let blob = json!({ "coin_spends": bundle.coin_spends });
        let (_source, parsed, _notes) =
            load_block_spends_input(&serde_json::to_string(&blob).expect("json")).expect("parse");
        assert_eq!(parsed.coin_spends.len(), 1);
    }
}
