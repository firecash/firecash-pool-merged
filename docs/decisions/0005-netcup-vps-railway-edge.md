---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0005: Stay on the existing NetCup VPS; use Railway for the edge

> **Note (2026-06-02):** the **stratum-edge** decision below (Railway
> TCP proxies) is superseded by
> [ADR-0022](0022-multiport-stratum-and-flyio-anycast-edge.md): Railway
> cannot expose the legacy stratum ports (`1111–8888`), so the geo edge
> moves to fly.io anycast. The NetCup-origin and observability decisions
> in this ADR still stand.

## Context and Problem Statement

The legacy pool runs on an existing NetCup VPS that has been
operational for some time. The previous deployment plan considered
moving to Hetzner CCX13 (~$15/month, 2 dedicated vCPU / 8 GB) as the
new central host, plus Fly.io for geographic stratum proxies.

When we captured the actual NetCup specs, the box turned out to be
substantially larger than assumed: 20 vCPU AMD EPYC 9634, 94 GB RAM,
3 TB SSD, swap already disabled. This is ~10× the budget I had
planned for. The user also prefers using Railway across the
non-pool stack (consistency with other Nacho-the-Kat projects) and
keeping ongoing costs minimal.

## Decision Drivers

- Minimise marginal operating cost (operator constraint)
- Operational consistency with the rest of the Nacho-the-Kat
  portfolio (Railway is already where other services live)
- Sufficient capacity headroom — proven by the measured NetCup
  specs
- Failure-domain separation between pool and observability /
  geographic edge (see ADR-0004)
- Avoid migrating production to a smaller box for no benefit

## Considered Options

1. **NetCup VPS for the pool, Railway for the geographic stratum
   edge, Railway for the observability stack.**
2. **Hetzner CCX13 for the pool, Fly.io for the edge, Railway for
   observability.** Original plan.
3. **NetCup VPS for everything (including observability and edge
   on the same box).**
4. **Multi-VPS HA across providers** — out of cost scope.

## Decision Outcome

**Chosen option: 1.** Stay on the existing NetCup VPS for the pool;
use Railway TCP proxies in three regions (us-east, eu-west,
ap-southeast) for the stratum edge; use Railway for the self-hosted
observability project (ADR-0004).

The NetCup capacity (~10× headroom) makes the box-size question
moot. The marginal cost of this architecture is zero (NetCup is
already paid) plus ~$5–10/month for Railway TCP proxies and ~$30–40
for the observability project — total monthly net cost is
substantially lower than the legacy Datadog + Fly.io footprint.

### Consequences

- Positive: zero migration of production data — the new pool
  builds and runs alongside the legacy one on the same box
- Positive: ~10× capacity headroom; we can run a long-lived shadow
  environment for the entire build period
- Positive: Railway-everywhere for non-pool services keeps the
  operator's mental model simple
- Positive: total recurring cost meaningfully lower than legacy
- Negative: Single-VPS topology means a NetCup outage takes the
  pool offline. Mitigated by the documented DR procedure
  (B2 → fresh VPS in < 4 h)
- Negative: TCP Proxy on Railway has no static inbound IP; we use
  CNAME-to-Railway-managed-hostname. Miners use a hostname, not an
  IP, so this is acceptable.

### Confirmation

- [`capacity-plan.md`](../capacity-plan.md) documents the measured
  NetCup specs
- Railway edge services in three regions are wired up in Phase 8
- DNS uses CNAMEs to Railway-provided proxy hostnames

## Pros and Cons of the Options

### Option 1: NetCup + Railway edge + Railway observability

- Good: continuity (no production migration)
- Good: 10× capacity headroom
- Good: operator-familiar Railway for everything non-pool
- Good: low net monthly cost
- Bad: single-host pool failure domain
- Bad: Railway doesn't provide static inbound IPs (acceptable;
  miners use hostnames)

### Option 2: Hetzner CCX13 + Fly.io + Railway

- Good: smaller VPS = predictable cost
- Bad: forces production migration before cutover for no benefit
  (current box is fine)
- Bad: introduces Fly.io as a fourth vendor when Railway suffices
- Rejected once NetCup capacity was confirmed

### Option 3: NetCup-only for everything

- Good: zero Railway cost
- Bad: violates failure-domain-separation driver (ADR-0004)
- Rejected

### Option 4: Multi-VPS HA

- Good: stronger uptime
- Bad: doubles cost; not justified at current pool size
- Out of scope; revisit if pool growth warrants

## More Information

- Railway TCP Proxy docs:
  <https://docs.railway.com/networking/tcp-proxy>
- Railway pricing as of March 2026:
  <https://docs.railway.com/pricing/plans>
- Companion ADRs:
  [0004 (self-host observability)](0004-self-host-observability.md),
  [`capacity-plan.md`](../capacity-plan.md)
