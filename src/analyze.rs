use anyhow::Result;
use std::path::Path;
use tracing::info;

pub fn analyze_results(run_dir: &std::path::PathBuf, generate_charts: bool) -> Result<()> {
    info!("Starting analysis of results in {:?}", run_dir);
    
    // Load trades data
    let trades_file = run_dir.join("trades.json");
    if !trades_file.exists() {
        return Err(anyhow::anyhow!("No trades.json found in run directory"));
    }
    
    let trades_json = std::fs::read_to_string(&trades_file)?;
    let trades: Vec<serde_json::Value> = serde_json::from_str(&trades_json)?;
    
    if trades.is_empty() {
        info!("No trades found in the dataset");
        return Ok(());
    }
    
    info!("Loaded {} trades for analysis", trades.len());
    
    // Generate summary statistics
    generate_summary_stats(&trades, run_dir)?;
    
    // Generate charts if requested
    if generate_charts {
        info!("Chart generation requested but not implemented yet");
    }
    
    info!("Analysis completed successfully");
    Ok(())
}

fn generate_summary_stats(trades: &[serde_json::Value], run_dir: &Path) -> Result<()> {
    let total_trades = trades.len();
    
    // Calculate P/L statistics
    let mut total_pnl = 0.0;
    let mut winning_trades = 0;
    
    for trade in trades {
        if let Some(pnl) = trade["pnl"].as_f64() {
            total_pnl += pnl;
            if pnl > 0.0 {
                winning_trades += 1;
            }
        }
    }
    
    let win_rate = if total_trades > 0 {
        winning_trades as f64 / total_trades as f64
    } else {
        0.0
    };
    
    // Create summary report
    let summary = format!(
        r#"# Backtesting Results Summary

## Trade Statistics
- **Total Trades**: {}
- **Winning Trades**: {}
- **Win Rate**: {:.2}%

## P/L Analysis
- **Total P/L**: {:.4}

## Performance Metrics
- **Average P/L per Trade**: {:.4}
"#,
        total_trades,
        winning_trades,
        win_rate * 100.0,
        total_pnl,
        total_pnl / total_trades as f64,
    );
    
    std::fs::write(run_dir.join("analysis_report.md"), summary)?;
    info!("Generated summary statistics report");
    
    Ok(())
}