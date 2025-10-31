#!/usr/bin/env python3
"""
Download XAU/USD tick data from Dukascopy and store daily files.

Features:
- Robust hourly downloads with retries and bounded concurrency
- Correct parsing of Dukascopy .bi5 tick records (20 bytes, big-endian)
- Millisecond UTC timestamps, ask/bid scaling with configurable factor
- Resume: skip existing daily output unless --force
- Output formats: CSV, CSV.GZ, or Parquet
- Progress and per-hour tick counts
"""

import argparse
import csv
import gzip
import lzma
import struct
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Iterator, Optional, Tuple
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

try:
    import pandas as pd  # Optional, used for Parquet output
except ImportError:
    pd = None

INSTRUMENT_DEFAULT = "XAUUSD"
BASE_URL = "https://datafeed.dukascopy.com/datafeed/{instrument}/{year:04d}/{month:02d}/{day:02d}/{hour:02d}h_ticks.bi5"
SCALE_FACTOR_DEFAULT = 1000.0  # Dukascopy gold is stored with three decimal places
RECORD_SIZE = 20
STRUCT_FMT = ">IIIII"  # msOffset, ask_raw, bid_raw, volume, ??? (unused)
MAX_MS_PER_HOUR = 3_600_000


@dataclass(frozen=True)
class Tick:
    ts_ms: int
    ask: float
    bid: float


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--start-date", required=True, help="Start date (YYYY-MM-DD), inclusive")
    parser.add_argument("--end-date", required=True, help="End date (YYYY-MM-DD), inclusive")
    parser.add_argument("--instrument", default=INSTRUMENT_DEFAULT, help="Instrument, e.g., XAUUSD")
    parser.add_argument("--scale-factor", type=float, default=SCALE_FACTOR_DEFAULT, help="Price scale factor")
    parser.add_argument("--output", default="data_raw", help="Directory for daily output files")
    parser.add_argument("--format", choices=["csv", "csv.gz", "parquet"], default="csv", help="Output format")
    parser.add_argument("--concurrency", type=int, default=8, help="Number of parallel hour downloads")
    parser.add_argument("--retries", type=int, default=3, help="Retry attempts for transient errors")
    parser.add_argument("--force", action="store_true", help="Overwrite existing daily files")
    return parser.parse_args()


def iterate_hours(start: datetime, end: datetime) -> Iterator[datetime]:
    current = start
    while current < end:
        yield current
        current += timedelta(hours=1)


def hour_url(instrument: str, hour: datetime) -> str:
    return BASE_URL.format(
        instrument=instrument,
        year=hour.year,
        month=hour.month,
        day=hour.day,
        hour=hour.hour,
    )


def ensure_output_dir(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)


def download_hour(instrument: str, hour: datetime, retries: int = 3) -> bytes:
    """
    Download a single hour .bi5, return raw bytes (may be empty if 404/no data).
    Retries on URLError and HTTP 5xx with exponential backoff.
    """
    url = hour_url(instrument, hour)
    attempt = 0
    backoff = 0.5
    while True:
        try:
            req = Request(url, headers={"User-Agent": "Mozilla/5.0 (compatible; tick-downloader/1.0)"})
            with urlopen(req, timeout=30) as response:
                return response.read()
        except HTTPError as exc:
            # 404 means hour has no data (weekend/holiday)
            if exc.code == 404:
                return b""
            # Retry on server errors
            if 500 <= exc.code < 600 and attempt < retries:
                attempt += 1
                sleep(backoff)
                backoff = min(backoff * 2, 8.0)
                continue
            raise
        except URLError as exc:
            if attempt < retries:
                attempt += 1
                sleep(backoff)
                backoff = min(backoff * 2, 8.0)
                continue
            raise


def sleep(seconds: float) -> None:
    try:
        import time
        time.sleep(seconds)
    except Exception:
        pass


def parse_ticks(hour: datetime, payload: bytes, scale_factor: float) -> Iterator[Tick]:
    """
    Parse Dukascopy hour payload into Tick objects.
    """
    if not payload:
        return iter(())

    try:
        raw = lzma.decompress(payload)
    except lzma.LZMAError:
        # Malformed hour data; skip
        return iter(())

    if len(raw) % RECORD_SIZE != 0:
        # Integrity check failed; skip this hour
        return iter(())

    start_of_hour = hour.replace(minute=0, second=0, microsecond=0)
    for offset in range(0, len(raw), RECORD_SIZE):
        ms_offset, ask_raw, bid_raw, _, _ = struct.unpack_from(STRUCT_FMT, raw, offset)

        # Validate ms offset within hour bounds
        if ms_offset >= MAX_MS_PER_HOUR:
            continue

        ts_ms = int((start_of_hour + timedelta(milliseconds=ms_offset)).timestamp() * 1000)
        ask_price = ask_raw / scale_factor
        bid_price = bid_raw / scale_factor
        yield Tick(ts_ms=ts_ms, ask=ask_price, bid=bid_price)


def write_day_csv(path: Path, ticks: Iterator[Tick]) -> int:
    """
    Write ticks to CSV at 'path'. Returns number of ticks written.
    """
    count = 0
    with path.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["timestamp", "askPrice", "bidPrice"])
        for t in ticks:
            # Prices are printed with 3 decimals to match Dukascopy scaling
            writer.writerow([t.ts_ms, f"{t.ask:.3f}", f"{t.bid:.3f}"])
            count += 1
    return count


def write_day_csv_gz(path: Path, ticks: Iterator[Tick]) -> int:
    """
    Write ticks to gzip-compressed CSV. Returns number of ticks.
    """
    count = 0
    with gzip.open(path, "wt", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["timestamp", "askPrice", "bidPrice"])
        for t in ticks:
            writer.writerow([t.ts_ms, f"{t.ask:.3f}", f"{t.bid:.3f}"])
            count += 1
    return count


def write_day_parquet(path: Path, ticks: Iterator[Tick]) -> int:
    """
    Write ticks to Parquet using pandas. Returns number of ticks.
    """
    if pd is None:
        raise RuntimeError("Parquet requires pandas. Please 'pip install pandas pyarrow'.")
    # Collect in memory per-day (acceptable; day-sized chunk)
    rows = [(t.ts_ms, t.ask, t.bid) for t in ticks]
    if not rows:
        # Create an empty file for consistency
        df = pd.DataFrame(columns=["timestamp", "askPrice", "bidPrice"])
        df.to_parquet(path, index=False)
        return 0
    df = pd.DataFrame(rows, columns=["timestamp", "askPrice", "bidPrice"])
    df.to_parquet(path, index=False)
    return len(rows)


def main() -> None:
    args = parse_args()

    # Validate dates
    try:
        start = datetime.strptime(args.start_date, "%Y-%m-%d").replace(tzinfo=timezone.utc)
        end = datetime.strptime(args.end_date, "%Y-%m-%d").replace(tzinfo=timezone.utc)
    except ValueError:
        print("Error: Dates must be in YYYY-MM-DD format.", file=sys.stderr)
        sys.exit(1)

    if start > end:
        print("Error: start-date must be <= end-date.", file=sys.stderr)
        sys.exit(1)

    end_inclusive = end + timedelta(days=1)

    output_dir = Path(args.output)
    ensure_output_dir(output_dir)

    total_ticks = 0
    total_days = 0

    day_start = start
    print(f"Downloading {args.instrument} ticks from {args.start_date} to {args.end_date} (UTC)")
    while day_start < end_inclusive:
        day_end = day_start + timedelta(days=1)

        # Determine file path according to format
        stem = f"{args.instrument.lower()}_ticks_{day_start.date()}"
        if args.format == "csv":
            daily_file = output_dir / f"{stem}.csv"
        elif args.format == "csv.gz":
            daily_file = output_dir / f"{stem}.csv.gz"
        else:
            daily_file = output_dir / f"{stem}.parquet"

        if daily_file.exists() and not args.force:
            print(f"↪ Skipping existing {daily_file.name}")
            total_days += 1
            day_start = day_end
            continue

        # Download hours in parallel
        hours = list(iterate_hours(day_start, day_end))
        hour_payloads: dict[datetime, bytes] = {}

        with ThreadPoolExecutor(max_workers=max(1, args.concurrency)) as pool:
            futures = {
                pool.submit(download_hour, args.instrument, h, args.retries): h for h in hours
            }
            for fut in as_completed(futures):
                h = futures[fut]
                try:
                    hour_payloads[h] = fut.result()
                except Exception as exc:
                    print(f"✗ Failed {hour_url(args.instrument, h)}: {exc}", file=sys.stderr)
                    hour_payloads[h] = b""

        # Parse ticks hour-by-hour in chronological order
        def day_ticks_iter() -> Iterator[Tick]:
            for h in sorted(hour_payloads.keys()):
                payload = hour_payloads[h]
                ticks_iter = parse_ticks(h, payload, args.scale_factor)
                # Optionally count ticks per hour for progress
                count = 0
                for t in ticks_iter:
                    count += 1
                    yield t
                print(f"  Hour {h.strftime('%Y-%m-%d %H:00')} → {count} ticks")

        # Atomic write: write to temp then rename
        tmp_path = daily_file.with_suffix(daily_file.suffix + ".tmp")
        try:
            if args.format == "csv":
                day_count = write_day_csv(tmp_path, day_ticks_iter())
            elif args.format == "csv.gz":
                day_count = write_day_csv_gz(tmp_path, day_ticks_iter())
            else:
                day_count = write_day_parquet(tmp_path, day_ticks_iter())

            tmp_path.replace(daily_file)
            print(f"✔ {daily_file.name}: {day_count} ticks")
        finally:
            # Cleanup temp on failure
            if tmp_path.exists() and not daily_file.exists():
                try:
                    tmp_path.unlink()
                except Exception:
                    pass

        total_ticks += day_count
        total_days += 1
        day_start = day_end

    print(f"Finished: {total_days} days from {args.start_date} to {args.end_date} → {total_ticks} ticks")


if __name__ == "__main__":
    main()
