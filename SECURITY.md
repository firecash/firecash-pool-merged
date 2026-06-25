# Security Policy

## Supported versions

This repository is the **production** Kaspa mining pool at `kas.katpool.com`.
The Rust runtime, public API, KAS payouts, and NACHO rebates on mainnet are
supported from the `main` branch. The legacy `Nacho-the-Kat/katpool-app`
stack is retired.

| Version | Supported          |
|---------|--------------------|
| `main`  | :white_check_mark: |
| `< 1.0` | :x:                |

## Reporting a vulnerability

**Do not** open public GitHub issues for security findings. They expose
the pool treasury and connected miners to active exploitation before a
fix is shipped.

Instead, report privately via one of the following channels, in order
of preference:

1. **GitHub Security Advisories**: open a private advisory at
   <https://github.com/Nacho-the-Kat/katpool/security/advisories/new>.
   This creates a private thread visible only to the maintainers and
   you, and integrates with GitHub's coordinated-disclosure tooling.
2. **Email**: <socials@onargon.com> with the subject line beginning
   `[katpool-security]`. Encrypt with the operator's PGP key if your
   finding includes proof-of-concept exploit code.

Please include:

- A clear description of the issue and its impact.
- The component(s) affected (e.g. `payout-kas`, `bridge`, `api`).
- Reproduction steps or a proof-of-concept where applicable.
- Your assessment of severity (Critical / High / Medium / Low).
- Whether you intend to publish a coordinated disclosure, and your
  preferred timeline. Default policy is 90 days from acknowledgment.

## Our commitments

| Stage | Target time-to-action |
|---|---|
| Acknowledge receipt | within 72 hours |
| Initial triage + severity classification | within 7 days |
| Fix in mainline (Critical / High) | within 30 days |
| Fix in mainline (Medium / Low) | within 90 days |
| Public disclosure | coordinated with reporter; defaults to 90 days after acknowledgment |

For findings affecting custody of treasury funds (private key handling,
payout signing, KRC-20 transaction construction), the Critical timeline
applies regardless of CVSS score.

## Scope

In scope:

- All code under this repository's `bridge/`, `accountant/`,
  `payout-kas/`, `payout-krc20/`, `api/`, `katpool/`, and `crates/`.
- The runtime deployment configuration under `ops/`.
- The on-disk database schema and migrations.
- Operational runbooks (incorrect step ordering that risks fund loss).
- Production frontends `katpool-dashboard-new/` and `katpool-landing-new/`.

Out of scope (but please still report, just informationally):

- Vulnerabilities in upstream dependencies that we cannot patch
  ourselves; we will coordinate with upstream.
- Issues in the retired `katpool-app` repository.
- Social-engineering attacks that don't involve a code or
  configuration defect.
- DoS amplification via miner abuse — these are tracked as anti-abuse
  improvements, not security advisories, unless they bypass the
  mitigations documented in `docs/threat-model.md`.

## What you can expect in our response

- Acknowledgment from a maintainer (not an automated bot).
- A working dialogue: we'll explain what we understand, ask for
  clarifications, and share our fix approach.
- Credit in the published advisory and the `CHANGELOG.md` entry,
  unless you ask us not to.
- A post-fix retrospective written publicly so other operators of
  similar systems can benefit.

## Out-of-band escalation

If you believe you've found a critical issue and have not received an
acknowledgment within 72 hours, escalate by tagging the operator's
GitHub handle (`@argonmining`) in the private advisory thread, or
mention `urgent` in the email subject line. Pool funds are at stake
and we treat the channel accordingly.
