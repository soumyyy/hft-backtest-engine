#!/usr/bin/env python3
"""
HFT Backtesting Engine - Results Analyzer
This script provides comprehensive analysis and visualization of backtesting results.
"""

import os
import json
import yaml
import glob
from datetime import datetime
import argparse

class Colors:
    """ANSI color codes for terminal output"""
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    BLUE = '\033[0;34m'
    PURPLE = '\033[0;35m'
    CYAN = '\033[0;36m'
    WHITE = '\033[1;37m'
    BOLD = '\033[1m'
    END = '\033[0m'

def print_header(text):
    """Print a formatted header"""
    print(f"\n{Colors.BLUE}{'='*60}{Colors.END}")
    print(f"{Colors.BLUE}{Colors.BOLD}{text}{Colors.END}")
    print(f"{Colors.BLUE}{'='*60}{Colors.END}")

def print_success(text):
    """Print success message"""
    print(f"{Colors.GREEN}✅ {text}{Colors.END}")

def print_warning(text):
    """Print warning message"""
    print(f"{Colors.YELLOW}⚠️  {text}{Colors.END}")

def print_error(text):
    """Print error message"""
    print(f"{Colors.RED}❌ {text}{Colors.END}")

def print_info(text):
    """Print info message"""
    print(f"{Colors.CYAN}ℹ️  {text}{Colors.END}")

def load_trades(run_dir):
    """Load trades from JSON file"""
    trades_file = os.path.join(run_dir, "trades.json")
    if not os.path.exists(trades_file):
        return None
    
    with open(trades_file, 'r') as f:
        return json.load(f)

def load_summary(run_dir):
    """Load summary from YAML file"""
    summary_file = os.path.join(run_dir, "summary.yaml")
    if not os.path.exists(summary_file):
        return None
    
    with open(summary_file, 'r') as f:
        return yaml.safe_load(f)

def analyze_trades(trades):
    """Perform detailed analysis on trades"""
    if not trades:
        return {}
    
    # Basic statistics
    total_trades = len(trades)
    total_pnl = sum(trade.get('pnl', 0) for trade in trades)
    winning_trades = sum(1 for trade in trades if trade.get('pnl', 0) > 0)
    losing_trades = sum(1 for trade in trades if trade.get('pnl', 0) < 0)
    
    # Calculate metrics
    win_rate = (winning_trades / total_trades * 100) if total_trades > 0 else 0
    avg_pnl = total_pnl / total_trades if total_trades > 0 else 0
    
    # P/L distribution
    pnls = [trade.get('pnl', 0) for trade in trades]
    max_profit = max(pnls) if pnls else 0
    max_loss = min(pnls) if pnls else 0
    
    # Price analysis
    prices = [trade.get('price', 0) for trade in trades]
    avg_price = sum(prices) / len(prices) if prices else 0
    min_price = min(prices) if prices else 0
    max_price = max(prices) if prices else 0
    
    # Spread analysis
    spreads = [trade.get('spread', 0) for trade in trades]
    avg_spread = sum(spreads) / len(spreads) if spreads else 0
    min_spread = min(spreads) if spreads else 0
    max_spread = max(spreads) if spreads else 0
    
    return {
        'total_trades': total_trades,
        'total_pnl': total_pnl,
        'winning_trades': winning_trades,
        'losing_trades': losing_trades,
        'win_rate': win_rate,
        'avg_pnl': avg_pnl,
        'max_profit': max_profit,
        'max_loss': max_loss,
        'avg_price': avg_price,
        'min_price': min_price,
        'max_price': max_price,
        'avg_spread': avg_spread,
        'min_spread': min_spread,
        'max_spread': max_spread
    }

def display_strategy_results(run_dir, strategy_name):
    """Display comprehensive results for a strategy"""
    print_header(f"📊 {strategy_name} Analysis")
    
    # Load data
    trades = load_trades(run_dir)
    summary = load_summary(run_dir)
    
    if not trades and not summary:
        print_error(f"No data found in {run_dir}")
        return
    
    # Display summary from YAML
    if summary:
        print(f"{Colors.PURPLE}📈 Summary Statistics:{Colors.END}")
        print("-" * 50)
        print(f"{'Total Trades':<20}: {summary.get('total_trades', 'N/A')}")
        print(f"{'Winning Trades':<20}: {summary.get('winning_trades', 'N/A')}")
        print(f"{'Win Rate':<20}: {summary.get('win_rate', 0) * 100:.2f}%")
        print(f"{'Total P/L':<20}: ${summary.get('total_pnl', 0):.2f}")
        print(f"{'Final Position':<20}: {summary.get('final_position_size', 'N/A')}")
        print("-" * 50)
    
    # Detailed analysis
    if trades:
        analysis = analyze_trades(trades)
        
        print(f"\n{Colors.PURPLE}📊 Detailed Analysis:{Colors.END}")
        print("-" * 50)
        print(f"{'Average P/L per Trade':<20}: ${analysis['avg_pnl']:.4f}")
        print(f"{'Best Trade':<20}: ${analysis['max_profit']:.2f}")
        print(f"{'Worst Trade':<20}: ${analysis['max_loss']:.2f}")
        print(f"{'Average Price':<20}: ${analysis['avg_price']:.2f}")
        print(f"{'Price Range':<20}: ${analysis['min_price']:.2f} - ${analysis['max_price']:.2f}")
        print(f"{'Average Spread':<20}: ${analysis['avg_spread']:.4f}")
        print(f"{'Spread Range':<20}: ${analysis['min_spread']:.4f} - ${analysis['max_spread']:.4f}")
        print("-" * 50)
        
        # Performance assessment
        if analysis['total_pnl'] > 0:
            print(f"{Colors.GREEN}💰 PROFITABLE STRATEGY!{Colors.END}")
        elif analysis['total_pnl'] < 0:
            print(f"{Colors.RED}💸 LOSS-MAKING STRATEGY{Colors.END}")
        else:
            print(f"{Colors.YELLOW}⚖️  BREAK-EVEN STRATEGY{Colors.END}")
        
        # Win rate assessment
        if analysis['win_rate'] > 60:
            print(f"{Colors.GREEN}🎯 High Win Rate: {analysis['win_rate']:.1f}%{Colors.END}")
        elif analysis['win_rate'] > 40:
            print(f"{Colors.YELLOW}📊 Moderate Win Rate: {analysis['win_rate']:.1f}%{Colors.END}")
        else:
            print(f"{Colors.RED}📉 Low Win Rate: {analysis['win_rate']:.1f}%{Colors.END}")

def list_all_results():
    """List all available results"""
    print_header("📋 All Available Results")
    
    runs_dir = "runs"
    if not os.path.exists(runs_dir):
        print_warning("No runs directory found.")
        return
    
    run_dirs = sorted(glob.glob(os.path.join(runs_dir, "*")), key=os.path.getmtime, reverse=True)
    
    if not run_dirs:
        print_warning("No results found. Run backtesting first.")
        return
    
    print(f"{Colors.CYAN}Available Run Results:{Colors.END}")
    print("-" * 80)
    print(f"{'Run Directory':<25} {'Strategy':<20} {'Trades':<10} {'P/L':<15} {'Win Rate':<10}")
    print("-" * 80)
    
    for run_dir in run_dirs:
        run_name = os.path.basename(run_dir)
        summary = load_summary(run_dir)
        
        if summary:
            strategy_name = "Unknown"
            # Try to determine strategy from config
            config_file = os.path.join(run_dir, "config.yaml")
            if os.path.exists(config_file):
                with open(config_file, 'r') as f:
                    config = yaml.safe_load(f)
                    strategy_name = config.get('name', 'Unknown')
            
            total_trades = summary.get('total_trades', 0)
            total_pnl = summary.get('total_pnl', 0)
            win_rate = summary.get('win_rate', 0) * 100
            
            print(f"{run_name:<25} {strategy_name[:19]:<20} {total_trades:<10} ${total_pnl:<14.2f} {win_rate:<9.1f}%")
        else:
            print(f"{run_name:<25} {'No Data':<20} {'N/A':<10} {'N/A':<15} {'N/A':<10}")
    
    print("-" * 80)

def main():
    parser = argparse.ArgumentParser(description='HFT Backtesting Engine - Results Analyzer')
    parser.add_argument('--run-dir', help='Specific run directory to analyze')
    parser.add_argument('--strategy', help='Strategy name for display')
    parser.add_argument('--list', action='store_true', help='List all available results')
    
    args = parser.parse_args()
    
    print_header("🎯 HFT Backtesting Engine - Results Analyzer")
    
    if args.list:
        list_all_results()
    elif args.run_dir:
        strategy_name = args.strategy or "Strategy Analysis"
        display_strategy_results(args.run_dir, strategy_name)
    else:
        # Interactive mode
        print(f"{Colors.CYAN}What would you like to do?{Colors.END}")
        print("1) Analyze specific run")
        print("2) Run a strategy")
        print("3) List all results")
        print("4) Exit")

        choice = input("\nEnter your choice (1-4): ").strip()
        
        if choice == "1":
            list_all_results()
            run_dir = input("\nEnter run directory name: ").strip()
            if run_dir:
                full_path = os.path.join("runs", run_dir)
                if os.path.exists(full_path):
                    display_strategy_results(full_path, "Strategy Analysis")
                else:
                    print_error(f"Run directory {full_path} not found!")
        elif choice == "2":
            print(f"{Colors.CYAN}Available Strategies to Run:{Colors.END}")
            print("1) Basic Scalping Strategy (jan25_scalp.yaml)")
            print("2) Basic Mean Reversion Strategy (jan25_meanrev.yaml)")
            print("3) Simple High-Frequency Strategy (jan25_simple.yaml)")
            print("4) Advanced Scalping Strategy (jan25_advanced_scalp.yaml)")
            print("5) Advanced Mean Reversion Strategy (jan25_advanced_meanrev.yaml)")
            print("6) Momentum Strategy (jan25_momentum.yaml)")
            print("7) Statistical Arbitrage Strategy (jan25_arbitrage.yaml)")
            print("8) Market Making Strategy (jan25_market_making.yaml)")
            print("9) Order Flow Detection (jan25_order_flow.yaml)")
            print("10) Profitable Momentum Strategy (jan25_profitable_momentum.yaml)")
            print("11) Pairs Trading Strategy (jan25_pairs_trading.yaml)")
            print("12) Range Trading Strategy (jan25_range_trading.yaml)")  

            strat_choice = input("\nSelect strategy to run (1-12): ").strip()
            strategies = {
                "1": ("configs/jan25_scalp.yaml", "Basic Scalping Strategy"),
                "2": ("configs/jan25_meanrev.yaml", "Basic Mean Reversion Strategy"),
                "3": ("configs/jan25_simple.yaml", "Simple High-Frequency Strategy"),
                "4": ("configs/jan25_advanced_scalp.yaml", "Advanced Scalping Strategy"),
                "5": ("configs/jan25_advanced_meanrev.yaml", "Advanced Mean Reversion Strategy"),
                "6": ("configs/jan25_momentum.yaml", "Momentum Strategy"),
                "7": ("configs/jan25_arbitrage.yaml", "Statistical Arbitrage Strategy"),
                "8": ("configs/jan25_market_making.yaml", "Market Making Strategy"),
                "9": ("configs/jan25_order_flow.yaml", "Order Flow Detection"),
                "10": ("configs/jan25_profitable_momentum.yaml", "Profitable Momentum Strategy"),
                "11": ("configs/jan25_pairs_trading.yaml", "Pairs Trading Strategy"),
                "12": ("configs/jan25_range_trading.yaml", "Range Trading Strategy")
            }

            if strat_choice in strategies:
                config_file, strategy_name = strategies[strat_choice]
                print_success(f"Running {strategy_name}...")
                os.system(f"cargo run -- run --config {config_file} --dataset-dir dataset")
                # Find the latest run and analyze it
                runs_dir = "runs"
                if os.path.exists(runs_dir):
                    run_dirs = sorted(glob.glob(os.path.join(runs_dir, "*")), key=os.path.getmtime, reverse=True)
                    if run_dirs:
                        latest_run = run_dirs[0]
                        os.system(f"cargo run -- analyze {latest_run}")
                        display_strategy_results(latest_run, strategy_name)
            else:
                print_error("Invalid strategy choice!")
        elif choice == "3":
            list_all_results()
        elif choice == "4":
            print_info("Goodbye! 👋")
        else:
            print_error("Invalid choice!")

if __name__ == "__main__":
    main()
