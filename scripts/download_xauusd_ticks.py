#!/usr/bin/env python3
"""Download XAU/USD tick data from Dukascopy and store daily CSV files."""

import argparse
import csv
import lzma
import struct
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Iterator
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

INSTRUMENT = "XAUUSD"
BASE_URL = "https://datafeed.dukascopy.com/datafeed/{instrument}/{year:04d}/{month:02d}/{day:02d}/{hour:02d}h_ticks.bi5"
SCALE_FACTOR = 1000.0  # Dukascopy stores gold prices with three decimal places


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--start-date", required=True, help="Start date (YYYY-MM-DD), inclusive")
    parser.add_argument("--end-date", required=True, help="End date (YYYY-MM-DD), inclusive")
    parser.add_argument(
        "--output",
        default="data_raw",
        help="Directory where raw daily CSV files will be written",
    )
    return parser.parse_args()


def iterate_hours(start: datetime, end: datetime) -> Iterator[datetime]:
    current = start
    while current < end:
        yield current
        current += timedelta(hours=1)


def hour_url(hour: datetime) -> str:
    return BASE_URL.format(
        instrument=INSTRUMENT,
        year=hour.year,
        month=hour.month,
        day=hour.day,
        hour=hour.hour,
    )


def ensure_output_dir(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)


def download_hour(hour: datetime) -> bytes:
    url = hour_url(hour)
    try:
        with urlopen(url) as response:
            return response.read()
    except HTTPError as exc:
        if exc.code == 404:
            return b""
        raise


def parse_ticks(hour: datetime, payload: bytes) -> Iterator[tuple[int, float, float]]:
    if not payload:
        return iter(())

    raw = lzma.decompress(payload)
    if len(raw) % 20 != 0:
        return iter(())

    start_of_hour = hour.replace(minute=0, second=0, microsecond=0)
    for offset in range(0, len(raw), 20):
        ms_offset, ask_raw, bid_raw, _, _ = struct.unpack_from(">IIIII", raw, offset)
        timestamp = int(
            (start_of_hour + timedelta(milliseconds=ms_offset)).timestamp() * 1000
        )
        ask_price = ask_raw / SCALE_FACTOR
        bid_price = bid_raw / SCALE_FACTOR
        yield timestamp, ask_price, bid_price


def main() -> None:
    args = parse_args()

    start = datetime.strptime(args.start_date, "%Y-%m-%d").replace(tzinfo=timezone.utc)
    end = datetime.strptime(args.end_date, "%Y-%m-%d").replace(tzinfo=timezone.utc)
    end_inclusive = end + timedelta(days=1)

    output_dir = Path(args.output)
    ensure_output_dir(output_dir)

    total_ticks = 0
    total_days = 0

    day_start = start
    while day_start < end_inclusive:
        day_end = day_start + timedelta(days=1)
        daily_file = output_dir / f"xauusd_ticks_{day_start.date()}.csv"

        with daily_file.open("w", newline="") as handle:
            writer = csv.writer(handle)
            writer.writerow(["timestamp", "askPrice", "bidPrice"])

            day_ticks = 0
            for hour in iterate_hours(day_start, day_end):
                try:
                    payload = download_hour(hour)
                except URLError as exc:
                    raise RuntimeError(f"Failed to download {hour_url(hour)}: {exc}") from exc
                except HTTPError as exc:
                    raise RuntimeError(f"HTTP error {exc.code} for {hour_url(hour)}") from exc

                for timestamp, ask, bid in parse_ticks(hour, payload):
                    writer.writerow([timestamp, f"{ask:.3f}", f"{bid:.3f}"])
                    day_ticks += 1

        print(f"\u2714\ufe0f {daily_file.name}: {day_ticks} ticks")
        total_ticks += day_ticks
        total_days += 1
        day_start = day_end

    print(
        f"Finished downloading {total_days} days from {args.start_date} to {args.end_date}: {total_ticks} ticks"
    )


if __name__ == "__main__":
    main()
