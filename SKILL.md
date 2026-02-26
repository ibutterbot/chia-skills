---
name: chia-skills
description: End-to-end Chia transaction analysis using coinset for chain data acquisition, chia-inspect for consensus and puzzle parsing, and clvm-workbench for raw CLVM debugging. Use when the user asks about blocks, mempool items, coin spends, spend bundles, CAT/singleton/DID/NFT layers, or CLVM decoding.
homepage: https://github.com/xch-dev/chia-wallet-sdk
metadata: { "openclaw": { "emoji": "🌱", "os": ["linux"], "workspace": "/home/ubuntu/chia-skills" } }
---

# chia-skills (coinset + chia-inspect + clvm-workbench)

Use this single skill for the full Chia analysis workflow:

1. Acquire blockchain data with `coinset`.
2. Interpret transaction semantics with `chia-inspect`.
3. Escalate to `clvm-workbench` for low-level CLVM debugging.

## Trigger cues

Use this skill when user intent includes:

- "coinset", "full node RPC", "query Chia chain data"
- "inspect spend bundle", "mempool item analysis", "analyze block spends"
- "coin spend", "puzzle reveal", "solution", "conditions", "cost"
- "identify CAT/singleton/DID/NFT wrappers/layers"
- "decode CLVM", "opd/opc/brun", "debug unknown puzzle internals"

## Tool roles

### 1) `coinset` (data acquisition)

Use `coinset` for RPC retrieval, id/address utilities, and optional `jq` filtering.

Canonical shape:

```bash
coinset <rpc_name> [rpc_args...] [flags...]
coinset <rpc_name> ... -q '<jq_filter>'
coinset <rpc_name> --help
```

Common queries:

```bash
coinset get_blockchain_state
coinset get_network_info
coinset get_all_mempool_tx_ids
coinset get_coin_record_by_name <0xCOIN_NAME>
coinset get_coin_records_by_puzzle_hash <0xPUZZLE_HASH_OR_ADDRESS>
coinset get_block_record_by_height <height>
coinset get_block_records <start_height> <end_height>
coinset address encode <0xPUZZLE_HASH>
coinset address decode <xch_address>
coinset coin_id <0xPARENT_COIN_ID> <0xPUZZLE_HASH> <amount>
```

Direct HTTP fallback:

- Mainnet: `https://api.coinset.org`
- Testnet11: `https://testnet11.api.coinset.org`

```bash
curl https://api.coinset.org/get_blockchain_state -d '{}'
```

### 2) `chia-inspect` (transaction truth)

Use `chia-inspect` to convert mempool/block/coin blobs into deterministic JSON:

- removals/additions and fee
- consensus-evaluated conditions and cost
- per-spend puzzle composition (`recognition.wrappers`, `candidates`, `parsed_solution`)
- per-spend raw CLVM behavior (`puzzle_behavior`)

Canonical invocations:

```bash
chia-inspect mempool --blob-json <PATH|-> --pretty
chia-inspect block --spends-json <PATH|-> --pretty
chia-inspect coin --coin-spend-json <PATH|-> --pretty
```

Accepted mempool input shapes:

- `{ "spend_bundle": { ... } }`
- `{ "coin_spends": [...], "aggregated_signature": "0x..." }`
- `{ "spend_bundle_bytes": "0x..." }`
- `{ "mempool_item": { "spend_bundle": { ... } } }`
- `{ "mempool_item": { "spend_bundle_bytes": "0x..." } }`

### 3) `clvm-workbench` (raw CLVM microscope)

Use when deeper CLVM analysis is still needed after `chia-inspect`.

Canonical invocations:

```bash
clvm-workbench opd 0xff01ff02ff8080
clvm-workbench opc "(q . 1)"
clvm-workbench run --program "<clvm_or_hex>" --env "<clvm_or_hex>"
clvm-workbench run --program "<clvm_or_hex>" --env "<clvm_or_hex>" --cost --verbose
```

## Recommended workflow

1. Fetch data with `coinset`.
2. Pipe or save JSON into `chia-inspect`.
3. Explain behavior using this strict order:
   - `evaluation.conditions` and `evaluation.cost`
   - `recognition.wrappers` / `recognition.candidates` / `recognition.parsed_solution`
   - `puzzle_behavior` raw CLVM details
4. Use `clvm-workbench` only for unresolved low-level mechanics.

## High-value pipeline examples

Mempool item from `coinset` -> `chia-inspect`:

```bash
coinset get_mempool_item_by_tx_id <0xTX_ID> | chia-inspect mempool --blob-json - --pretty
```

Coin record lookup then inspection (if spend blob is available):

```bash
coinset get_coin_record_by_name <0xCOIN_NAME>
chia-inspect coin --coin-spend-json coin_spend.json --pretty
```

## Output fields to trust

Use these fields as authoritative:

- `result.status`, `result.error`
- `result.summary.removals`, `result.summary.additions`, `result.summary.fee_mojos`
- `result.spends[].evaluation.cost`, `result.spends[].evaluation.conditions`
- `result.spends[].puzzle.recognition.wrappers`
- `result.spends[].puzzle.recognition.candidates`
- `result.spends[].puzzle.recognition.parsed_solution`
- `result.spends[].puzzle_behavior.dynamic.conditions`

## Failure policy

- If shape/keys are invalid, report exactly which required keys are missing.
- If decode/eval fails, return the exact tool error and request corrected input.
- If recognition is ambiguous, report ambiguity and avoid definitive wrapper claims.
- If `parse_error` appears on a wrapper, keep successful layers but mark confidence as partial.
- Do not infer transaction intent from incomplete spend data.

## Notes

- Current inspection schema: `chia.inspect.spendbundle.v2`.
- `chia-inspect` and `clvm-workbench` are local binaries built from `/home/ubuntu/chia-skills`.
- `coinset` provides acquisition only; interpretation should be anchored in `chia-inspect`.
