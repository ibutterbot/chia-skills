use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use chia_consensus::allocator::make_allocator;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::owned_conditions::{OwnedSpendBundleConditions, OwnedSpendConditions};
use chia_consensus::spendbundle_conditions::get_conditions_from_spendbundle;
use chia_protocol::{Bytes, Coin, CoinSpend, SpendBundle};
use chialisp::classic::clvm::{OPERATORS_LATEST_VERSION, keyword_from_atom};
use chialisp::classic::clvm_tools::binutils::disassemble;
use clvm_utils::tree_hash_from_bytes;
use clvmr::allocator::{Allocator as ClvmAllocator, NodePtr, SExp};
use clvmr::serde::node_from_bytes_backrefs;
use clvmr::LIMIT_HEAP;
use serde_json::json;

use crate::input::InputSource;
use crate::recognize::recognize_puzzle_and_solution;
use crate::schema::{
    AggSigInfo, ClvmBehavior, CoinRef, CoinSpendView, ConditionInfo, ConstantBuckets, DynamicBehavior,
    ErrorInfo, EvaluationInfo, Explanation, FailureInfo, InspectionOutput, InputInfo, NetDelta, NetworkInfo,
    PuzzleBehavior, PuzzleId, PuzzleInfo, ResultInfo, SignatureSummary, SourceInfo, SpendAnalysis,
    StaticFeatures, Summary, ToolInfo,
};
use crate::util::encode_hex_prefixed;

const DEFAULT_MAX_COST: u64 = 11_000_000_000;
const DEFAULT_PREV_TX_HEIGHT: u32 = 10_000_000;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ExplainLevel {
    Conditions,
    Deep,
}

impl Default for ExplainLevel {
    fn default() -> Self {
        Self::Deep
    }
}

pub fn inspect_bundle(
    source: InputSource,
    spend_bundle: SpendBundle,
    notes: Vec<String>,
    explain_level: ExplainLevel,
) -> Result<InspectionOutput> {
    let mut allocator = make_allocator(LIMIT_HEAP);
    let eval = get_conditions_from_spendbundle(
        &mut allocator,
        &spend_bundle,
        DEFAULT_MAX_COST,
        DEFAULT_PREV_TX_HEIGHT,
        &TEST_CONSTANTS,
    );

    match eval {
        Ok(conditions) => {
            let owned = OwnedSpendBundleConditions::from(&allocator, conditions);
            Ok(build_success_output(
                source,
                notes,
                spend_bundle,
                owned,
                explain_level,
            ))
        }
        Err(err) => Ok(build_error_output(source, notes, spend_bundle, &format!("{err:?}"))),
    }
}

fn build_success_output(
    source: InputSource,
    notes: Vec<String>,
    spend_bundle: SpendBundle,
    owned: OwnedSpendBundleConditions,
    explain_level: ExplainLevel,
) -> InspectionOutput {
    let mut spends = Vec::<SpendAnalysis>::new();
    let mut removals = Vec::<CoinRef>::new();
    let mut additions = Vec::<CoinRef>::new();
    let mut agg_sig_me = Vec::<AggSigInfo>::new();
    let mut agg_sig_unsafe = Vec::<AggSigInfo>::new();

    let spend_count = spend_bundle.coin_spends.len().min(owned.spends.len());
    for idx in 0..spend_count {
        let spend = &spend_bundle.coin_spends[idx];
        let conds = &owned.spends[idx];
        let spend_analysis = analyze_single_spend(spend, conds, explain_level, &mut agg_sig_me);
        removals.push(coin_ref_from_coin(&spend.coin));
        additions.extend(spend_analysis.evaluation.additions.iter().cloned());
        spends.push(spend_analysis);
    }

    for (pk, msg) in &owned.agg_sig_unsafe {
        agg_sig_unsafe.push(AggSigInfo {
            pubkey: encode_hex_prefixed(&pk.to_bytes()),
            msg: encode_hex_prefixed(msg.as_ref()),
        });
    }

    removals.sort_by(|a, b| a.coin_id.cmp(&b.coin_id));
    additions.sort_by(|a, b| a.coin_id.cmp(&b.coin_id));
    agg_sig_me.sort_by(|a, b| a.pubkey.cmp(&b.pubkey).then(a.msg.cmp(&b.msg)));
    agg_sig_unsafe.sort_by(|a, b| a.pubkey.cmp(&b.pubkey).then(a.msg.cmp(&b.msg)));

    let fee_mojos = owned
        .removal_amount
        .saturating_sub(owned.addition_amount)
        .try_into()
        .unwrap_or(u64::MAX);
    let net_xch_delta_by_puzzle_hash = compute_net_delta(&removals, &additions);

    InspectionOutput {
        schema_version: "chia.inspect.spendbundle.v2".to_string(),
        tool: ToolInfo {
            name: "chia-inspect".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        network: NetworkInfo {
            name: "offline".to_string(),
            genesis_challenge: Some(encode_hex_prefixed(TEST_CONSTANTS.genesis_challenge.as_ref())),
        },
        input: InputInfo {
            source: SourceInfo {
                kind: source.kind().to_string(),
                value: None,
                rpc: None,
            },
            notes,
        },
        result: ResultInfo {
            status: "ok".to_string(),
            error: None,
            summary: Summary {
                removals,
                additions,
                fee_mojos,
                net_xch_delta_by_puzzle_hash,
            },
            spends,
            signatures: SignatureSummary {
                aggregated_signature: encode_hex_prefixed(
                    &spend_bundle.aggregated_signature.to_bytes(),
                ),
                agg_sig_me,
                agg_sig_unsafe,
            },
            offer: None,
        },
    }
}

fn build_error_output(
    source: InputSource,
    notes: Vec<String>,
    spend_bundle: SpendBundle,
    message: &str,
) -> InspectionOutput {
    let mut spends = Vec::new();
    let mut removals = Vec::new();
    for spend in &spend_bundle.coin_spends {
        removals.push(coin_ref_from_coin(&spend.coin));
        let (puzzle_disasm, static_features, uses_backrefs) = analyze_clvm_bytes(spend.puzzle_reveal.as_ref());
        let (solution_disasm, _, _) = analyze_clvm_bytes(spend.solution.as_ref());
        let recognition =
            recognize_puzzle_and_solution(spend.puzzle_reveal.as_ref(), spend.solution.as_ref());
        let puzzle_hash = tree_hash_from_bytes(spend.puzzle_reveal.as_ref())
            .map(|h| encode_hex_prefixed(h.as_ref()))
            .unwrap_or_else(|_| encode_hex_prefixed(spend.coin.puzzle_hash.as_ref()));

        spends.push(SpendAnalysis {
            coin_spend: CoinSpendView {
                coin: coin_ref_from_coin(&spend.coin),
                puzzle_reveal: encode_hex_prefixed(spend.puzzle_reveal.as_ref()),
                solution: encode_hex_prefixed(spend.solution.as_ref()),
            },
            puzzle: PuzzleInfo {
                id: PuzzleId {
                    puzzle_hash: encode_hex_prefixed(spend.coin.puzzle_hash.as_ref()),
                    tree_hash: puzzle_hash.clone(),
                    shatree: puzzle_hash,
                },
                recognition,
                puzzle_reveal_disasm: puzzle_disasm.clone(),
                solution_disasm: solution_disasm.clone(),
            },
            evaluation: EvaluationInfo {
                status: "failed".to_string(),
                cost: 0,
                conditions: Vec::new(),
                additions: Vec::new(),
                announcements: Vec::new(),
                assertions: Vec::new(),
                failure: Some(FailureInfo {
                    kind: "validation_error".to_string(),
                    message: message.to_string(),
                }),
            },
            puzzle_behavior: PuzzleBehavior {
                clvm: ClvmBehavior {
                    puzzle_reveal_bytes: encode_hex_prefixed(spend.puzzle_reveal.as_ref()),
                    solution_bytes: encode_hex_prefixed(spend.solution.as_ref()),
                    puzzle_opd: puzzle_disasm,
                    solution_opd: solution_disasm,
                    uses_backrefs,
                    serialized_len_bytes: spend.puzzle_reveal.len(),
                },
                static_features,
                dynamic: DynamicBehavior {
                    status: "failed".to_string(),
                    cost: 0,
                    conditions: Vec::new(),
                    created_coins: Vec::new(),
                    failure: Some(FailureInfo {
                        kind: "validation_error".to_string(),
                        message: message.to_string(),
                    }),
                },
                explanation: Explanation::default(),
            },
        });
    }

    InspectionOutput {
        schema_version: "chia.inspect.spendbundle.v2".to_string(),
        tool: ToolInfo {
            name: "chia-inspect".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        network: NetworkInfo {
            name: "offline".to_string(),
            genesis_challenge: Some(encode_hex_prefixed(TEST_CONSTANTS.genesis_challenge.as_ref())),
        },
        input: InputInfo {
            source: SourceInfo {
                kind: source.kind().to_string(),
                value: None,
                rpc: None,
            },
            notes,
        },
        result: ResultInfo {
            status: "failed".to_string(),
            error: Some(ErrorInfo {
                kind: "validation_error".to_string(),
                message: message.to_string(),
                details: None,
            }),
            summary: Summary {
                removals,
                additions: Vec::new(),
                fee_mojos: 0,
                net_xch_delta_by_puzzle_hash: Vec::new(),
            },
            spends,
            signatures: SignatureSummary {
                aggregated_signature: encode_hex_prefixed(
                    &spend_bundle.aggregated_signature.to_bytes(),
                ),
                agg_sig_me: Vec::new(),
                agg_sig_unsafe: Vec::new(),
            },
            offer: None,
        },
    }
}

fn analyze_single_spend(
    spend: &CoinSpend,
    conds: &OwnedSpendConditions,
    explain_level: ExplainLevel,
    agg_sig_me_out: &mut Vec<AggSigInfo>,
) -> SpendAnalysis {
    let coin_ref = coin_ref_from_coin(&spend.coin);
    let (puzzle_disasm, static_features, uses_backrefs) = analyze_clvm_bytes(spend.puzzle_reveal.as_ref());
    let (solution_disasm, _, _) = analyze_clvm_bytes(spend.solution.as_ref());
    let recognition = recognize_puzzle_and_solution(spend.puzzle_reveal.as_ref(), spend.solution.as_ref());

    let mut create_coin = conds.create_coin.clone();
    create_coin.sort_by(|a, b| {
        a.0.as_ref()
            .cmp(b.0.as_ref())
            .then(a.1.cmp(&b.1))
            .then(a.2.as_ref().map(Bytes::as_ref).cmp(&b.2.as_ref().map(Bytes::as_ref)))
    });

    let mut conditions = Vec::<ConditionInfo>::new();
    let mut additions = Vec::<CoinRef>::new();
    let mut explanation = Explanation::default();

    add_signature_conditions(
        &conds.agg_sig_me,
        "AGG_SIG_ME",
        &mut conditions,
        &mut explanation,
        agg_sig_me_out,
    );
    add_signature_conditions(
        &conds.agg_sig_parent,
        "AGG_SIG_PARENT",
        &mut conditions,
        &mut explanation,
        &mut Vec::new(),
    );
    add_signature_conditions(
        &conds.agg_sig_puzzle,
        "AGG_SIG_PUZZLE",
        &mut conditions,
        &mut explanation,
        &mut Vec::new(),
    );
    add_signature_conditions(
        &conds.agg_sig_amount,
        "AGG_SIG_AMOUNT",
        &mut conditions,
        &mut explanation,
        &mut Vec::new(),
    );
    add_signature_conditions(
        &conds.agg_sig_puzzle_amount,
        "AGG_SIG_PUZZLE_AMOUNT",
        &mut conditions,
        &mut explanation,
        &mut Vec::new(),
    );
    add_signature_conditions(
        &conds.agg_sig_parent_amount,
        "AGG_SIG_PARENT_AMOUNT",
        &mut conditions,
        &mut explanation,
        &mut Vec::new(),
    );
    add_signature_conditions(
        &conds.agg_sig_parent_puzzle,
        "AGG_SIG_PARENT_PUZZLE",
        &mut conditions,
        &mut explanation,
        &mut Vec::new(),
    );

    for (puzzle_hash, amount, hint) in create_coin {
        let new_coin = Coin::new(conds.coin_id, puzzle_hash, amount);
        let coin_ref = coin_ref_from_coin(&new_coin);
        let has_hint = hint.is_some();
        let mut args = vec![json!(encode_hex_prefixed(puzzle_hash.as_ref())), json!(amount)];
        if let Some(ref hint) = hint {
            args.push(json!([encode_hex_prefixed(hint.as_ref())]));
        }
        conditions.push(ConditionInfo {
            opcode: "CREATE_COIN".to_string(),
            args,
            raw: None,
        });
        additions.push(coin_ref.clone());
        explanation.value_flow.push(json!({
            "action": "create_coin",
            "puzzle_hash": coin_ref.puzzle_hash,
            "amount": coin_ref.amount,
            "memos_present": has_hint,
        }));
    }

    add_optional_assertion(
        "ASSERT_HEIGHT_RELATIVE",
        conds.height_relative.map(u64::from),
        &mut conditions,
        &mut explanation,
    );
    add_optional_assertion(
        "ASSERT_SECONDS_RELATIVE",
        conds.seconds_relative,
        &mut conditions,
        &mut explanation,
    );
    add_optional_assertion(
        "ASSERT_BEFORE_HEIGHT_RELATIVE",
        conds.before_height_relative.map(u64::from),
        &mut conditions,
        &mut explanation,
    );
    add_optional_assertion(
        "ASSERT_BEFORE_SECONDS_RELATIVE",
        conds.before_seconds_relative,
        &mut conditions,
        &mut explanation,
    );
    add_optional_assertion(
        "ASSERT_MY_BIRTH_HEIGHT",
        conds.birth_height.map(u64::from),
        &mut conditions,
        &mut explanation,
    );
    add_optional_assertion(
        "ASSERT_MY_BIRTH_SECONDS",
        conds.birth_seconds,
        &mut conditions,
        &mut explanation,
    );

    if explain_level == ExplainLevel::Conditions {
        explanation.constraints.clear();
    }

    let puzzle_hash = tree_hash_from_bytes(spend.puzzle_reveal.as_ref())
        .map(|h| encode_hex_prefixed(h.as_ref()))
        .unwrap_or_else(|_| encode_hex_prefixed(spend.coin.puzzle_hash.as_ref()));

    let puzzle_behavior = PuzzleBehavior {
        clvm: ClvmBehavior {
            puzzle_reveal_bytes: encode_hex_prefixed(spend.puzzle_reveal.as_ref()),
            solution_bytes: encode_hex_prefixed(spend.solution.as_ref()),
            puzzle_opd: puzzle_disasm.clone(),
            solution_opd: solution_disasm.clone(),
            uses_backrefs,
            serialized_len_bytes: spend.puzzle_reveal.len(),
        },
        static_features,
        dynamic: DynamicBehavior {
            status: "ok".to_string(),
            cost: conds.execution_cost + conds.condition_cost,
            conditions: conditions.clone(),
            created_coins: additions.clone(),
            failure: None,
        },
        explanation,
    };

    SpendAnalysis {
        coin_spend: CoinSpendView {
            coin: coin_ref,
            puzzle_reveal: encode_hex_prefixed(spend.puzzle_reveal.as_ref()),
            solution: encode_hex_prefixed(spend.solution.as_ref()),
        },
        puzzle: PuzzleInfo {
            id: PuzzleId {
                puzzle_hash: encode_hex_prefixed(spend.coin.puzzle_hash.as_ref()),
                tree_hash: puzzle_hash.clone(),
                shatree: puzzle_hash,
            },
            recognition,
            puzzle_reveal_disasm: puzzle_disasm,
            solution_disasm: solution_disasm,
        },
        evaluation: EvaluationInfo {
            status: "ok".to_string(),
            cost: conds.execution_cost + conds.condition_cost,
            conditions,
            additions,
            announcements: Vec::new(),
            assertions: Vec::new(),
            failure: None,
        },
        puzzle_behavior,
    }
}

fn add_signature_conditions(
    pairs: &[(chia_bls::PublicKey, Bytes)],
    opcode: &str,
    conditions: &mut Vec<ConditionInfo>,
    explanation: &mut Explanation,
    agg_sig_out: &mut Vec<AggSigInfo>,
) {
    for (pk, msg) in pairs {
        let pk_hex = encode_hex_prefixed(&pk.to_bytes());
        let msg_hex = encode_hex_prefixed(msg.as_ref());
        conditions.push(ConditionInfo {
            opcode: opcode.to_string(),
            args: vec![json!(pk_hex), json!(msg_hex)],
            raw: None,
        });
        explanation.enforced_signatures.push(json!({
            "kind": opcode,
            "pubkey": pk_hex,
            "message": msg_hex,
            "doc": "https://chialisp.com/conditions/",
        }));
        agg_sig_out.push(AggSigInfo {
            pubkey: pk_hex,
            msg: msg_hex,
        });
    }
}

fn add_optional_assertion(
    opcode: &str,
    value: Option<u64>,
    conditions: &mut Vec<ConditionInfo>,
    explanation: &mut Explanation,
) {
    if let Some(v) = value {
        conditions.push(ConditionInfo {
            opcode: opcode.to_string(),
            args: vec![json!(v)],
            raw: None,
        });
        explanation.constraints.push(json!({
            "kind": opcode,
            "value": v,
        }));
    }
}

fn compute_net_delta(removals: &[CoinRef], additions: &[CoinRef]) -> Vec<NetDelta> {
    let mut map = BTreeMap::<String, i128>::new();
    for coin in removals {
        *map.entry(coin.puzzle_hash.clone()).or_insert(0) -= i128::from(coin.amount);
    }
    for coin in additions {
        *map.entry(coin.puzzle_hash.clone()).or_insert(0) += i128::from(coin.amount);
    }

    map.into_iter()
        .map(|(puzzle_hash, delta_mojos)| NetDelta {
            puzzle_hash,
            delta_mojos,
        })
        .collect()
}

fn coin_ref_from_coin(coin: &Coin) -> CoinRef {
    CoinRef {
        coin_id: encode_hex_prefixed(coin.coin_id().as_ref()),
        parent_coin_id: encode_hex_prefixed(coin.parent_coin_info.as_ref()),
        puzzle_hash: encode_hex_prefixed(coin.puzzle_hash.as_ref()),
        amount: coin.amount,
    }
}

fn analyze_clvm_bytes(bytes: &[u8]) -> (String, StaticFeatures, bool) {
    let mut allocator = ClvmAllocator::new();
    let uses_backrefs = bytes.contains(&0xfe);

    match node_from_bytes_backrefs(&mut allocator, bytes) {
        Ok(node) => {
            let disasm = disassemble(&allocator, node, Some(OPERATORS_LATEST_VERSION));
            let features = extract_static_features(&allocator, node);
            (disasm, features, uses_backrefs)
        }
        Err(err) => (
            format!("<failed to disassemble: {err}>"),
            StaticFeatures {
                operators_used: Vec::new(),
                env_paths_used: Vec::new(),
                constants: ConstantBuckets {
                    bytes32: Vec::new(),
                    g1_pubkeys: Vec::new(),
                    small_ints: Vec::new(),
                },
            },
            uses_backrefs,
        ),
    }
}

fn extract_static_features(allocator: &ClvmAllocator, root: NodePtr) -> StaticFeatures {
    let keywords = keyword_from_atom(OPERATORS_LATEST_VERSION);
    let mut operators = BTreeSet::<String>::new();
    let mut env_paths = BTreeSet::<u32>::new();
    let mut bytes32 = BTreeSet::<String>::new();
    let mut g1_pubkeys = BTreeSet::<String>::new();
    let mut small_ints = BTreeSet::<u64>::new();

    visit_clvm(
        allocator,
        root,
        false,
        false,
        keywords,
        &mut operators,
        &mut env_paths,
        &mut bytes32,
        &mut g1_pubkeys,
        &mut small_ints,
    );

    StaticFeatures {
        operators_used: operators.into_iter().collect(),
        env_paths_used: env_paths.into_iter().collect(),
        constants: ConstantBuckets {
            bytes32: bytes32.into_iter().collect(),
            g1_pubkeys: g1_pubkeys.into_iter().collect(),
            small_ints: small_ints.into_iter().collect(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn visit_clvm(
    allocator: &ClvmAllocator,
    node: NodePtr,
    in_quoted: bool,
    operator_position: bool,
    keywords: &std::collections::HashMap<Vec<u8>, String>,
    operators: &mut BTreeSet<String>,
    env_paths: &mut BTreeSet<u32>,
    bytes32: &mut BTreeSet<String>,
    g1_pubkeys: &mut BTreeSet<String>,
    small_ints: &mut BTreeSet<u64>,
) {
    match allocator.sexp(node) {
        SExp::Pair(left, right) => {
            let is_quote = !in_quoted
                && matches!(allocator.sexp(left), SExp::Atom)
                && allocator.atom(left).as_ref() == [1_u8];

            if !in_quoted && matches!(allocator.sexp(left), SExp::Atom) {
                let atom = allocator.atom(left);
                if let Some(name) = keywords.get(atom.as_ref()) {
                    operators.insert(name.clone());
                }
            }

            visit_clvm(
                allocator,
                left,
                in_quoted,
                true,
                keywords,
                operators,
                env_paths,
                bytes32,
                g1_pubkeys,
                small_ints,
            );
            visit_clvm(
                allocator,
                right,
                in_quoted || is_quote,
                false,
                keywords,
                operators,
                env_paths,
                bytes32,
                g1_pubkeys,
                small_ints,
            );
        }
        SExp::Atom => {
            let atom = allocator.atom(node);
            let bytes = atom.as_ref();

            if bytes.len() == 32 {
                bytes32.insert(encode_hex_prefixed(bytes));
            } else if bytes.len() == 48 {
                g1_pubkeys.insert(encode_hex_prefixed(bytes));
            }

            if let Some(value) = atom_to_u64(bytes) {
                if value <= 1_000_000 {
                    small_ints.insert(value);
                }
                if !in_quoted && !operator_position && value > 0 && value <= u64::from(u32::MAX) {
                    env_paths.insert(value as u32);
                }
            }
        }
    }
}

fn atom_to_u64(atom: &[u8]) -> Option<u64> {
    if atom.is_empty() {
        return Some(0);
    }
    if atom[0] & 0x80 != 0 {
        return None;
    }
    if atom.len() > 8 {
        return None;
    }
    let mut v = 0_u64;
    for b in atom {
        v = (v << 8) | u64::from(*b);
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chia_protocol::Program;

    #[test]
    fn atom_u64_parser() {
        assert_eq!(atom_to_u64(&[]), Some(0));
        assert_eq!(atom_to_u64(&[0x7f]), Some(127));
        assert_eq!(atom_to_u64(&[0x00, 0x80]), Some(128));
        assert_eq!(atom_to_u64(&[0xff]), None);
    }

    #[test]
    fn analyze_clvm_smoke() {
        let program = Program::from(vec![0xff, 0x01, 0x01]);
        let (_disasm, features, _backrefs) = analyze_clvm_bytes(program.as_ref());
        assert!(features.operators_used.iter().any(|op| op == "q"));
    }
}
