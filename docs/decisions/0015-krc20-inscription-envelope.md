---
status: accepted
date: 2026-05-31
deciders: argonmining
---

# ADR-0015: KRC-20 inscription envelope is byte-compatible with the live production transfer

## Context and Problem Statement

Phase 5 (`payout-krc20`) inscribes KRC-20 `transfer` operations on Kaspa
via the kasplex commit/reveal flow. The *commit* transaction pays a P2SH
output whose redeem script embeds the operation; the *reveal* transaction
spends it, exposing the script on-chain for the kasplex indexer.

The exact redeem-script bytes are consensus-critical for the indexer: if
they drift from what kasplex accepts, the transfer is broadcast and
confirmed on-chain but **never credited** — money leaves the treasury and
the recipient's KRC-20 balance never moves. We must reproduce the accepted
format byte-for-byte in Rust (`kaspa-txscript`).

Sources disagreed on the envelope layout:

1. The scaffold doc-comment in `payout-krc20/src/lib.rs` and one public
   README (`codecustard/kaspa`) describe:
   `<pubkey> OP_CHECKSIG_ECDSA OP_FALSE OP_IF "kasplex" OP_1 OP_0 OP_0 <json> OP_ENDIF`
   — i.e. ECDSA checksig and an ordinals-style `OP_1 OP_0 OP_0` marker
   triplet between the tag and the JSON.
2. The live production pool (`katpool-payment/src/trxs/krc20/krc20Transfer.ts`),
   the widely-used `coinchimp/kaspa-krc20-apps` reference, and the kaspa
   WASM `ScriptBuilder` convention all emit:
   `<x-only pubkey> OP_CHECKSIG OP_FALSE OP_IF "kasplex" addI64(0) <json> OP_ENDIF`
   — Schnorr checksig with a 32-byte x-only key and a **single** `OP_0`
   push (`addI64(0)` encodes to one `0x00` byte), no marker triplet.

## Decision Drivers

* Correctness is empirical, not aspirational: real money rides on the
  indexer accepting the bytes.
* We must not assume; we resolve against the strongest available evidence.
* The Rust output must be reproducible and pinned by a test.

## Considered Options

1. Follow the prose spec / scaffold comment (`OP_CHECKSIG_ECDSA`,
   `OP_1 OP_0 OP_0`).
2. Follow the live production envelope (`OP_CHECKSIG`, single `OP_0`).

## Decision Outcome

**Chosen option: 2 (live production envelope).** The decisive evidence is
on-chain: the production `katpool-payment` pool is *currently settling
NACHO rebates* using option 2, which means the kasplex indexer demonstrably
accepts and credits that exact byte layout. Two independent ecosystem
implementations (`coinchimp`, the kaspa WASM `ScriptBuilder` examples) emit
the identical structure. The prose "marker triplet" description is
contradicted by every working `ScriptBuilder` implementation — `addI64(0n)`
produces one `OP_0`, not `OP_1 OP_0 OP_0` — and is not present in any
transaction the indexer has credited for this pool. The Schnorr/x-only
choice is also consistent with our Phase 4 signer, which signs Schnorr.

`payout-krc20::inscription::build_transfer_inscription` therefore emits:

```text
0x20 <32-byte x-only pubkey>   # push pubkey
0xac                           # OP_CHECKSIG (Schnorr; NOT 0xab OP_CHECKSIG_ECDSA)
0x00                           # OP_FALSE
0x63                           # OP_IF
0x07 "kasplex"                 # push tag
0x00                           # OP_0  (add_i64(0); NOT OP_1 OP_0 OP_0)
<push> <json>                  # canonical push of {"p":"krc-20","op":"transfer",…}
0x68                           # OP_ENDIF
```

The JSON payload serialises its fields in the fixed order
`p, op, tick, amt, to`, compact (no whitespace), matching
`JSON.stringify(data, null, 0)` in production. Field order is part of the
on-chain bytes, so the serde struct fixes that order.

### Consequences

- Positive: byte-for-byte parity with a layout the indexer is proven to
  credit; no live-money guess.
- Positive: the envelope is pinned by `tests/inscription.rs`
  (`envelope_is_byte_exact_kasplex_layout`) reconstructed from first
  principles, so any future drift fails CI.
- Negative: if kasplex later ships a v2 envelope that *requires* the marker
  triplet, we must add it behind a version flag. Mitigation: the layout is
  isolated in one pure function with a single call site.

### Confirmation

- `payout-krc20/tests/inscription.rs` asserts the exact bytes, the canonical
  compact JSON, P2SH derivation (testnet-10 `kaspatest:` prefix), the
  hash-binds-the-payload property, and the reveal signature script
  `<sig><pushed redeem script>`.
- The first live testnet-10 reveal in the Phase 5 rehearsal must show the
  recipient's KRC-20 balance increment via the kasplex API — the
  end-to-end confirmation that the bytes are accepted.

## More Information

- Production source of truth: `docker_deployment/katpool-payment/src/trxs/krc20/krc20Transfer.ts`.
- Ecosystem references: `coinchimp/kaspa-krc20-apps`, `KaffinPX/KasplexBuilder`.
- Related: ADR-0002 (forked rusty-kaspa), `docs/kips.md` §5.2 (KRC-20 reveal mass).
- Follow-up: the reveal transaction's `transient_storage_mass` handling is
  planned through the Phase 4 `katpool-storagemass` planner (`docs/kips.md`).
