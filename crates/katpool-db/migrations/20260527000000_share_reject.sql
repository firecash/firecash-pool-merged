-- Phase 3 M2: persistence for stratum share rejections.
--
-- M1 only metric-ticked PoolEvent::ShareRejected. M2 persists each
-- rejection so the per-miner stats surface (and operator
-- forensics) can answer "which workers are getting rejected, by
-- which reason, when".
--
-- The reason enum mirrors `katpool_domain::ShareRejectReason::as_str()`
-- output one-for-one — DON'T renumber or rename variants without
-- updating both. The downstream metric label `reason` (already
-- exported by the bridge in Phase 1) uses identical strings, so
-- existing alerts will line up against the DB persistence.

CREATE TYPE share_reject_reason AS ENUM (
    'stale',
    'low_difficulty',
    'bad_pow',
    'missing_job',
    'malformed_frame',
    'duplicate_submit',
    'bad_address'
);

CREATE TABLE share_reject (
    id              BIGSERIAL PRIMARY KEY,
    wallet_id       BIGINT NOT NULL REFERENCES wallet(id) ON DELETE RESTRICT,
    worker_id       BIGINT NOT NULL REFERENCES worker(id) ON DELETE RESTRICT,
    reason          share_reject_reason NOT NULL,
    rejected_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    correlation_id  UUID NOT NULL
);

-- Per-miner recency: "give me the last N rejects for worker X".
CREATE INDEX idx_share_reject_worker_time
    ON share_reject (worker_id, rejected_at DESC);

-- Per-wallet recency: "give me the last N rejects for wallet X".
CREATE INDEX idx_share_reject_wallet_time
    ON share_reject (wallet_id, rejected_at DESC);

-- Reason-bucket aggregation: "how many low_difficulty rejects
-- across the whole pool in the last hour".
CREATE INDEX idx_share_reject_reason_time
    ON share_reject (reason, rejected_at DESC);
