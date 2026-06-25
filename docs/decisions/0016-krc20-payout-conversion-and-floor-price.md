---
status: accepted
date: 2026-05-31
deciders: argonmining
---

# ADR-0016: KAS→NACHO payout conversion, floor-price quote, and the (non-)multiplier

## Context and Problem Statement

ADR-0012 fixed how NACHO rebate *accrues*: per matured block, tier-aware
(`STANDARD_REBATE_BPS = 3300`, `ELITE_REBATE_BPS = 10000`), stored in
**KAS-sompi** in `nacho_rebate_accrual.accrued_sompi`. It explicitly
deferred "the actual KAS→NACHO conversion at payout time" to "the Phase 5
ADR". This is that ADR.

Two questions must be answered deterministically before any NACHO leaves
the treasury, because both move money:

1. **Is a full-rebate multiplier re-applied at payout?** The legacy pool
   (`transferKrc20Tokens.ts`) multiplied the rebate by `3n` at payout for
   "full rebate" wallets (≥100M NACHO or a KATCLAIM NFT) via
   `checkFullFeeRebate`, which had a truthiness bug. `docs/architecture.md`
   §4.4 carries that "3× at payout" language forward.
2. **How is the KAS-sompi pending balance converted to NACHO base units?**

## Decision Drivers

* Correctness is money-critical and must be integer-deterministic
  (ADR-0013 verification posture forbids float in value math).
* Resolve the legacy-vs-new conflict from evidence, not prose.
* A degraded external price API must never cause an over- or under-payment.

## Decision

### 1. No multiplier is re-applied at payout

The tier multiplier is **already baked into `accrued_sompi`** at allocation
time: `accountant::allocation` computes `nacho_accrual_sompi =
fee_share × applied_rebate_bps / 10_000` (3300 standard / 10000 elite,
ADR-0012 §3, §5) and accrues it via `nacho_rebate::accrue`. The repo doc is
explicit: `accrued_sompi` is "cumulative sompi accrued via the … NACHO
accrual rule". Re-multiplying at payout would pay elite wallets their rebate
**twice over**.

Therefore the payout engine converts `pending_sompi = accrued_sompi −
paid_sompi` straight to NACHO with **no multiplier**. The legacy `×3` and
`docs/architecture.md` §4.4's "3× multiplier … at payout" are legacy
carryover and are **superseded by this ADR**. The elite tier still gets its
full rebate — it was applied per block, in sompi, where it belongs and is
auditable (`share_allocation.applied_tier` / `applied_rebate_bps`).

### 2. Eligibility is the pending-sompi threshold (already in the repo)

Recipient selection is `nacho_rebate::list_pending(min_pending_sompi,
limit)`: wallets with `accrued_sompi − paid_sompi ≥ min_pending_sompi`,
highest first. The threshold is operator-tunable; default
`DEFAULT_MIN_PENDING_SOMPI` lives in `payout-krc20`. This mirrors the KAS
payout eligibility shape from Phase 4.

### 3. Conversion is exact integer math

The floor price is a decimal **KAS-per-NACHO-token** price (e.g. `0.000330`;
see §5 for how it is sourced). Because **both KAS-sompi and NACHO base units
carry 8 decimal places**, the scale factors cancel and:

```text
nacho_base_units = floor(pending_sompi / floor_price_kas_per_nacho)
```

To keep this exact (the legacy did lossy `f64` division), the floor price is
parsed from its decimal string into a fixed-point rational
`value = mantissa / 10^scale` (e.g. `"0.000365" → mantissa=365, scale=6`),
and the division is integer:

```text
nacho_base_units = floor(pending_sompi × 10^scale / mantissa)   // u128
```

`u128` headroom prevents overflow (`pending_sompi ≤ i64::MAX`, `10^scale`
bounded by a max accepted scale). Scientific notation, negative, and
zero-mantissa prices are rejected.

### 4. Dust gate after conversion

A reveal transaction costs a fixed fee; paying out a NACHO amount worth less
than the reveal is value-destructive. Post-conversion amounts below
`DEFAULT_MIN_NACHO_BASE_UNITS` are skipped (the pending balance stays
accrued for a later cycle). This is the new-pool analogue of the legacy
`nachoThresholdAmount` skip, but applied to a deterministic integer amount.

### 5. Floor price: CoinGecko market ratio, direct HTTPS behind a trait

> **Amendment (2026-06-13):** the price source moved from the Kasplex
> marketplace floor (`api.kaspa.com/api/floor-price`) to the **CoinGecko market
> ratio**. The original endpoint is a single-marketplace *floor listing*;
> CoinGecko is a volume-weighted market price with broader coverage and a
> documented public API. The conversion contract (§3) is unchanged — only how
> the KAS-per-NACHO number is obtained.

Source: a single keyless HTTPS GET
`https://api.coingecko.com/api/v3/simple/price?ids=nacho-the-kat,kaspa&vs_currencies=usd&precision=18`,
returning `{"kaspa":{"usd":<P_kas>},"nacho-the-kat":{"usd":<P_nacho>}}`.
CoinGecko prices in **USD**, but the conversion needs **KAS per NACHO**, so the
engine derives it from both legs in one call:

```text
floor_price_kas_per_nacho = P_nacho_usd / P_kas_usd
```

The USD scale cancels in the ratio. The division is **exact and float-free**
(ADR-0013): both quotes are read as their verbatim JSON number text (serde_json
`raw_value`, since a plain `Number` round-trips through `f64` without
`arbitrary_precision`), divided with `bigdecimal`, then **floored** (never
rounded up — a payout must never be over-funded) to a fixed 18-digit scale into
the integer `FloorPrice` §3 consumes. A zero/negative/missing leg, or a ratio
that underflows to zero, fails the quote (and thus the cycle) **closed** (§6).

The legacy pool's Puppeteer/headless-Chrome dance remains **not** required and
is dropped (ADR-0001 Rust-first; no Bun/Node/Chrome on the payout path). The
client mirrors `accountant::tier_kasplex` conventions: owned `reqwest::Client`
with request + connect timeouts and a `katpool-payout-krc20/<version>`
user-agent, behind a `FloorPriceSource` trait so the engine and tests can
substitute a fake. The same CoinGecko endpoint is the target of the Blackbox
`indexer` synthetic probe (B5), so monitoring tracks the real dependency.

### 6. Circuit breaker fails the cycle CLOSED, never guesses a price

The quote source is wrapped in a circuit breaker (closed → open after
`failure_threshold` consecutive failures → half-open after `cooldown` →
closed on success). While open, requests short-circuit without hitting the
API. Unlike the tier classifier — which safely falls back to the *lower*
rebate tier — there is **no safe fallback price**: a stale or guessed price
gives the wrong amount on every payout. So on an open circuit (or any quote failure) the
**NACHO cycle is skipped** for this tick and retried next tick. Under-paying
by deferring a cycle is recoverable; paying the wrong amount is not. The
breaker state machine is pure and time-injected for deterministic tests.
The price is fetched **once per cycle** ("cache for cycle duration",
architecture §4.4) by the engine (M5.5).

## Consequences

- Positive: elite rebate is paid exactly once, in the place it is audited.
- Positive: conversion is exact and reproducible; no `f64` in value math.
- Positive: a flapping/blocked price API degrades to "cycle skipped + alert",
  never to a wrong payment.
- Negative: a sustained price-API outage stalls NACHO payouts until it
  recovers. Acceptable: rebate balances persist in sompi and pay next cycle.
- Negative: `docs/architecture.md` §4.4 now disagrees with code; it is
  marked legacy-descriptive and points here. Mitigation: this ADR is the
  governing reference.

### Confirmation

- `payout-krc20::rebate` unit + property tests pin the exact conversion,
  the floor-price parse (incl. rejection cases), and the dust gate.
- `payout-krc20::quote` tests cover the circuit-breaker transitions
  (deterministic, time-injected) and the HTTP client against a wiremock
  server (success, non-200, malformed body, timeout → failure).
- End-to-end confirmation arrives with the M5.6 testnet-10 rehearsal: a
  recipient's KRC-20 balance increments by the computed amount.

## More Information

- Supersedes the payout-time multiplier in `docs/architecture.md` §4.4.
- Builds on ADR-0012 (accrual model), ADR-0015 (inscription envelope),
  ADR-0013 (verification posture).
- Legacy reference: `katpool-payment/src/trxs/krc20/{swapToKrc20,transferKrc20Tokens}.ts`.
