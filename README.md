# chia-skills

Unified Chia transaction tooling:

- `coinset` for Chia RPC data acquisition
- `chia-inspect` for spend-bundle/coin-spend interpretation
- `clvm-workbench` for raw CLVM debugging
- `SKILL.md` for OpenClaw skill integration

## What this contains

- `chia-inspect`: Offline-first inspector for mempool/block/coin spend blobs.
- `clvm-workbench`: Raw CLVM utility CLI (`opd`, `opc`, `run`).
- `chia-inspect-core`: Shared parsing, CLVM analysis, consensus evaluation, and JSON output logic.

## Workspace layout

```text
chia-skills/
  Cargo.toml
  crates/
    chia-inspect-core/
    chia-inspect/
    clvm-workbench/
```

## Install

### 1) Install `coinset`

Recommended:

```bash
go install github.com/coinset-org/cli/cmd/coinset@latest
```

Alternative:

```bash
brew install coinset-org/cli/coinset
```

### 2) Install `chia-inspect` and `clvm-workbench` binaries

#### Option A (recommended): GitHub release binaries

Linux x86_64 example:

```bash
REPO="ibutterbot/chia-skills"
gh release download --repo "$REPO" --pattern "chia-skills-*-x86_64-unknown-linux-gnu.tar.gz" --clobber
tar -xzf chia-skills-*-x86_64-unknown-linux-gnu.tar.gz
install -m 0755 chia-inspect ~/.local/bin/chia-inspect
install -m 0755 clvm-workbench ~/.local/bin/clvm-workbench
```

macOS and Windows release assets are also published:

- `chia-skills-<tag>-aarch64-apple-darwin.tar.gz`
- `chia-skills-<tag>-x86_64-pc-windows-msvc.zip`

#### Option B: build/install from source

```bash
cd /home/ubuntu/chia-skills
cargo install --locked --path crates/chia-inspect
cargo install --locked --path crates/clvm-workbench
```

### 3) Install this OpenClaw skill from this repo

```bash
mkdir -p ~/.openclaw/skills/chia-skills
cp /home/ubuntu/chia-skills/SKILL.md ~/.openclaw/skills/chia-skills/SKILL.md
```

Enable `chia-skills` in `~/.openclaw/openclaw.json` under `skills.entries`.

### 4) Verify install

```bash
coinset version
chia-inspect --help
clvm-workbench --help
```

## Build (contributors)

```bash
cd /home/ubuntu/chia-skills
cargo build --workspace
```

## Quickstart

### 1) Inspect a mempool/blob JSON

```bash
chia-inspect mempool --blob-json path/to/mempool_blob.json --pretty
```

### 2) Inspect block spend entries JSON

```bash
chia-inspect block --spends-json path/to/block_spends.json --pretty
```

### 3) Inspect a single coin spend JSON

```bash
chia-inspect coin --coin-spend-json path/to/coin_spend.json --pretty
```

## Using with coinset

`chia-inspect` is offline-first on purpose. Use `coinset` to fetch, then pass JSON to `chia-inspect`.

### Example flow

```bash
# 1) Fetch from coinset using your preferred RPC call.
#    Save raw JSON output to a file.
coinset <rpc_name> <rpc_args...> > /tmp/input.json

# 2) Run the inspector against that blob.
chia-inspect mempool --blob-json /tmp/input.json --pretty
```

### Accepted mempool blob shapes

- `{ "spend_bundle": { ... } }`
- `{ "coin_spends": [...], "aggregated_signature": "0x..." }`
- `{ "spend_bundle_bytes": "0x..." }`
- `{ "mempool_item": { "spend_bundle": { ... } } }`
- `{ "mempool_item": { "spend_bundle_bytes": "0x..." } }`

### Accepted block shapes

- `{ "coin_spends": [...] }`
- `{ "block_spends": [...] }`
- `[ { "coin": ..., "puzzle_reveal": ..., "solution": ... }, ... ]`

### Accepted coin shapes

- `{ "coin": ..., "puzzle_reveal": ..., "solution": ... }`
- `{ "coin_spend": { ... } }`

## clvm-workbench usage

```bash
# bytes -> CLVM
clvm-workbench opd 0x01

# CLVM -> bytes
clvm-workbench opc "(q . 1)"

# run CLVM
clvm-workbench run --program "(q . 1)" --env "()"
clvm-workbench run --program "(q . 1)" --env "()" --cost --verbose
```

## Output

`chia-inspect` emits schema version `chia.inspect.spendbundle.v2` and includes:

- SpendBundle-level summary (removals/additions/fee/net deltas).
- Per-spend CLVM and semantic analysis under `result.spends[].puzzle_behavior`.
- Consensus-derived conditions and cost.
- Wallet-SDK powered puzzle recognition under `result.spends[].puzzle.recognition`:
  - `wrappers[]`: ordered outer-to-inner layer stack with extracted params and source paths.
  - `candidates[]`: detected layer candidates with confidence.
  - `parsed_solution`: per-layer parsed solution details aligned to the wrapper stack.

Schema migration notes (`v1` -> `v2`):

- `result.spends[].puzzle.recognition.wrappers` changed from `string[]` to structured `WrapperInfo[]`.
- `result.spends[].puzzle.recognition.parsed_solution` is new.
- `result.spends[].puzzle.recognition.candidates[]` is retained and now complements wrapper details.

Common composition examples:

- CAT + standard: `["cat_layer", "standard_layer"]`
- Singleton DID: `["singleton_layer", "did_layer", "standard_layer"]`
- Singleton NFT: `["singleton_layer", "nft_state_layer", "nft_ownership_layer", "standard_layer"]`

Current detector coverage:

- `cat_layer`
- `singleton_layer`
- `did_layer`
- `nft_state_layer`
- `nft_ownership_layer`
- `royalty_transfer_layer`
- `augmented_condition_layer`
- `bulletin_layer`
- `option_contract_layer`
- `revocation_layer`
- `p2_singleton_layer`
- `p2_curried_layer`
- `p2_one_of_many_layer`
- `p2_delegated_conditions_layer`
- `settlement_layer`
- `stream_layer`
- `standard_layer`

Interpretation contract:

- Treat `result.spends[].evaluation.conditions` and `.cost` as consensus-truth semantics.
- Use recognition (`wrappers`, `candidates`, `parsed_solution`) as composition metadata layered on top of consensus truth.
- Candidate confidence semantics:
  - `1.0`: single clear layer match
  - `0.8`: layer matched, but solution parsing had an error
  - `0.5`: ambiguous multi-layer match at same peel depth

`parsed_solution` behavior:

- Best-effort and non-fatal: failures do not fail inspection output.
- Includes per-layer parse results, decode errors when present, and a summary for any remaining undecoded solution.

Quick triage snippets:

```bash
# Show wrapper composition per spend
chia-inspect mempool --blob-json /tmp/input.json --pretty \
  | jq -r '.result.spends[] | .puzzle.recognition.wrappers | map(.name) | join(" -> ")'

# Show layer parse errors (if any)
chia-inspect mempool --blob-json /tmp/input.json --pretty \
  | jq '.result.spends[] | .puzzle.recognition.wrappers[] | select(.parse_error != null) | {name, parse_error}'
```

Current limitations:

- Feature-gated Wallet SDK detectors are not enabled in this build (`chip-0035` datalayer, `action-layer`).
- Recognition may be partial/ambiguous for novel compositions; when that happens, use `clvm-workbench` for deeper raw-CLVM analysis.

## Tests

```bash
cd /home/ubuntu/chia-skills
cargo test --workspace
```

Includes:

- Input shape normalization tests.
- CLVM feature extraction tests.
- Golden JSON snapshot test for deterministic output.

## Skill doc in this repo

- `SKILL.md`
