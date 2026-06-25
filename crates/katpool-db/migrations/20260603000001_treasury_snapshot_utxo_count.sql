-- Treasury UTXO consolidation observability.
--
-- The consolidation engine records a treasury_snapshot every tick so the
-- spendable UTXO count (and KAS balance) is visible over time. Add a nullable
-- utxo_count: it stays NULL for rows written by other snapshot paths (e.g. key
-- rotation) and for rows that predate this migration; the consolidation engine
-- always populates it.
--
-- INTEGER is ample headroom for any realistic treasury fragmentation; the count
-- is a spendable-coin tally, not a monetary value.

ALTER TABLE treasury_snapshot
    ADD COLUMN utxo_count INTEGER CHECK (utxo_count IS NULL OR utxo_count >= 0);
