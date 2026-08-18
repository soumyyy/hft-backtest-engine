use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use csv::{ReaderBuilder, Writer, WriterBuilder};
use std::collections::hash_map::{Entry, HashMap};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub fn ingest_data(
    input_dir: PathBuf,
    output_dir: PathBuf,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Result<()> {
    info!("Starting data ingestion process from {:?}", input_dir);

    fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

    let csv_files = find_csv_files(&input_dir)?;
    if csv_files.is_empty() {
        warn!("No CSV files found in {:?}", input_dir);
        return Ok(());
    }

    let filters = DateFilters::new(start_date.as_deref(), end_date.as_deref())?;

    let mut total_records = 0usize;
    let mut written_records = 0usize;

    for csv_file in csv_files {
        info!("Processing file: {:?}", csv_file);
        let (processed, kept) = process_csv_file(&csv_file, &output_dir, &filters)?;
        total_records += processed;
        written_records += kept;
    }

    info!(
        "Ingestion complete: processed {} rows, wrote {} rows to {:?}",
        total_records,
        written_records,
        output_dir
    );

    Ok(())
}

fn find_csv_files(input_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut csv_files = Vec::new();

    for entry in fs::read_dir(input_dir)
        .with_context(|| format!("Failed to read input directory {:?}", input_dir))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("csv") {
            csv_files.push(path);
        }
    }

    csv_files.sort();
    Ok(csv_files)
}

struct DateFilters {
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
}

impl DateFilters {
    fn new(start: Option<&str>, end: Option<&str>) -> Result<Self> {
        let start = match start {
            Some(value) => Some(NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .with_context(|| format!("Invalid start_date '{}'. Expected YYYY-MM-DD", value))?),
            None => None,
        };

        let end = match end {
            Some(value) => Some(NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .with_context(|| format!("Invalid end_date '{}'. Expected YYYY-MM-DD", value))?),
            None => None,
        };

        if let (Some(start_date), Some(end_date)) = (start, end) {
            anyhow::ensure!(
                start_date <= end_date,
                "start_date must be earlier than or equal to end_date"
            );
        }

        Ok(Self { start, end })
    }

    fn includes(&self, date: NaiveDate) -> bool {
        if let Some(start) = self.start {
            if date < start {
                return false;
            }
        }

        if let Some(end) = self.end {
            if date > end {
                return false;
            }
        }

        true
    }
}

fn process_csv_file(
    csv_file: &Path,
    output_dir: &Path,
    filters: &DateFilters,
) -> Result<(usize, usize)> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_path(csv_file)
        .with_context(|| format!("Failed to open {:?}" , csv_file))?;

    let mut writers: HashMap<String, Writer<BufWriter<File>>> = HashMap::new();
    let mut processed = 0usize;
    let mut written = 0usize;
    let mut per_day_counts: HashMap<String, usize> = HashMap::new();

    for record in reader.records() {
        processed += 1;
        let record = record.with_context(|| format!("Failed to read row {} from {:?}", processed, csv_file))?;

        let timestamp: i64 = record
            .get(0)
            .context("Missing timestamp column")?
            .parse()
            .with_context(|| format!("Invalid timestamp value at row {}", processed))?;
        let ask_price: f64 = record
            .get(1)
            .context("Missing askPrice column")?
            .parse()
            .with_context(|| format!("Invalid askPrice value at row {}", processed))?;
        let bid_price: f64 = record
            .get(2)
            .context("Missing bidPrice column")?
            .parse()
            .with_context(|| format!("Invalid bidPrice value at row {}", processed))?;

        if ask_price <= 0.0 || bid_price <= 0.0 {
            continue;
        }

        let trade_date = DateTime::<Utc>::from_timestamp_millis(timestamp)
            .with_context(|| format!("Failed to convert timestamp {} to datetime", timestamp))?
            .date_naive();

        if !filters.includes(trade_date) {
            continue;
        }

        let date_key = trade_date.format("%Y-%m-%d").to_string();
        let writer = match writers.entry(date_key.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let file_path = output_dir.join(format!("xauusd_{}.csv", date_key));
                let file = File::create(&file_path)
                    .with_context(|| format!("Failed to create {:?}", file_path))?;
                let mut writer = WriterBuilder::new()
                    .has_headers(false)
                    .from_writer(BufWriter::new(file));
                writer
                    .write_record(["timestamp", "askPrice", "bidPrice", "spread", "midPrice"])
                    .with_context(|| format!("Failed to write header to {:?}", file_path))?;
                entry.insert(writer)
            }
        };

        let spread = ask_price - bid_price;
        let mid_price = (ask_price + bid_price) / 2.0;

        writer
            .write_record([
                timestamp.to_string(),
                format!("{:.3}", ask_price),
                format!("{:.3}", bid_price),
                format!("{:.6}", spread),
                format!("{:.6}", mid_price),
            ])
            .with_context(|| format!("Failed to write tick for {}", date_key))?;

        *per_day_counts.entry(date_key).or_default() += 1;
        written += 1;
    }

    for writer in writers.values_mut() {
        writer.flush()?;
    }

    if per_day_counts.is_empty() {
        warn!("No rows from {:?} matched the requested date range", csv_file);
    } else {
        let mut counts: Vec<_> = per_day_counts.into_iter().collect();
        counts.sort_by_key(|(date, _)| date.clone());
        for (date, count) in counts {
            info!("\u{2022} {}: wrote {} ticks", date, count);
        }
    }

    Ok((processed, written))
}
