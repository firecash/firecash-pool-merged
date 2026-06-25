-- ===============================================================
-- Coinbase-reward anchor for PROP allocation.
--
-- Background. The original Phase 3 model matured a *found block* and
-- allocated its reward by parsing that block's own coinbase. That is
-- incorrect against Kaspa consensus:
--
--   * A block B's own coinbase pays the miners of the blocks B merges,
--     not B itself. B's reward is paid in the coinbase of a later chain
--     block C that merges B as blue (consensus/src/processes/coinbase.rs
--     in rusty-kaspa tn10-toc3).
--   * Coinbase maturity is 1000 DAA-score depth (BPS 10 x 100 s), not a
--     100 blue-score depth.
--   * Block "blueness" is GHOSTDAG colour (get_current_block_color), not
--     selected-parent-chain membership (is_chain_block).
--
-- New model. The pool's realised reward is the set of *coinbase UTXOs*
-- credited to the pool address. Each such UTXO, once it reaches the
-- consensus coinbase-maturity depth, is the ground-truth unit of reward:
-- an exact amount with an acceptance DAA score. We anchor PROP allocation
-- on the coinbase UTXO outpoint (naturally unique -> exactly-once), and
-- the `block` table becomes pure lifecycle telemetry (which found blocks
-- were accepted blue vs orphaned).
--
-- This migration introduces `coinbase_reward` and re-anchors
-- `share_allocation` from `block_id` to `coinbase_reward_id`.
-- ===============================================================

-- ---------------------------------------------------------------
-- coinbase_reward — one row per matured coinbase UTXO credited to
-- the pool address. Anchored by the UTXO outpoint.
-- ---------------------------------------------------------------

CREATE TABLE coinbase_reward (
    id                      BIGSERIAL PRIMARY KEY,
    -- The coinbase UTXO outpoint (transaction id + output index). The
    -- transaction is the coinbase of the chain block that accepted (as
    -- blue) the pool block whose work earned this reward.
    outpoint_transaction_id BYTEA NOT NULL CHECK (octet_length(outpoint_transaction_id) = 32),
    outpoint_index          INTEGER NOT NULL CHECK (outpoint_index >= 0),
    -- Exact sompi value of the UTXO — the reward distributed by PROP.
    amount_sompi            BIGINT NOT NULL CHECK (amount_sompi >= 0),
    -- DAA score of the block that created the UTXO (the accepting
    -- chain block). Both the maturity gate and the PROP window key off
    -- this value.
    block_daa_score         BIGINT NOT NULL CHECK (block_daa_score >= 0),
    -- When the tracker first observed the matured UTXO.
    discovered_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- When the allocation engine distributed it (NULL until allocated).
    -- Doubles as the idempotency gate: an allocated reward is never
    -- re-distributed.
    allocated_at            TIMESTAMPTZ,
    UNIQUE (outpoint_transaction_id, outpoint_index)
);

-- Drives the tracker's "find the next reward to allocate" query.
CREATE INDEX idx_coinbase_reward_unallocated
    ON coinbase_reward (block_daa_score)
    WHERE allocated_at IS NULL;

-- ---------------------------------------------------------------
-- Re-anchor share_allocation: block_id -> coinbase_reward_id.
--
-- Any rows produced under the old (incorrect) reward model are not
-- reconcilable with the new anchor, and no environment has produced a
-- valid allocation under the new model yet, so we clear and re-anchor.
-- Dropping `block_id` also drops its FK and the UNIQUE(block_id,
-- wallet_id) constraint that referenced it.
-- ---------------------------------------------------------------

DELETE FROM share_allocation;

ALTER TABLE share_allocation
    DROP COLUMN block_id;

ALTER TABLE share_allocation
    ADD COLUMN coinbase_reward_id BIGINT NOT NULL
        REFERENCES coinbase_reward(id) ON DELETE CASCADE;

ALTER TABLE share_allocation
    ADD CONSTRAINT share_allocation_reward_wallet_uniq
        UNIQUE (coinbase_reward_id, wallet_id);

CREATE INDEX idx_share_allocation_reward
    ON share_allocation (coinbase_reward_id);
