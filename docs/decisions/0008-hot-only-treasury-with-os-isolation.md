---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0008: Hot-only treasury wallet with OS-level process isolation

## Context and Problem Statement

The pool processes daily KAS payouts to 25–60 recipients and an
analogous daily NACHO rebate cycle. A purely cold-wallet model
(every payout requires manual signing on an offline device) is
not viable at this cadence. A multi-sig or threshold-signature
model is not natively supported by Kaspa today, and the MPC
landscape in May 2026 (post-THORChain incident) has fresh
warnings against GG20-class TSS schemes from Ledger's own CTO.

Hardware-wallet-backed automated signing (Ledger / Trezor) requires
human confirmation per transaction on the device — incompatible
with our daily automated cycle to many recipients.

That narrows the realistic options to:

- A **software hot key** with strong OS-level isolation
- A **two-VM remote signer** (key in a separate, hardened VM with
  rate-limited signing service)
- A **hot/cold split** with periodic manual sweep to cold storage

The operator's preference, given operational simplicity and our
single-operator team size, is the software hot key with rigorous
OS-level isolation. ADR-0003 covers the encryption-at-rest model;
this ADR covers the wallet-balance topology and process isolation.

## Decision Drivers

- Operational simplicity (single operator)
- Daily automated payouts (no human-in-the-loop signing)
- Acceptable residual risk given current treasury size
  (~30M NACHO, ~25k KAS)
- Hardware wallets don't support automated batch signing on Kaspa
- MPC schemes have known recent failure modes
- Cost discipline (no HSM hardware investment)

## Considered Options

1. **Hot-only wallet with strong OS-level isolation.** Single key
   on the production VPS, sops-encrypted at rest, mlocked and
   zeroized at runtime, behind systemd hardening.
2. **Two-VM remote signer.** Pool VM sends unsigned tx to a
   separate, hardened Signer VM over a private wireguard mesh; the
   signer validates against a policy, signs, returns.
3. **Hot/cold split with manual sweep.** Keep only ~7–14 days of
   expected payouts on the hot wallet; periodically sweep overflow
   to a cold address.
4. **Hardware-backed automated signing.** Not viable on Kaspa in
   May 2026; documented in research notes.
5. **MPC / threshold signing.** Recent TSS exploits (THORChain
   2026) raise the risk; not justified at our scale.

## Decision Outcome

**Chosen option: 1 (Hot-only with OS-level isolation).** Documented
in [`custody.md`](../custody.md). We accept the residual risk that
a full VPS compromise drains the hot wallet, mitigated by:

- Unprivileged uid, no shell account
- Encrypted-at-rest key (ADR-0003), delivered via systemd
  `LoadCredentialEncrypted=`
- `mlock`, `zeroize`, no `Debug` impl for key bytes
- systemd hardening: `NoNewPrivileges`, `ProtectSystem=strict`,
  `SystemCallFilter`, `MemoryDenyWriteExecute`, etc.
- Swap disabled at OS level
- Per-cycle and per-recipient payout-policy guards (max KAS per
  cycle, max recipients per cycle, max new-address rate)
- Offline backup of the sops file + age key (paper + USB)
- fail2ban on SSH; treasury account is `nologin`
- Audit log of every payout via Loki

The decision is **revisitable** if the hot balance routinely
exceeds 14 days of expected payouts, in which case we add a
manual sweep-to-cold step (option 3) on top.

### Consequences

- Positive: simplest operational model
- Positive: no per-payout human action required
- Positive: implementable now without external dependencies
- Negative: full-host compromise = full treasury drain. This is
  the residual risk we explicitly accept.
- Negative: cannot scale to much-larger treasuries without
  revisiting

### Confirmation

- `crates/katpool-secrets` enforces the in-memory hygiene
  (mlock + zeroize + no Debug)
- Systemd hardening profile measured at boot — chaos test in
  Phase 9 attempts forbidden operations and confirms `EPERM`
- Payout-policy guards reject any transaction violating per-cycle
  or per-recipient thresholds
- Sweep-to-cold script (`ops/scripts/sweep_overflow_to_cold.sh`)
  exists and is documented in [`custody.md`](../custody.md)
- Sweep is triggered manually when the alert
  `TreasuryHotBalanceExceedsBudget` fires

## Pros and Cons of the Options

### Option 1: Hot-only + OS isolation

- Good: simple, ships now
- Good: no per-payout latency
- Bad: full-host compromise = full drain

### Option 2: Two-VM remote signer

- Good: pool host compromise no longer = full drain (signer is
  separate, policy-restricted)
- Bad: +1 VM to operate, secure, back up
- Bad: roughly +1 week of engineering work
- Bad: adds a network hop and a failure mode to every payout
- Future-revisit option if hot balance grows

### Option 3: Hot/cold split + manual sweep

- Good: reduces blast radius
- Good: minimal additional code (just the sweep script)
- Implemented as the **manual companion** to option 1 — not an
  alternative. When/if hot balance grows, sweep weekly.

### Option 4: Hardware wallet (Ledger / Trezor)

- Bad: not viable for automated batched Kaspa payouts in May 2026
- Rejected by capability, not preference

### Option 5: MPC / threshold

- Bad: recent TSS exploit class (GG20)
- Bad: complex, hard to audit
- Rejected

## More Information

- THORChain TSS incident (2026): cited in research notes
- Ledger Enterprise HSM On-Premise: out-of-scope for current pool
  scale
- Companion docs:
  [`custody.md`](../custody.md),
  [`threat-model.md`](../threat-model.md)
- Companion ADR: [0003 (sops + age)](0003-sops-only-treasury-custody.md)
