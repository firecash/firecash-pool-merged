---
status: accepted
date: 2026-05-25
deciders: argonmining
---

# ADR-0003: Encrypt treasury secrets at rest with sops + age (no plaintext)

## Context and Problem Statement

The legacy pool stores `TREASURY_PRIVATE_KEY` as a plaintext value in
`/root/docker_deployment/katpool-app/.env`, world-readable to root
only but otherwise unencrypted. This is below the operational bar for
a wallet holding production funds. We need a secrets-at-rest model
that survives accidental disclosure (e.g., a misconfigured backup
ingest, a misrouted file copy, or a curious dependency).

This ADR covers the **encryption-at-rest** mechanism. The
**wallet-balance-topology** decision (hot-only versus hot+cold) is
the separate [ADR-0008](0008-hot-only-treasury-with-os-isolation.md).

## Decision Drivers

- No plaintext secrets ever land on disk, in git, or in CI artifacts
- Decryption material lives off the production host
- Standard tooling, well-audited, widely used in industry
- Compatible with systemd `LoadCredentialEncrypted=` for in-process
  delivery without a shell decryption step

## Considered Options

1. **sops + age** — Mozilla sops with age (modern ChaCha20-Poly1305)
   recipients
2. **sops + GPG** — same wrapper, traditional GPG recipients
3. **HashiCorp Vault** — full secrets-management server, online API
4. **Kubernetes-style sealed-secrets** — only if we adopt k8s; we are
   not (see ADR-0005 NetCup + Railway)
5. **Roll our own age-encrypted blob** — no tooling layer
6. **Continue plaintext .env** — no-op; rejected

## Decision Outcome

**Chosen option: 1 (sops + age).** Encrypted file is committed to git
at `ops/secrets/secrets.sops.yaml`. The age private key lives on the
operator's workstation only (1Password vault) and never touches the
production host or CI. Decryption at process boot happens via
systemd `LoadCredentialEncrypted=`, which reads the encrypted file
and feeds plaintext through a credentials-tmpfs that only the
katpool process can read.

### Consequences

- Positive: no plaintext key in git, on disk, or in CI
- Positive: file changes are reviewable in PRs even though encrypted
  (sops produces stable structural diffs)
- Positive: per-key recipient management (add/remove operators
  without re-encrypting everything)
- Positive: works with systemd's credentials subsystem; the key
  never needs a shell, env-var, or world-readable file at runtime
- Negative: if the age private key is lost AND the offline backup is
  lost, the encrypted file is unrecoverable. Mitigation: offline
  backup of the age key in a fireproof safe (paper + USB).
- Negative: sops file requires a working `sops` + `age` install to
  decrypt; pinned in CI and onboarding docs.

### Confirmation

- `ops/secrets/secrets.sops.yaml` exists, is committed, and decrypts
  successfully with the operator's age key
- The age **private** key file is not in any repository, not in any
  CI secret, and not on the production VPS
- Systemd unit uses `LoadCredentialEncrypted=` with a per-host
  credential-encryption key derived from the VPS's TPM (where
  available) or a host-bound key file owned by root
- Quarterly key-rotation drill exists ([`runbooks/11-key-rotation.md`](../runbooks/11-key-rotation.md))

## Pros and Cons of the Options

### Option 1: sops + age

- Good: modern, audited, widely used (Linux Foundation projects,
  countless OSS deployments)
- Good: age is a simple, modern primitive (ChaCha20-Poly1305 +
  X25519); no GPG complexity
- Good: cooperates with systemd credentials
- Bad: requires extra tooling on operator's machine

### Option 2: sops + GPG

- Good: same wrapper benefits as option 1
- Bad: GPG is significantly more complex to set up and key-manage
- Bad: poorer ergonomics for new operators

### Option 3: HashiCorp Vault

- Good: best-in-class for large org with many secrets
- Bad: runtime dependency on a Vault server; outage of Vault = pool
  outage
- Bad: substantial operational overhead for a single-operator pool
- Bad: a Vault server itself needs custody hardening — turtles down

### Option 4: Sealed-secrets / k8s

- N/A: we're not on k8s (ADR-0005)

### Option 5: Roll our own

- Bad: no audit, no community review, no documented disaster recovery
- Bad: not better than sops + age in any dimension

### Option 6: Plaintext

- Bad: the problem we're solving
- Rejected

## More Information

- sops: <https://github.com/getsops/sops>
- age: <https://github.com/FiloSottile/age>
- systemd LoadCredentialEncrypted:
  <https://systemd.io/CREDENTIALS/>
- Companion ADRs:
  [0008 (hot-only with OS isolation)](0008-hot-only-treasury-with-os-isolation.md),
  the operational document [`custody.md`](../custody.md)
