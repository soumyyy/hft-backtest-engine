# HFT Backtest Engine TODO

## Phase 1: Project Scaffolding

- [ ] Create `TODO.md`
- [ ] Create directory structure: `configs`, `data_raw`, `dataset`, `runs`, `src`
- [ ] Create `Cargo.toml` with all dependencies
- [ ] Create `src/main.rs` with CLI dispatcher (ingest, run, analyze)
- [ ] Create `src/ingest.rs`, `src/engine.rs`, `src/analyze.rs` modules
- [ ] Create `configs/jan24_meanrev.yaml` and `configs/jan24_scalp.yaml`

## Phase 2: Ingest Command

- [ ] Implement `ingest` command in `src/ingest.rs`
- [ ] Read CSV from `data_raw`
- [ ] Validate and normalize data
- [ ] Partition data by day into Parquet files with Zstd compression
- [ ] Write partitioned data to `dataset`

## Phase 3: Backtesting Engine

- [ ] Implement `run` command in `src/engine.rs`
- [ ] Read partitioned Parquet data from `dataset`
- [ ] Implement streaming event loop
- [ ] Implement mean reversion and scalping strategies
- [ ] Implement cost and slippage model
- [ ] Log trades to `runs/.../trades.parquet`
- [ ] Instrument with `tracing` and `hdrhistogram`

## Phase 4: Analysis

- [ ] Implement `analyze` command in `src/analyze.rs`
- [ ] Read `trades.parquet` from a run directory
- [ ] Generate summary metrics (P/L, win rate, etc.) and save to `summary.parquet`
- [ ] Generate P/L curve and price charts with `plotters`
- [ ] Save charts to `runs/.../`
- [ ] Implement vectorized analytics with Polars LazyFrames

## Phase 5: Reproducibility and Final Touches

- [ ] Implement config loading from YAML files
- [ ] Copy config to run directory
- [ ] Record git commit hash, Rust/Polars versions
- [ ] Add `sysinfo` for memory/CPU snapshots
- [ ] Write final report and presentation slides
