#!/usr/bin/env python3
"""katpool canary credit-watcher (ADR-0004 end-to-end probe).

Runs anywhere with Python 3.9+ (macOS / Linux), no third-party packages. It is
the *watcher* half of the canary: a small CPU miner (you run it separately —
see README.md) submits real shares to the pool from OUTSIDE the VPS using the
canary wallet; this script polls the pool's public API for that wallet's
`last_seen_at` (which only advances when the accountant actually credits a
share) and publishes it as `canary_last_credited_timestamp_seconds` to
VictoriaMetrics through vmauth. The `CanaryMinerNotPaid` alert fires when
`now - max(canary_last_credited_timestamp_seconds) > 1h`, i.e. the real
accept -> credit path has stalled even if internal metrics look healthy.

Config is via environment (all required unless noted):

  KATPOOL_CANARY_API_BASE      e.g. https://api-tn10.katpool.com  (no trailing /)
  KATPOOL_CANARY_ADDRESS       the canary wallet (kaspatest:... / kaspa:...)
  KATPOOL_CANARY_VMAUTH_URL    e.g. https://vmauth-stage-testnet-10.up.railway.app
  KATPOOL_CANARY_VMAUTH_USER   vmauth write user (same identity as the vmagent)
  KATPOOL_CANARY_VMAUTH_PASSWORD
  KATPOOL_CANARY_NETWORK       metric label, e.g. testnet-10  (default: unknown)
  KATPOOL_CANARY_INTERVAL_SECS poll interval seconds          (default: 60)

Exit codes: only fatal on bad config; transient HTTP errors are logged and
retried so a flaky network never kills the probe.
"""

import base64
import json
import os
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

METRIC = "canary_last_credited_timestamp_seconds"


def env(name: str, default: str | None = None) -> str:
    val = os.environ.get(name, default)
    if val is None or val == "":
        sys.exit(f"canary: required env {name} is not set")
    return val


def log(level: str, msg: str) -> None:
    ts = datetime.now(timezone.utc).isoformat()
    print(f"{ts} {level} {msg}", flush=True)


def parse_ts(value: str) -> float:
    """RFC3339 / ISO-8601 -> unix seconds (handles a trailing 'Z')."""
    return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()


def fetch_last_credited(api_base: str, address: str, timeout: float) -> float | None:
    """Return the canary wallet's `last_seen_at` as unix seconds, or None."""
    url = f"{api_base}/api/v1/miners/{urllib.parse.quote(address, safe='')}"
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        body = json.load(resp)
    seen = body.get("last_seen_at")
    return parse_ts(seen) if seen else None


def push_metric(vmauth_url: str, user: str, password: str, network: str, value: float, timeout: float) -> None:
    """Publish the gauge to VictoriaMetrics via the vmauth import path."""
    line = f'{METRIC}{{network="{network}",instance="canary"}} {value:.0f}\n'
    url = f"{vmauth_url}/api/v1/import/prometheus"
    auth = base64.b64encode(f"{user}:{password}".encode()).decode()
    req = urllib.request.Request(
        url,
        data=line.encode(),
        method="POST",
        headers={"Authorization": f"Basic {auth}", "Content-Type": "text/plain"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        if resp.status not in (200, 204):
            raise RuntimeError(f"vmauth import returned HTTP {resp.status}")


def main() -> None:
    api_base = env("KATPOOL_CANARY_API_BASE").rstrip("/")
    address = env("KATPOOL_CANARY_ADDRESS")
    vmauth_url = env("KATPOOL_CANARY_VMAUTH_URL").rstrip("/")
    user = env("KATPOOL_CANARY_VMAUTH_USER")
    password = env("KATPOOL_CANARY_VMAUTH_PASSWORD")
    network = os.environ.get("KATPOOL_CANARY_NETWORK", "unknown")
    interval = float(os.environ.get("KATPOOL_CANARY_INTERVAL_SECS", "60"))
    timeout = 10.0

    log("INFO", f"canary watcher started: api={api_base} network={network} interval={interval:.0f}s")
    while True:
        try:
            last = fetch_last_credited(api_base, address, timeout)
            if last is None:
                log("WARN", "canary wallet has no last_seen_at yet (miner running + crediting?)")
            else:
                push_metric(vmauth_url, user, password, network, last, timeout)
                age = time.time() - last
                log("INFO", f"published {METRIC}={last:.0f} (credited {age:.0f}s ago)")
        except (urllib.error.URLError, OSError, ValueError, RuntimeError) as e:
            log("ERROR", f"cycle failed (will retry): {e}")
        time.sleep(interval)


if __name__ == "__main__":
    main()
