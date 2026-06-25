-- Pool-wide hashrate history scans `share` on `credited_at` over 24h+ spans.
-- Without a time index the planner seq-scans millions of rows (~1.8s per
-- request, tripping the API's 5s timeout under concurrent dashboard load).
CREATE INDEX idx_share_credited_at
    ON share (credited_at)
    INCLUDE (difficulty);
