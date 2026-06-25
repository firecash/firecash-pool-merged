//! Wire-contract snapshot tests (ADR-0021 acceptance).
//!
//! These lock the public JSON shape of every `/api/v1` response **without a
//! database**: response models are built from fixed, deterministic inputs and
//! snapshotted with `insta`. A change to a field name, money encoding, or
//! enum label will fail the snapshot — exactly the regression guard the
//! dashboard depends on. DB-backed behavior (status codes, real rows) is
//! covered separately by `tests/endpoints.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{DateTime, Utc};
use insta::assert_json_snapshot;

use api::models::{
    ActiveMinersHistory, ActiveMinersPointView, ActiveSessionsView, BalanceResponse, BlockCounts,
    BlockView, BlocksPage, CycleView, CyclesPage, FirmwareBreakdown, FirmwareEntryView,
    FullRebateResponse, GeoBreakdown, GeoEntryView, HashrateHistory, HashratePointView,
    KasBalanceView, LeaderboardEntryView, LeaderboardResponse, MinerPayoutView, MinerPayoutsPage,
    MinerProfile, NachoRebateView, PayoutTotals, PoolRejectsResponse, PoolStats, RejectReasonCount,
    RejectsResponse, TreasuryView, WorkerView, WorkersResponse,
};
use api::money::KasAmount;

fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .expect("valid rfc3339")
        .with_timezone(&Utc)
}

const ADDR: &str = "kaspa:qz4j8mu269z8llgcczmfukm9fan2fq822kzxu4cfukd5fqrhxpsv2zhs9jxnp";

#[test]
fn pool_stats_wire() {
    let resp = PoolStats {
        as_of: ts("2026-01-02T03:04:05Z"),
        window_secs: 600,
        miners_active: 3,
        workers_active: 5,
        hashrate_hs: 1.23e12,
        accepted_shares: 4_096,
        blocks: BlockCounts {
            found: 2,
            submitted_to_node: 1,
            confirmed_blue: 0,
            matured: 7,
            orphaned: 1,
        },
        payouts: PayoutTotals {
            kas_confirmed: KasAmount::from_sompi(173_951_600_000),
            nacho_confirmed: KasAmount::from_sompi(50_000_000),
            confirmed_payouts: 12,
        },
        treasury: Some(TreasuryView {
            captured_at: ts("2026-01-02T03:04:05Z"),
            kas_balance: KasAmount::from_sompi(9_900_000_000),
            nacho_balance: "123456789".to_owned(),
            daa_score: 414_699_558,
            blue_score: 414_700_000,
        }),
    };
    assert_json_snapshot!(resp);
}

#[test]
fn balance_wire() {
    let resp = BalanceResponse {
        address: ADDR.to_owned(),
        network: "mainnet".to_owned(),
        kas: KasBalanceView {
            allocated: KasAmount::from_sompi(900_000_000),
            paid: KasAmount::from_sompi(200_000_000),
            payable: KasAmount::from_sompi(700_000_000),
        },
        nacho_rebate: NachoRebateView {
            accrued: KasAmount::from_sompi(25_000_000),
            paid: KasAmount::from_sompi(0),
            pending: KasAmount::from_sompi(25_000_000),
        },
    };
    assert_json_snapshot!(resp);
}

#[test]
fn miner_profile_wire() {
    let resp = MinerProfile {
        address: ADDR.to_owned(),
        network: "mainnet".to_owned(),
        first_seen_at: ts("2025-12-01T00:00:00Z"),
        last_seen_at: ts("2026-01-02T03:04:05Z"),
        window_secs: 600,
        accepted_shares: 1_000,
        rejected_shares: 4,
        hashrate_hs: 7.5e9,
        workers_count: 2,
        kas: KasBalanceView {
            allocated: KasAmount::from_sompi(900_000_000),
            paid: KasAmount::from_sompi(200_000_000),
            payable: KasAmount::from_sompi(700_000_000),
        },
        nacho_rebate: NachoRebateView::zero(),
    };
    assert_json_snapshot!(resp);
}

#[test]
fn workers_wire() {
    let resp = WorkersResponse {
        address: ADDR.to_owned(),
        window_secs: 600,
        workers: vec![WorkerView {
            name: "rig-01".to_owned(),
            first_seen_at: ts("2025-12-01T00:00:00Z"),
            last_seen_at: ts("2026-01-02T03:04:05Z"),
            accepted_shares: 512,
            hashrate_hs: 3.2e9,
        }],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn blocks_page_wire() {
    let resp = BlocksPage {
        blocks: vec![BlockView {
            id: 42,
            hash: "aa".repeat(32),
            status: "matured",
            daa_score: 414_699_558,
            blue_score: Some(414_700_000),
            found_at: ts("2026-01-02T03:00:00Z"),
            confirmed_at: Some(ts("2026-01-02T03:01:00Z")),
            matured_at: Some(ts("2026-01-02T06:00:00Z")),
            reward: Some(KasAmount::from_sompi(11_000_000_000)),
        }],
        next_before: Some(42),
    };
    assert_json_snapshot!(resp);
}

#[test]
fn cycles_page_wire() {
    let resp = CyclesPage {
        cycles: vec![CycleView {
            id: 7,
            kind: "kas",
            status: "settled",
            daa_start: 1_000,
            daa_end: 2_000,
            planned_at: ts("2026-01-02T00:00:00Z"),
            settled_at: Some(ts("2026-01-02T00:05:00Z")),
            total: KasAmount::from_sompi(500_000_000),
            total_recipients: 3,
        }],
        next_before: None,
    };
    assert_json_snapshot!(resp);
}

#[test]
fn miner_payouts_page_wire() {
    let resp = MinerPayoutsPage {
        address: ADDR.to_owned(),
        payouts: vec![MinerPayoutView {
            id: 99,
            cycle_id: 7,
            kind: "nacho",
            amount: KasAmount::from_sompi(33_000_000),
            status: "confirmed",
            tx_hash: None,
            krc20_commit_hash: Some("bb".repeat(32)),
            krc20_reveal_hash: Some("cc".repeat(32)),
            planned_at: ts("2026-01-02T00:00:00Z"),
            submitted_at: Some(ts("2026-01-02T00:01:00Z")),
            confirmed_at: Some(ts("2026-01-02T00:10:00Z")),
            failure_reason: None,
            nacho_amount: Some("500000000".to_owned()),
        }],
        next_before: None,
    };
    assert_json_snapshot!(resp);
}

#[test]
fn rejects_wire() {
    let resp = RejectsResponse {
        address: ADDR.to_owned(),
        window_secs: 600,
        total: 9,
        by_reason: vec![
            RejectReasonCount {
                reason: "low_difficulty",
                count: 6,
            },
            RejectReasonCount {
                reason: "stale",
                count: 3,
            },
        ],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn pool_rejects_wire() {
    let resp = PoolRejectsResponse {
        window_secs: 3_600,
        total: 12,
        by_reason: vec![
            RejectReasonCount {
                reason: "low_difficulty",
                count: 7,
            },
            RejectReasonCount {
                reason: "bad_pow",
                count: 5,
            },
        ],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn hashrate_history_wire() {
    let resp = HashrateHistory {
        from: ts("2026-01-02T00:00:00Z"),
        to: ts("2026-01-02T02:00:00Z"),
        bucket: "1h",
        points: vec![
            HashratePointView {
                bucket_start: ts("2026-01-02T00:00:00Z"),
                hashrate_hs: 1.0e9,
                partial: false,
            },
            HashratePointView {
                bucket_start: ts("2026-01-02T01:00:00Z"),
                hashrate_hs: 2.0e9,
                partial: false,
            },
        ],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn leaderboard_wire() {
    let resp = LeaderboardResponse {
        window_secs: 3_600,
        entries: vec![
            LeaderboardEntryView {
                rank: 1,
                address: ADDR.to_owned(),
                network: "mainnet".to_owned(),
                accepted_shares: 4_096,
                hashrate_hs: 1.2e12,
                pool_share: 0.62,
            },
            LeaderboardEntryView {
                rank: 2,
                address: ADDR.to_owned(),
                network: "mainnet".to_owned(),
                accepted_shares: 2_048,
                hashrate_hs: 6.0e11,
                pool_share: 0.31,
            },
        ],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn active_miners_history_wire() {
    let resp = ActiveMinersHistory {
        from: ts("2026-01-02T00:00:00Z"),
        to: ts("2026-01-02T02:00:00Z"),
        bucket: "1h",
        points: vec![
            ActiveMinersPointView {
                bucket_start: ts("2026-01-02T00:00:00Z"),
                miners: 12,
            },
            ActiveMinersPointView {
                bucket_start: ts("2026-01-02T01:00:00Z"),
                miners: 17,
            },
        ],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn pool_geo_wire() {
    let resp = GeoBreakdown {
        window_secs: 86_400,
        entries: vec![
            GeoEntryView {
                country: "US".to_owned(),
                workers: 12,
                sessions: 18,
            },
            GeoEntryView {
                country: "DE".to_owned(),
                workers: 4,
                sessions: 5,
            },
        ],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn pool_active_sessions_wire() {
    let resp = ActiveSessionsView {
        active_sessions: 5,
        active_workers: 3,
    };
    assert_json_snapshot!(resp);
}

#[test]
fn firmware_breakdown_wire() {
    let resp = FirmwareBreakdown {
        window_secs: 86_400,
        entries: vec![
            FirmwareEntryView {
                app: Some("IceRiverMiner/1.2.0".to_owned()),
                workers: 8,
                sessions: 11,
            },
            FirmwareEntryView {
                app: Some("GodMiner/1.0".to_owned()),
                workers: 3,
                sessions: 4,
            },
            FirmwareEntryView {
                app: None,
                workers: 0,
                sessions: 2,
            },
        ],
    };
    assert_json_snapshot!(resp);
}

#[test]
fn full_rebate_wire() {
    assert_json_snapshot!(
        "full_rebate_elite",
        FullRebateResponse {
            address: ADDR.to_owned(),
            tier: Some("elite"),
            full_rebate: true,
        }
    );
    assert_json_snapshot!(
        "full_rebate_unknown",
        FullRebateResponse {
            address: ADDR.to_owned(),
            tier: None,
            full_rebate: false,
        }
    );
}
