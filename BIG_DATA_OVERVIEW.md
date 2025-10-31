## Big-Data Architecture in the HFT Backtesting Engine

This engine is built around real-world tick data volumes that push beyond typical toy “backtest” scripts. The stack, file layout, and processing choices are all tuned so we can ingest, store, and analyze hundreds of millions of price observations without rewriting the system each time the dataset grows.

### 1. Raw Market Data Acquisition

- **High-frequency XAU/USD ticks**: We pull Dukascopy BI5 files for every hour between two dates. A single day comprises 24 compressed binaries. Each file expands into tens of thousands of ticks, so a month of gold data lands in the tens of millions of rows.
- **Streaming download script (`scripts/download_xauusd_ticks.py`)**: Fetches hour-by-hour, decompresses with `lzma`, and unpacks binary tick structures. The script writes one CSV per day—keeping the file system structure partition-friendly for downstream batch jobs and allowing incremental reruns by date.

### 2. Ingestion and Normalization (`src/ingest.rs`)

- **Partition-per-day output**: Raw CSVs are validated, filtered by date, enriched with derived columns (spread, mid price), and written back out as daily CSVs (drop-in replacement for Parquet once storage needs demand it).
- **Memory-aware streaming**: We use `csv::Reader` with row iteration and buffered writers, so we never load an entire file into memory. Even large day files (millions of ticks) are processed in constant memory.
- **Robust error handling**: The ingestion pipeline attaches granular context to parse/IO errors, keeping data corrections manageable even when dealing with massive tick logs.

### 3. Backtesting Engine (`src/engine.rs`)

- **Sequential tick processing**: The engine streams each day’s file line-by-line, applying strategy logic without materializing the whole dataset. This is crucial for scaling to weeks or months of tick data on an 8 GB machine.
- **Stateful strategies**: Position and price history tracking uses `VecDeque` buffers capped at configurable window sizes. This preserves recent context for indicators while preventing unbounded memory growth.
- **Run artifacts**: Each simulation saves raw trades (JSON) and summaries (YAML) into timestamped run folders, so we can layer analytics across many large runs without recomputation.

### 4. Analytics Pipeline (`src/analyze.rs`, `analyze_results.py`)

- **Columnar analytics with Polars**: Rust and Python components both rely on Polars (via the `polars` Rust crate and `polars` Python package) to perform vectorized aggregations over large trade logs. Lazy execution keeps transformations efficient and lets us compose complex metrics without hand-tuned loops.
- **Extensible reporting**: Summary generation is ready for additional metrics (drawdown curves, volatility clustering, etc.). Because trades are stored in a structured format, we can regenerate analytics across many gigabytes of runs quickly.

### 5. Tooling and Automation

- **`run_analysis.sh` orchestration**: Automates multi-strategy workflows—running the backtest, launching the analyzer, and surfacing key metrics. This reduces manual intervention, which becomes vital when each run represents millions of ticks.
- **Configuration-driven experiments**: Strategy YAML files encapsulate parameters, letting us sweep large grids of settings without code changes. We can schedule batches of runs simply by iterating over config files.

### 6. Scaling Beyond the Current Dataset

The current demos process ~733 K ticks in a second, but the architecture is designed for much more:

- Swap the per-day CSV outputs with Parquet or Arrow for better compression and predicate pushdown.
- Store run artifacts in object storage (S3, MinIO) and read lazily with Polars.
- Parallelize strategy execution across CPU cores or distribute via a task queue, since each run directory is isolated.
- Introduce incremental ingestion: because raw downloads are hourly files, we can add fresh data without reprocessing the entire history.

### 7. Summary

By treating tick data as a big-data problem from the start—partitioning storage, streaming through files, and leaning on vectorized analytics frameworks—we keep the backtesting engine robust as the dataset scales. The current infrastructure already handles weeks of millisecond-level data comfortably, and it creates a clear path to months or years of history with minimal changes.
