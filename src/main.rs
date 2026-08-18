use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod ingest;
mod engine;
mod analyze;

#[derive(Parser)]
#[command(name = "hft-backtest")]
#[command(about = "High-Frequency Trading Backtesting Engine for XAU/USD")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest raw tick data and convert to optimized format
    Ingest {
        /// Path to raw CSV data directory
        #[arg(short, long, default_value = "data_raw")]
        input_dir: PathBuf,
        /// Output directory for processed data
        #[arg(short, long, default_value = "dataset")]
        output_dir: PathBuf,
        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        start_date: Option<String>,
        /// End date (YYYY-MM-DD)
        #[arg(long)]
        end_date: Option<String>,
    },
    /// Run backtesting simulation
    Run {
        /// Path to processed dataset
        #[arg(short, long, default_value = "dataset")]
        dataset_dir: PathBuf,
        /// Path to strategy configuration file
        #[arg(short, long)]
        config: PathBuf,
        /// Output directory for run results
        #[arg(short, long, default_value = "runs")]
        output_dir: PathBuf,
        /// Run name (auto-generated if not provided)
        #[arg(long)]
        name: Option<String>,
    },
    /// Analyze backtesting results
    Analyze {
        /// Path to run directory containing results
        run_dir: PathBuf,
        /// Generate charts and visualizations
        #[arg(long, default_value = "true")]
        charts: bool,
    },
}

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Ingest {
            input_dir,
            output_dir,
            start_date,
            end_date,
        } => {
            tracing::info!("Starting data ingestion from {:?}", input_dir);
            ingest::ingest_data(input_dir, output_dir, start_date, end_date)?;
            tracing::info!("Data ingestion completed successfully");
        }
        Commands::Run {
            dataset_dir,
            config,
            output_dir,
            name,
        } => {
            tracing::info!("Starting backtesting simulation");
            engine::run_backtest(dataset_dir, config, output_dir, name)?;
            tracing::info!("Backtesting simulation completed");
        }
        Commands::Analyze { run_dir, charts } => {
            tracing::info!("Analyzing results from {:?}", run_dir);
            analyze::analyze_results(&run_dir, charts)?;
            tracing::info!("Analysis completed");
        }
    }

    Ok(())
}
