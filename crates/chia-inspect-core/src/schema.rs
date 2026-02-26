use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct InspectionOutput {
    pub schema_version: String,
    pub tool: ToolInfo,
    pub network: NetworkInfo,
    pub input: InputInfo,
    pub result: ResultInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInfo {
    pub name: String,
    pub genesis_challenge: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputInfo {
    pub source: SourceInfo,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub kind: String,
    pub value: Option<String>,
    pub rpc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultInfo {
    pub status: String,
    pub error: Option<ErrorInfo>,
    pub summary: Summary,
    pub spends: Vec<SpendAnalysis>,
    pub signatures: SignatureSummary,
    pub offer: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorInfo {
    pub kind: String,
    pub message: String,
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Summary {
    pub removals: Vec<CoinRef>,
    pub additions: Vec<CoinRef>,
    pub fee_mojos: u64,
    pub net_xch_delta_by_puzzle_hash: Vec<NetDelta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetDelta {
    pub puzzle_hash: String,
    pub delta_mojos: i128,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpendAnalysis {
    pub coin_spend: CoinSpendView,
    pub puzzle: PuzzleInfo,
    pub evaluation: EvaluationInfo,
    pub puzzle_behavior: PuzzleBehavior,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoinSpendView {
    pub coin: CoinRef,
    pub puzzle_reveal: String,
    pub solution: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoinRef {
    pub coin_id: String,
    pub parent_coin_id: String,
    pub puzzle_hash: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PuzzleInfo {
    pub id: PuzzleId,
    pub recognition: PuzzleRecognition,
    pub puzzle_reveal_disasm: String,
    pub solution_disasm: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PuzzleId {
    pub puzzle_hash: String,
    pub tree_hash: String,
    pub shatree: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PuzzleRecognition {
    pub recognized: bool,
    pub candidates: Vec<PuzzleCandidate>,
    pub wrappers: Vec<WrapperInfo>,
    pub parsed_solution: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WrapperInfo {
    pub name: String,
    pub source_repo: String,
    pub source_ref: String,
    pub source_path: Option<String>,
    pub mod_hash: String,
    pub curried_args_tree_hash: Option<String>,
    pub inner_puzzle_tree_hash: Option<String>,
    pub params: Value,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PuzzleCandidate {
    pub name: String,
    pub confidence: f64,
    pub source_repo: Option<String>,
    pub source_path: Option<String>,
    pub source_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvaluationInfo {
    pub status: String,
    pub cost: u64,
    pub conditions: Vec<ConditionInfo>,
    pub additions: Vec<CoinRef>,
    pub announcements: Vec<Value>,
    pub assertions: Vec<Value>,
    pub failure: Option<FailureInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConditionInfo {
    pub opcode: String,
    pub args: Vec<Value>,
    pub raw: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureInfo {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SignatureSummary {
    pub aggregated_signature: String,
    pub agg_sig_me: Vec<AggSigInfo>,
    pub agg_sig_unsafe: Vec<AggSigInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggSigInfo {
    pub pubkey: String,
    pub msg: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PuzzleBehavior {
    pub clvm: ClvmBehavior,
    pub static_features: StaticFeatures,
    pub dynamic: DynamicBehavior,
    pub explanation: Explanation,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClvmBehavior {
    pub puzzle_reveal_bytes: String,
    pub solution_bytes: String,
    pub puzzle_opd: String,
    pub solution_opd: String,
    pub uses_backrefs: bool,
    pub serialized_len_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticFeatures {
    pub operators_used: Vec<String>,
    pub env_paths_used: Vec<u32>,
    pub constants: ConstantBuckets,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConstantBuckets {
    pub bytes32: Vec<String>,
    pub g1_pubkeys: Vec<String>,
    pub small_ints: Vec<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DynamicBehavior {
    pub status: String,
    pub cost: u64,
    pub conditions: Vec<ConditionInfo>,
    pub created_coins: Vec<CoinRef>,
    pub failure: Option<FailureInfo>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Explanation {
    pub enforced_signatures: Vec<Value>,
    pub value_flow: Vec<Value>,
    pub constraints: Vec<Value>,
}
