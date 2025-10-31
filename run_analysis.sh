#!/bin/bash

# HFT Backtesting Engine - Complete Analysis Script
# This script runs backtesting, analyzes results, and displays comprehensive reports

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

declare -a CONFIG_FILES=()
declare -a STRATEGY_NAMES=()

# Function to print colored output
print_header() {
    echo -e "${BLUE}================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}================================================${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${CYAN}ℹ️  $1${NC}"
}

# Function to display results in a nice table
display_results() {
    local run_dir=$1
    local strategy_name=$2
    
    print_header "📊 $strategy_name Results"
    
    if [ -f "$run_dir/summary.yaml" ]; then
        echo -e "${PURPLE}Summary Statistics:${NC}"
        echo "----------------------------------------"
        
        # Extract key metrics from YAML
        total_trades=$(grep "total_trades:" "$run_dir/summary.yaml" | awk '{print $2}')
        total_pnl=$(grep "total_pnl:" "$run_dir/summary.yaml" | awk '{print $2}')
        win_rate=$(grep "win_rate:" "$run_dir/summary.yaml" | awk '{print $2}')
        winning_trades=$(grep "winning_trades:" "$run_dir/summary.yaml" | awk '{print $2}')
        
        printf "%-20s: %s\n" "Total Trades" "$total_trades"
        printf "%-20s: %s\n" "Winning Trades" "$winning_trades"
        printf "%-20s: %.2f%%\n" "Win Rate" "$(echo "$win_rate * 100" | bc -l)"
        printf "%-20s: $%.2f\n" "Total P/L" "$total_pnl"
        
        # Calculate average P/L per trade
        if [ "$total_trades" -gt 0 ]; then
            avg_pnl=$(echo "scale=4; $total_pnl / $total_trades" | bc -l)
            printf "%-20s: $%.4f\n" "Avg P/L per Trade" "$avg_pnl"
        fi
        
        echo "----------------------------------------"
        
        # Color code the P/L
        if (( $(echo "$total_pnl > 0" | bc -l) )); then
            echo -e "${GREEN}💰 PROFITABLE STRATEGY!${NC}"
        elif (( $(echo "$total_pnl < 0" | bc -l) )); then
            echo -e "${RED}💸 LOSS-MAKING STRATEGY${NC}"
        else
            echo -e "${YELLOW}⚖️  BREAK-EVEN STRATEGY${NC}"
        fi
    fi
    
    if [ -f "$run_dir/analysis_report.md" ]; then
        echo ""
        echo -e "${PURPLE}Detailed Analysis Report:${NC}"
        echo "----------------------------------------"
        cat "$run_dir/analysis_report.md"
    fi
    
    echo ""
}

load_strategies() {
    CONFIG_FILES=()
    STRATEGY_NAMES=()

    if compgen -G "configs/*.yaml" > /dev/null; then
        while IFS= read -r config_file; do
            name=$(grep -m1 "^name:" "$config_file" | cut -d':' -f2- | sed 's/^ *//; s/^"//; s/"$//')
            if [ -z "$name" ]; then
                name=$(basename "$config_file")
            fi
            CONFIG_FILES+=("$config_file")
            STRATEGY_NAMES+=("$name")
        done < <(ls configs/*.yaml 2>/dev/null | sort)
    fi
}

# Function to show file sizes and processing info
show_processing_info() {
    print_header "📈 Processing Information"
    
    if [ -d "dataset" ]; then
        echo -e "${CYAN}Dataset Information:${NC}"
        for file in dataset/*.csv; do
            if [ -f "$file" ]; then
                size=$(du -h "$file" | cut -f1)
                lines=$(wc -l < "$file")
                echo "  📁 $(basename "$file"): $size ($lines lines)"
            fi
        done
    fi
    
    echo ""
    echo -e "${CYAN}Available Strategies:${NC}"
    if [ "${#CONFIG_FILES[@]}" -eq 0 ]; then
        print_warning "No strategy configurations found in configs/ directory!"
    else
        for idx in "${!CONFIG_FILES[@]}"; do
            echo "  ⚙️  $(basename "${CONFIG_FILES[idx]}"): ${STRATEGY_NAMES[idx]}"
        done
    fi
    
    echo ""
}

# Function to run backtesting for a strategy
run_strategy() {
    local config_file=$1
    local strategy_name=$2
    
    print_header "🚀 Running $strategy_name"
    
    # Check if dataset exists
    if [ ! -d "dataset" ] || [ -z "$(ls -A dataset/*.csv 2>/dev/null)" ]; then
        print_error "No dataset found! Please run data ingestion first."
        return 1
    fi
    
    # Run backtesting
    print_info "Starting backtesting simulation..."
    if cargo run -- run --config "$config_file" --dataset-dir dataset; then
        print_success "Backtesting completed successfully!"
        
        # Find the latest run directory
        latest_run=$(ls -t runs/ | head -n1)
        if [ -n "$latest_run" ]; then
            run_dir="runs/$latest_run"
            print_info "Results saved to: $run_dir"
            
            # Analyze results
            print_info "Analyzing results..."
            if cargo run -- analyze "$run_dir"; then
                print_success "Analysis completed!"
                display_results "$run_dir" "$strategy_name"
            else
                print_error "Analysis failed!"
                return 1
            fi
        else
            print_error "No run directory found!"
            return 1
        fi
    else
        print_error "Backtesting failed!"
        return 1
    fi
}

# Function to show all available results
show_all_results() {
    print_header "📋 All Available Results"
    
    if [ ! -d "runs" ] || [ -z "$(ls -A runs/ 2>/dev/null)" ]; then
        print_warning "No results found. Run backtesting first."
        return
    fi
    
    echo -e "${CYAN}Available Run Results:${NC}"
    echo "----------------------------------------"
    
    for run_dir in runs/*/; do
        if [ -d "$run_dir" ]; then
            run_name=$(basename "$run_dir")
            timestamp=$(echo "$run_name" | cut -d'_' -f1,2)
            
            if [ -f "$run_dir/summary.yaml" ]; then
                total_trades=$(grep "total_trades:" "$run_dir/summary.yaml" | awk '{print $2}')
                total_pnl=$(grep "total_pnl:" "$run_dir/summary.yaml" | awk '{print $2}')
                
                printf "📁 %-20s: %s trades, P/L: $%.2f\n" "$run_name" "$total_trades" "$total_pnl"
            else
                printf "📁 %-20s: (No summary available)\n" "$run_name"
            fi
        fi
    done
    
    echo "----------------------------------------"
    echo ""
}

# Main script
main() {
    print_header "🎯 HFT Backtesting Engine - Complete Analysis"
    echo -e "${YELLOW}This script will run backtesting, analyze results, and display comprehensive reports.${NC}"
    echo ""
    
    load_strategies

    if [ "${#CONFIG_FILES[@]}" -eq 0 ]; then
        print_error "No strategy configurations found in configs/ directory!"
        exit 1
    fi

    # Show processing info
    show_processing_info
    
    # Ask user what to do
    echo -e "${CYAN}Available Trading Strategies:${NC}"
    for idx in "${!STRATEGY_NAMES[@]}"; do
        printf "%d) %s\n" "$((idx + 1))" "${STRATEGY_NAMES[idx]}"
    done

    run_all_choice=$(( ${#STRATEGY_NAMES[@]} + 1 ))
    show_results_choice=$(( run_all_choice + 1 ))
    exit_choice=$(( show_results_choice + 1 ))

    echo "${run_all_choice}) 🔄 Run All Strategies"
    echo "${show_results_choice}) 📋 Show All Previous Results"
    echo "${exit_choice}) 🚪 Exit"
    echo ""
    read -p "Enter your choice (1-${exit_choice}): " choice

    if ! [[ "$choice" =~ ^[0-9]+$ ]]; then
        print_error "Invalid choice. Please run the script again."
        exit 1
    fi

    choice_num=$choice

    if (( choice_num >= 1 && choice_num <= ${#STRATEGY_NAMES[@]} )); then
        idx=$((choice_num - 1))
        run_strategy "${CONFIG_FILES[idx]}" "${STRATEGY_NAMES[idx]}"
    elif (( choice_num == run_all_choice )); then
        print_header "🔄 Running All Strategies"
        for idx in "${!CONFIG_FILES[@]}"; do
            run_strategy "${CONFIG_FILES[idx]}" "${STRATEGY_NAMES[idx]}"
            echo ""
        done
    elif (( choice_num == show_results_choice )); then
        show_all_results
    elif (( choice_num == exit_choice )); then
        print_info "Goodbye! 👋"
        exit 0
    else
        print_error "Invalid choice. Please run the script again."
        exit 1
    fi
    
    print_header "🎉 Analysis Complete!"
    print_info "All results are saved in the runs/ directory."
    print_info "You can view detailed reports in each run's folder."
}

# Run main function
main "$@"
