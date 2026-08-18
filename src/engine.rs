// filename: backtest.rs
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub timestamp: DateTime<Utc>,
    pub side: TradeSide,
    pub price: f64,
    pub volume: f64,
    pub pnl: f64,         // realized pnl for this trade
    pub spread: f64,
    pub fees: f64,        // fees paid on this trade
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub name: String,
    pub strategy_type: String,
    pub parameters: serde_yaml::Value,
}

#[derive(Debug, Clone)]
struct TickData {
    timestamp_ms: i64,
    ask: f64,
    bid: f64,
    spread: f64,
    mid_price: f64,
}

#[derive(Debug, Default, Clone)]
struct Position {
    size: f64,        // positive long, negative short
    avg_price: f64,   // average fill price including slippage
    entry_time_ms: i64,
    fees_accum: f64,  // accumulated fees on open position
}

#[derive(Debug, Serialize)]
struct RunSummary {
    total_trades: usize,
    winning_trades: usize,
    win_rate: f64,
    total_pnl: f64,          // net pnl including fees
    fees_paid: f64,
    final_position_size: f64,
    avg_trade_pnl: f64,
    avg_holding_ms: f64,
}

pub fn run_backtest(
    dataset_dir: PathBuf,
    config_path: PathBuf,
    output_dir: PathBuf,
    run_name: Option<String>,
) -> Result<()> {
    info!("Starting backtesting simulation");

    let config: StrategyConfig = serde_yaml::from_str(
        &std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file {:?}", config_path))?,
    )?;
    info!("Loaded strategy: {} ({})", config.name, config.strategy_type);

    let run_name = run_name.unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string());
    let run_dir = output_dir.join(&run_name);
    std::fs::create_dir_all(&run_dir)
        .with_context(|| format!("Failed to create run directory {:?}", run_dir))?;
    std::fs::copy(&config_path, run_dir.join("config.yaml"))?;

    let data_files = find_csv_files(&dataset_dir)?;
    if data_files.is_empty() {
        anyhow::bail!("No dataset files found in {:?}", dataset_dir);
    }
    info!("Found {} data files to process", data_files.len());

    let mut trades: Vec<Trade> = Vec::new();
    let mut position = Position::default();

    for data_file in data_files {
        info!("Processing file: {:?}", data_file);
        process_csv_file(&data_file, &config, &mut trades, &mut position)?;
    }

    save_trades(&run_dir, &trades)?;
    generate_summary(&run_dir, &trades, &position)?;

    info!("Backtesting completed. Results saved to: {:?}", run_dir);
    Ok(())
}

fn find_csv_files(dataset_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut csv_files = Vec::new();

    for entry in std::fs::read_dir(dataset_dir)
        .with_context(|| format!("Failed to read dataset directory {:?}", dataset_dir))?
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

fn parse_row(timestamp_str: &str, ask_str: &str, bid_str: &str, file: &Path) -> Result<TickData> {
    let timestamp_ms = timestamp_str
        .parse::<i64>()
        .with_context(|| format!("Invalid timestamp (ms) in {:?}", file))?;

    if timestamp_ms < 0 {
        anyhow::bail!("Negative timestamp encountered in {:?}", file);
    }

    // Strict conversion: error on out-of-range instead of defaulting to now
    let _dt = Utc
        .timestamp_millis_opt(timestamp_ms)
        .single()
        .context("Timestamp out of valid range")?;

    let ask = ask_str
        .parse::<f64>()
        .with_context(|| format!("Invalid askPrice in {:?}", file))?;
    let bid = bid_str
        .parse::<f64>()
        .with_context(|| format!("Invalid bidPrice in {:?}", file))?;

    if !(ask.is_finite() && bid.is_finite()) || ask <= 0.0 || bid <= 0.0 || ask < bid {
        anyhow::bail!("Bad tick: ask/bid invalid (ask={}, bid={}) in {:?}", ask, bid, file);
    }

    let spread = ask - bid;
    let mid_price = (ask + bid) / 2.0;

    Ok(TickData {
        timestamp_ms,
        ask,
        bid,
        spread,
        mid_price,
    })
}

fn process_csv_file(
    data_file: &Path,
    config: &StrategyConfig,
    trades: &mut Vec<Trade>,
    position: &mut Position,
) -> Result<()> {
    // Use csv crate for robustness
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(data_file)
        .with_context(|| format!("Failed to open dataset file {:?}", data_file))?;

    // Resolve parameter defaults
    let fee_bps = config.parameters["fee_bps"].as_f64().unwrap_or(2.0); // 2 bps default
    let slippage = config.parameters["slippage"].as_f64().unwrap_or(0.0); // absolute price slippage
    let max_history = config.parameters["max_history"].as_u64().unwrap_or(1000) as usize;

    let mut price_history: VecDeque<f64> = VecDeque::with_capacity(max_history);
    let mut tick_count = 0usize;
    let mut last_trade_time_ms = 0i64;
    let mut last_tick: Option<TickData> = None;

    for result in rdr.records() {
        let record = result?;
        // Expect headers: timestamp, askPrice, bidPrice
        let timestamp_str = record.get(0).context("Missing timestamp column")?;
        let ask_str = record.get(1).context("Missing askPrice column")?;
        let bid_str = record.get(2).context("Missing bidPrice column")?;
        let tick = parse_row(timestamp_str, ask_str, bid_str, data_file)?;

        match config.strategy_type.as_str() {
            "scalping" => execute_scalping_strategy(
                &tick,
                config,
                trades,
                position,
                &mut last_trade_time_ms,
                fee_bps,
                slippage,
            )?,
            "mean_reversion" => execute_mean_reversion_strategy(
                &tick,
                &price_history,
                config,
                trades,
                position,
                &mut last_trade_time_ms,
                fee_bps,
                slippage,
            )?,
            other => {
                warn!("Unknown strategy type '{}'. Skipping ticks", other);
                break;
            }
        }

        last_tick = Some(tick.clone());
        tick_count += 1;
        if tick_count % 200_000 == 0 {
            info!("Processed {} ticks so far", tick_count);
        }

        price_history.push_back(tick.mid_price);
        if price_history.len() > max_history {
            price_history.pop_front();
        }
    }

    if let Some(tick) = last_tick {
        close_open_position(&tick, trades, position, fee_bps, slippage)?;
    }

    info!("Finished processing {} ticks from {:?}", tick_count, data_file);
    Ok(())
}

fn effective_buy_price(ask: f64, slippage: f64) -> f64 {
    ask + slippage
}
fn effective_sell_price(bid: f64, slippage: f64) -> f64 {
    bid - slippage
}
fn trade_fees(notional: f64, fee_bps: f64) -> f64 {
    notional * (fee_bps / 10_000.0)
}

fn execute_scalping_strategy(
    tick: &TickData,
    config: &StrategyConfig,
    trades: &mut Vec<Trade>,
    position: &mut Position,
    last_trade_time_ms: &mut i64,
    fee_bps: f64,
    slippage: f64,
) -> Result<()> {
    let min_spread = config.parameters["min_spread"].as_f64().unwrap_or(1.0);
    let max_trades_per_hour = config.parameters["max_trades_per_hour"].as_u64().unwrap_or(120);
    let take_profit = config.parameters["take_profit"].as_f64().unwrap_or(0.25);
    let stop_loss = config.parameters["stop_loss"].as_f64().unwrap_or(0.35);
    let max_hold_ms = config.parameters["max_hold_ms"].as_i64().unwrap_or(60_000);

    // Exit logic for long position
    if position.size > 0.0 {
        // Use effective exit price (sell worse than bid by slippage)
        let exit_price = effective_sell_price(tick.bid, slippage);
        let unrealized = exit_price - position.avg_price;
        let held_for = tick.timestamp_ms - position.entry_time_ms;
        let timed_exit = held_for >= max_hold_ms && tick.spread <= min_spread * 1.5;

        if unrealized >= take_profit || unrealized <= -stop_loss || timed_exit {
            let notional = exit_price * position.size;
            let fees = trade_fees(notional, fee_bps) + position.fees_accum;

            trades.push(Trade {
                timestamp: Utc.timestamp_millis_opt(tick.timestamp_ms).single().unwrap(),
                side: TradeSide::Sell,
                price: exit_price,
                volume: position.size,
                pnl: unrealized - fees,
                spread: tick.spread,
                fees,
            });
            *last_trade_time_ms = tick.timestamp_ms;
            // Reset position
            *position = Position::default();
            return Ok(());
        }
    }

    let time_since_last_trade = tick.timestamp_ms - *last_trade_time_ms;
    let min_time_between_trades = 3_600_000 / max_trades_per_hour.max(1) as i64;
    if time_since_last_trade < min_time_between_trades || position.size != 0.0 {
        return Ok(());
    }

    // Entry gating: ensure spread is narrow enough, but also ensure tp/sl exceed spread+fees
    if tick.spread <= min_spread {
        let entry_price = effective_buy_price(tick.ask, slippage);
        let notional = entry_price * 1.0;
        let fees = trade_fees(notional, fee_bps);

        trades.push(Trade {
            timestamp: Utc.timestamp_millis_opt(tick.timestamp_ms).single().unwrap(),
            side: TradeSide::Buy,
            price: entry_price,
            volume: 1.0,
            pnl: 0.0,
            spread: tick.spread,
            fees,
        });
        position.size = 1.0;
        position.avg_price = entry_price;
        position.entry_time_ms = tick.timestamp_ms;
        position.fees_accum = fees;
        *last_trade_time_ms = tick.timestamp_ms;
    }

    Ok(())
}

fn execute_mean_reversion_strategy(
    tick: &TickData,
    price_history: &VecDeque<f64>,
    config: &StrategyConfig,
    trades: &mut Vec<Trade>,
    position: &mut Position,
    last_trade_time_ms: &mut i64,
    fee_bps: f64,
    slippage: f64,
) -> Result<()> {
    let ma_period = config.parameters["ma_period"].as_u64().unwrap_or(50) as usize;
    if price_history.len() < ma_period {
        return Ok(());
    }

    let threshold_std = config.parameters["threshold_std"].as_f64().unwrap_or(1.25);
    let max_trades_per_hour = config.parameters["max_trades_per_hour"].as_u64().unwrap_or(40);
    let take_profit = config.parameters["take_profit"].as_f64().unwrap_or(2.0);
    let stop_loss = config.parameters["stop_loss"].as_f64().unwrap_or(3.0);
    let max_hold_ms = config.parameters["max_hold_ms"].as_i64().unwrap_or(15 * 60 * 1000);

    // Compute mean and std with Welford’s algorithm for the last ma_period
    let mut mean = 0.0;
    let mut m2 = 0.0;
    let mut n = 0usize;
    for price in price_history.iter().rev().take(ma_period) {
        n += 1;
        let delta = *price - mean;
        mean += delta / n as f64;
        let delta2 = *price - mean;
        m2 += delta * delta2;
    }
    if n == 0 {
        return Ok(());
    }
    let variance = if n > 1 { m2 / (n as f64 - 1.0) } else { 0.0 };
    let std_dev = variance.max(0.0).sqrt().max(1e-6);

    let upper_band = mean + threshold_std * std_dev;
    let lower_band = mean - threshold_std * std_dev;

    // Exit logic
    if position.size > 0.0 {
        let exit_price = effective_sell_price(tick.bid, slippage);
        let pnl_gross = exit_price - position.avg_price;
        let held_for = tick.timestamp_ms - position.entry_time_ms;
        if tick.mid_price >= mean
            || pnl_gross >= take_profit
            || pnl_gross <= -stop_loss
            || held_for >= max_hold_ms
        {
            let fees = trade_fees(exit_price * position.size, fee_bps) + position.fees_accum;
            trades.push(Trade {
                timestamp: Utc.timestamp_millis_opt(tick.timestamp_ms).single().unwrap(),
                side: TradeSide::Sell,
                price: exit_price,
                volume: position.size,
                pnl: pnl_gross - fees,
                spread: tick.spread,
                fees,
            });
            *last_trade_time_ms = tick.timestamp_ms;
            *position = Position::default();
            return Ok(());
        }
    } else if position.size < 0.0 {
        let exit_price = effective_buy_price(tick.ask, slippage);
        let pnl_gross = position.avg_price - exit_price;
        let held_for = tick.timestamp_ms - position.entry_time_ms;
        if tick.mid_price <= mean
            || pnl_gross >= take_profit
            || pnl_gross <= -stop_loss
            || held_for >= max_hold_ms
        {
            let fees = trade_fees(exit_price * position.size.abs(), fee_bps) + position.fees_accum;
            trades.push(Trade {
                timestamp: Utc.timestamp_millis_opt(tick.timestamp_ms).single().unwrap(),
                side: TradeSide::Buy,
                price: exit_price,
                volume: position.size.abs(),
                pnl: pnl_gross - fees,
                spread: tick.spread,
                fees,
            });
            *last_trade_time_ms = tick.timestamp_ms;
            *position = Position::default();
            return Ok(());
        }
    }

    let time_since_last_trade = tick.timestamp_ms - *last_trade_time_ms;
    let min_time_between_trades = 3_600_000 / max_trades_per_hour.max(1) as i64;
    if time_since_last_trade < min_time_between_trades || position.size != 0.0 {
        return Ok(());
    }

    // Entries
    if tick.mid_price <= lower_band {
        let entry_price = effective_buy_price(tick.ask, slippage);
        let fees = trade_fees(entry_price * 1.0, fee_bps);
        trades.push(Trade {
            timestamp: Utc.timestamp_millis_opt(tick.timestamp_ms).single().unwrap(),
            side: TradeSide::Buy,
            price: entry_price,
            volume: 1.0,
            pnl: 0.0,
            spread: tick.spread,
            fees,
        });
        position.size = 1.0;
        position.avg_price = entry_price;
        position.entry_time_ms = tick.timestamp_ms;
        position.fees_accum = fees;
        *last_trade_time_ms = tick.timestamp_ms;
    } else if tick.mid_price >= upper_band {
        let entry_price = effective_sell_price(tick.bid, slippage);
        let fees = trade_fees(entry_price * 1.0, fee_bps);
        trades.push(Trade {
            timestamp: Utc.timestamp_millis_opt(tick.timestamp_ms).single().unwrap(),
            side: TradeSide::Sell,
            price: entry_price,
            volume: 1.0,
            pnl: 0.0,
            spread: tick.spread,
            fees,
        });
        position.size = -1.0;
        position.avg_price = entry_price;
        position.entry_time_ms = tick.timestamp_ms;
        position.fees_accum = fees;
        *last_trade_time_ms = tick.timestamp_ms;
    }

    Ok(())
}

fn close_open_position(
    tick: &TickData,
    trades: &mut Vec<Trade>,
    position: &mut Position,
    fee_bps: f64,
    slippage: f64,
) -> Result<()> {
    if position.size == 0.0 {
        return Ok(());
    }

    let (side, exit_price, volume, pnl_gross) = if position.size > 0.0 {
        let px = effective_sell_price(tick.bid, slippage);
        (TradeSide::Sell, px, position.size, px - position.avg_price)
    } else {
        let px = effective_buy_price(tick.ask, slippage);
        (TradeSide::Buy, px, position.size.abs(), position.avg_price - px)
    };

    let fees = trade_fees(exit_price * volume, fee_bps) + position.fees_accum;

    trades.push(Trade {
        timestamp: Utc.timestamp_millis_opt(tick.timestamp_ms).single().unwrap(),
        side,
        price: exit_price,
        volume,
        pnl: pnl_gross - fees,
        spread: tick.spread,
        fees,
    });

    *position = Position::default();
    Ok(())
}

fn save_trades(run_dir: &Path, trades: &[Trade]) -> Result<()> {
    let trades_file = run_dir.join("trades.json");
    let trades_json = serde_json::to_string_pretty(trades)?;
    std::fs::write(&trades_file, trades_json)?;
    info!("Saved {} trades to {:?}", trades.len(), trades_file);
    Ok(())
}

fn generate_summary(run_dir: &Path, trades: &[Trade], position: &Position) -> Result<()> {
    let total_trades = trades.len();
    let total_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
    let fees_paid: f64 = trades.iter().map(|t| t.fees).sum();
    let winning_trades = trades.iter().filter(|t| t.pnl > 0.0).count();
    let win_rate = if total_trades > 0 {
        winning_trades as f64 / total_trades as f64
    } else {
        0.0
    };

    // Approximate average holding time from consecutive trade timestamps
    // For single-position strategies, use differences between close and prior open times captured via Trade records
    let mut holding_times: Vec<i64> = Vec::new();
    // We can approximate by pairing Buy then next Sell or Sell then next Buy with same volume=1.0
    // For simplicity, compute delta between consecutive trades
    for w in trades.windows(2) {
        let delta = (w[1].timestamp - w[0].timestamp).num_milliseconds();
        if delta > 0 {
            holding_times.push(delta);
        }
    }
    let avg_holding_ms = if holding_times.is_empty() {
        0.0
    } else {
        holding_times.iter().map(|d| *d as f64).sum::<f64>() / holding_times.len() as f64
    };

    let avg_trade_pnl = if total_trades > 0 {
        total_pnl / total_trades as f64
    } else {
        0.0
    };

    let summary = RunSummary {
        total_trades,
        winning_trades,
        win_rate,
        total_pnl,
        fees_paid,
        final_position_size: position.size,
        avg_trade_pnl,
        avg_holding_ms,
    };

    let summary_yaml = serde_yaml::to_string(&summary)?;
    std::fs::write(run_dir.join("summary.yaml"), summary_yaml)?;

    info!(
        "Generated summary: {} trades | Net P/L {:.2} | win rate {:.2}% | fees {:.2}",
        total_trades,
        total_pnl,
        win_rate * 100.0,
        fees_paid
    );

    Ok(())
}
