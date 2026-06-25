-- Durable payout acceptance score.
--
-- KAS payout confirmation infers on-chain acceptance from the treasury *change*
-- coin produced by the payout tx (its block_daa_score gives the accepting DAA
-- height, used for the maturity-depth check). That signal is ephemeral: once
-- the change coin is spent (e.g. swept by the consolidation engine), a later
-- confirmation pass can no longer observe it and the payout would otherwise be
-- stranded in `submitted` forever even though the miner was paid on chain.
--
-- Record the accepting DAA score the first time the change coin is observed, so
-- confirmation can advance `accepted -> confirmed` purely by depth thereafter —
-- independent of whether the change coin still exists in the UTXO set.
--
-- Nullable: stays NULL until first observed on chain, and for rows that predate
-- this migration. BIGINT matches the chain DAA-score domain (daa_score columns).

ALTER TABLE payout
    ADD COLUMN accepted_daa_score BIGINT CHECK (accepted_daa_score IS NULL OR accepted_daa_score >= 0);
