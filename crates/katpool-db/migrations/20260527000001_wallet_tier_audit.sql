-- Phase 3 M3: wallet-tier postgres enum + audit-trail columns on
-- `share_allocation`.
--
-- Per ADR-0012: every allocation row records the exact fee
-- parameters used at computation time so historical allocations
-- remain reproducible across future operator changes to
-- `KATPOOL_FEE_TOPLINE_BPS`. The accountant's `WalletTier` Rust
-- enum maps to this postgres `wallet_tier` enum via `sqlx::Type`
-- in `accountant/src/config.rs`.
--
-- ## Migration semantics for existing rows
--
-- The bootstrap migration's `share_allocation` table currently
-- holds zero rows in production (the accountant doesn't write to
-- it until this milestone). We still add `DEFAULT`s on the new
-- columns so the `NOT NULL` constraint can be applied without a
-- two-step migration: the defaults satisfy the constraint for
-- any pre-existing rows (none, in practice), and we drop the
-- defaults immediately after so future INSERT statements must specify the
-- column values explicitly — preventing an "I forgot to set the
-- tier" code bug from silently producing rows tagged
-- `standard, 0bps, 0bps`.

CREATE TYPE wallet_tier AS ENUM ('standard', 'elite');

ALTER TABLE share_allocation
    ADD COLUMN applied_topline_bps SMALLINT NOT NULL DEFAULT 0
        CHECK (applied_topline_bps >= 0 AND applied_topline_bps <= 1000),
    ADD COLUMN applied_rebate_bps  SMALLINT NOT NULL DEFAULT 0
        CHECK (applied_rebate_bps >= 0 AND applied_rebate_bps <= 10000),
    ADD COLUMN applied_tier        wallet_tier NOT NULL DEFAULT 'standard';

ALTER TABLE share_allocation
    ALTER COLUMN applied_topline_bps DROP DEFAULT,
    ALTER COLUMN applied_rebate_bps  DROP DEFAULT,
    ALTER COLUMN applied_tier        DROP DEFAULT;
