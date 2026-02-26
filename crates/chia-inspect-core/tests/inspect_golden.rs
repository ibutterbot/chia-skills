use chia_inspect_core::{ExplainLevel, inspect_bundle, load_mempool_blob_input};
use chia_protocol::{Coin, CoinSpend, Program, SpendBundle};
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
fn golden_simple_mempool_blob() {
    let bundle = sample_spend_bundle();
    let blob = json!({ "spend_bundle": bundle });
    let blob_str = serde_json::to_string(&blob).expect("blob json");
    let (source, parsed, notes) = load_mempool_blob_input(&blob_str).expect("parse blob");
    let output = inspect_bundle(source, parsed, notes, ExplainLevel::Deep).expect("inspect");
    let actual = serde_json::to_string_pretty(&output).expect("serialize output");
    let expected = include_str!("fixtures/simple_inspection.json");
    assert_eq!(actual.trim(), expected.trim());
}
