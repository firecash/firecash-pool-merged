-- ===============================================================
-- KRC-20 commit/reveal frozen network fees.
--
-- Background. The KRC-20 commit and reveal transactions previously
-- reserved a fixed fee (0.0001 KAS each). kaspad's mempool rejects any
-- transaction whose fee is below the mass-based minimum relay fee
-- (RejectInsufficientFee), and a real commit/reveal pair needs roughly
-- 0.002 / 0.0018 KAS — so the fixed fee was ~20x too low and both
-- transactions would be rejected (the same defect already fixed for the
-- KAS payout engine).
--
-- New model. Fees are sized adaptively from the node fee-estimate,
-- floored at the relay minimum and computed from each transaction's
-- exact (signed-length) mass, mirroring the KAS FeeRate policy. Because
-- the commit `change` value and the reveal `return` value are committed
-- into their respective transaction ids — which are recorded BEFORE
-- broadcast for crash-safety — the resolved fees must be reproducible on
-- a crash-resume. They are therefore frozen onto the transfer row the
-- first time it is executed; every subsequent reconstruction (reveal
-- build, drift check, commit re-broadcast) reuses the persisted value so
-- the re-derived txid is bit-identical.
--
-- Both columns are NULL until the transfer is first executed (a still
-- `pending`, never-broadcast transfer has no frozen fee yet).
-- ===============================================================

ALTER TABLE krc20_pending_transfer
    ADD COLUMN commit_fee_sompi BIGINT CHECK (commit_fee_sompi >= 0),
    ADD COLUMN reveal_fee_sompi BIGINT CHECK (reveal_fee_sompi >= 0);
