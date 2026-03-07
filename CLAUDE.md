# Fundamentals

A Rust workspace that fetches SEC EDGAR fundamental data for stock tickers, suitable for
CANSLIM, SEPA, and Episodic Pivot analysis.

## Status

The `edgar` crate is **complete and validated against all 100 NASDAQ 100 tickers**.
`runner` exposes it as a CLI binary. The `scraper` crate (TradingView Chrome automation)
exists but is not yet integrated.

## Workspace Structure

- `runner/` — binary; takes ticker as CLI arg, prints `EdgarFundamentals` JSON to stdout
- `edgar/` — SEC EDGAR client; fully implemented
- `scraper/` — TradingView browser scraper; not yet integrated
- `model/` — older shared types; superseded by `edgar/src/types.rs`
- `config/` — `config.toml` singleton via `OnceLock`

## Build & Run

```bash
# from workspace root (config.toml must be present here)
cargo build --release
cargo run --release -- AAPL          # stdout = JSON, stderr = logs
RUST_LOG=debug cargo run -- AAPL    # verbose
```

## Key Files

| File | Purpose |
|------|---------|
| `edgar/src/lib.rs` | Public API: `fetch_fundamentals(ticker) -> Result<EdgarFundamentals>` |
| `edgar/src/types.rs` | All structs with serde derives |
| `edgar/src/client.rs` | `EdgarClient`: reqwest + 110ms rate limit + User-Agent header |
| `edgar/src/fetch.rs` | Calls EDGAR API, builds typed structs from parsed facts |
| `edgar/src/parse.rs` | `extract_facts()` — XBRL filtering, dedup; `parse_form4_xml()` |
| `edgar/src/compute.rs` | YoY/QoQ growth, margins, FCF, ROE, EPS acceleration |
| `runner/src/main.rs` | Tracing setup, calls `edgar::fetch_fundamentals`, prints JSON |

## Validation Workflow

To re-validate Rust output against raw EDGAR data:

```bash
# 1. Download raw EDGAR companyfacts JSON for all NASDAQ 100 tickers
python3 download_fixtures.py        # saves to edgar_fixtures/{TICKER}.json

# 2. Run Rust for a ticker and save output
cargo run --release -- AAPL 2>/dev/null > rust_outputs/AAPL.json

# 3. Verify: compare Rust output against fixture using same parsing logic
python3 verify.py AAPL              # or: python3 verify.py  (all tickers)
```

These scripts don't exist in the repo — they were used for one-off validation and deleted.
Re-create them if needed; the logic is straightforward:
- `download_fixtures.py`: fetch `https://data.sec.gov/api/xbrl/companyfacts/{CIK}.json`
  for each ticker (110ms delay, User-Agent header required)
- `verify.py`: apply the same XBRL filtering rules as `parse.rs` to the fixture JSON,
  compare key fields against the Rust output JSON, report mismatches

Last validation result: **100/100 clean** (March 2026).

## Implementation Notes

### Critical: JSON pointer and `USD/shares`
`serde_json::Value::pointer` follows RFC 6901 — `/` is a path separator. The EDGAR unit
key `"USD/shares"` contains a literal slash and **must be escaped**:
```rust
let unit_esc = unit.replace('/', "~1");
facts.pointer(&format!("/us-gaap/{concept}/units/{unit_esc}"))
```
Without this, all EPS lookups silently return `None`. Fixed in `edgar/src/parse.rs`.

### XBRL single-quarter filter
EDGAR stores both single-quarter and YTD cumulative entries under the same end date for 10-Qs.
Single-quarter entries have a `frame` field (e.g. `"CY2025Q1"`); YTD entries do not.
**Accept a 10-Q entry if: has `frame` OR duration ≤ 120 days.**
For 10-K: accept if duration is 300–400 days.

### YoY matching: date-proximity
EDGAR's `fy` field = "fiscal year of the next 10-K", not the period itself — unreliable.
Use date proximity instead: find the entry closest to `current − 365 days` within ±30 days.
Implemented as `find_yoy_index()` in `edgar/src/compute.rs`.

### EPS fallback
When `EarningsPerShareDiluted` is absent (e.g. ABNB, MNST), `compute.rs` computes
`eps_diluted = net_income / diluted_shares` as a proxy. `eps_basic` has no fallback.

### Concept fallback chains (fetch.rs)
- EPS diluted: `EarningsPerShareDiluted` → `EarningsPerShareBasicAndDiluted`
- EPS basic: `EarningsPerShareBasic` → `EarningsPerShareBasicAndDiluted`
- Net income: `NetIncomeLoss` → `ProfitLoss` → `NetIncomeLossAvailableToCommonStockholdersBasic`
- Diluted shares: `WeightedAverageNumberOfDilutedSharesOutstanding` → `WeightedAverageNumberOfShareOutstandingBasicAndDiluted`
- Revenue: `Revenues` → `RevenueFromContractWithCustomerExcludingAssessedTax` → `RevenueFromContractWithCustomerIncludingAssessedTax` → `SalesRevenueNet` → `SalesRevenueGoodsNet` → `SalesRevenueServicesNet`
- OCF: `NetCashProvidedByUsedInOperatingActivities` → `NetCashProvidedByUsedInOperatingActivitiesContinuingOperations`
- ST debt: `ShortTermBorrowings` → `LongTermDebtCurrent`
- COGS: `CostOfGoodsAndServicesSold` → `CostOfRevenue`

### Known data gaps (not bugs)
- ASML, ARM, PDD, TRI, FER, CCEP: foreign IFRS filers — no US-GAAP XBRL, all arrays empty
- BKR, TTWO: `eps_diluted` None for some quarters — genuine EDGAR XBRL gap

### Deps gotchas
- **reqwest 0.13**: use `{ version = "0.13", features = ["json"] }` with default features.
  Do not set `default-features = false` — TLS feature names changed in 0.13.
- **tracing**: add `.with_writer(std::io::stderr)` to the subscriber so stdout stays clean JSON.
