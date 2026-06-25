//! Tests for `accountant::KasplexTierClassifier` against
//! [`wiremock`]-mocked kasplex endpoints. No real HTTP traffic —
//! every test stands up a fresh wiremock server in-process and
//! points the classifier at it.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)]

use std::time::Duration;

use katpool_domain::WalletAddress;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use accountant::{KasplexConfig, KasplexTierClassifier, TierClassifier, WalletTier};

const TEST_WALLET: &str = "kaspa:qypczcz0lhyf3tfsuqj86e7qc8us7r8a53nhlr4u6x4kq38td0hsjycf7sya7zq";

fn test_wallet() -> WalletAddress {
    WalletAddress::new(TEST_WALLET.to_owned()).expect("valid wallet")
}

/// Standard test harness: spins up two wiremock servers (NFT +
/// KRC-20) and a classifier wired to them. The two `*_template`
/// arguments are the response bodies to serve for *any* GET — for
/// tests that need path-specific routing, mount the Mocks
/// directly (see `caches_result_within_ttl` below).
async fn harness(
    nft_response: ResponseTemplate,
    krc20_response: ResponseTemplate,
) -> (KasplexTierClassifier, MockServer, MockServer) {
    let nft_server = MockServer::start().await;
    let krc20_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(nft_response)
        .mount(&nft_server)
        .await;
    Mock::given(method("GET"))
        .respond_with(krc20_response)
        .mount(&krc20_server)
        .await;

    let cfg = KasplexConfig {
        nft_base: nft_server.uri(),
        krc20_base: krc20_server.uri(),
        nft_ticker: "NACHO".to_owned(),
        krc20_ticker: "NACHO".to_owned(),
        elite_krc20_threshold_base_units: 10_000_000_000_000_000,
        ttl: Duration::from_secs(60),
        http_timeout: Duration::from_secs(2),
        ..KasplexConfig::default()
    };
    let classifier = KasplexTierClassifier::new(cfg).expect("build classifier");
    (classifier, nft_server, krc20_server)
}

// ---- response builders --------------------------------------------

fn nft_ok_with_token() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "message": "success",
        "result": [{"tick": "NACHO", "tokenId": "1", "opScoreMod": "26236285306000"}]
    }))
}

fn nft_ok_empty() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"message": "success", "result": []}))
}

fn krc20_ok_balance(balance: &str, locked: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "message": "successful",
        "result": [{
            "tick": "NACHO",
            "balance": balance,
            "locked": locked,
            "dec": "8",
            "opScoreMod": "1"
        }]
    }))
}

fn krc20_ok_empty() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"message": "successful", "result": []}))
}

#[tokio::test]
async fn returns_elite_when_wallet_holds_an_nft() {
    let (cls, _nft, _krc20) = harness(nft_ok_with_token(), krc20_ok_balance("0", "0")).await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(tier, WalletTier::Elite, "NFT-only path should mark elite");
}

#[tokio::test]
async fn returns_elite_when_wallet_holds_katclaim_nft_only() {
    // No NACHO NFT, no threshold KRC-20 balance — but a KATCLAIM NFT
    // independently qualifies the wallet as Elite.
    let nft_server = MockServer::start().await;
    let krc20_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/krc721/mainnet/address/{TEST_WALLET}/NACHO"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "success",
            "result": []
        })))
        .mount(&nft_server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/krc721/mainnet/address/{TEST_WALLET}/KATCLAIM"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "success",
            "result": [{"tick": "KATCLAIM", "tokenId": "458", "opScoreMod": "1"}]
        })))
        .mount(&nft_server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/krc20/address/{TEST_WALLET}/token/NACHO")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "successful",
            "result": []
        })))
        .mount(&krc20_server)
        .await;

    let cfg = KasplexConfig {
        nft_base: nft_server.uri(),
        krc20_base: krc20_server.uri(),
        nft_ticker: "NACHO".to_owned(),
        krc20_ticker: "NACHO".to_owned(),
        elite_krc20_threshold_base_units: 10_000_000_000_000_000,
        ttl: Duration::from_secs(60),
        http_timeout: Duration::from_secs(2),
        ..KasplexConfig::default()
    };
    let cls = KasplexTierClassifier::new(cfg).expect("build classifier");
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(
        tier,
        WalletTier::Elite,
        "KATCLAIM NFT alone should mark elite"
    );
}

#[tokio::test]
async fn returns_elite_when_wallet_holds_threshold_krc20_balance() {
    // 10^16 = exactly the threshold.
    let (cls, _nft, _krc20) =
        harness(nft_ok_empty(), krc20_ok_balance("10000000000000000", "0")).await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(
        tier,
        WalletTier::Elite,
        "≥ 100M NACHO via balance should mark elite"
    );
}

#[tokio::test]
async fn locked_holdings_count_toward_threshold() {
    // 50M unlocked + 50M locked = 100M → elite.
    let (cls, _nft, _krc20) = harness(
        nft_ok_empty(),
        krc20_ok_balance("5000000000000000", "5000000000000000"),
    )
    .await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(
        tier,
        WalletTier::Elite,
        "balance+locked ≥ threshold should mark elite"
    );
}

#[tokio::test]
async fn returns_standard_when_neither_qualifies() {
    // 1 base unit short of threshold + no NFT → Standard.
    let (cls, _nft, _krc20) =
        harness(nft_ok_empty(), krc20_ok_balance("9999999999999999", "0")).await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(tier, WalletTier::Standard);
}

#[tokio::test]
async fn empty_krc20_result_means_no_balance() {
    let (cls, _nft, _krc20) = harness(nft_ok_empty(), krc20_ok_empty()).await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(tier, WalletTier::Standard);
}

#[tokio::test]
async fn falls_back_to_standard_on_500_from_both_endpoints() {
    // Safe fallback per ADR-0012: never over-rebate.
    let (cls, _nft, _krc20) = harness(ResponseTemplate::new(500), ResponseTemplate::new(500)).await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(tier, WalletTier::Standard);
}

#[tokio::test]
async fn one_endpoint_5xx_other_says_elite_returns_elite() {
    let (cls, _nft, _krc20) = harness(nft_ok_with_token(), ResponseTemplate::new(500)).await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(
        tier,
        WalletTier::Elite,
        "NFT success + KRC-20 5xx → still elite"
    );
}

#[tokio::test]
async fn caches_result_within_ttl() {
    let nft_server = MockServer::start().await;
    let krc20_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/krc721/mainnet/address/{TEST_WALLET}/NACHO")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "success",
            "result": [{"tick": "NACHO", "tokenId": "1", "opScoreMod": "1"}]
        })))
        .expect(1) // critical: only ONE call across the two classify() invocations
        .mount(&nft_server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/krc721/mainnet/address/{TEST_WALLET}/KATCLAIM"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "success",
            "result": []
        })))
        .expect(1)
        .mount(&nft_server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/krc20/address/{TEST_WALLET}/token/NACHO")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "successful",
            "result": []
        })))
        .expect(1)
        .mount(&krc20_server)
        .await;

    let cfg = KasplexConfig {
        nft_base: nft_server.uri(),
        krc20_base: krc20_server.uri(),
        nft_ticker: "NACHO".to_owned(),
        krc20_ticker: "NACHO".to_owned(),
        elite_krc20_threshold_base_units: 10_000_000_000_000_000,
        ttl: Duration::from_secs(60),
        http_timeout: Duration::from_secs(2),
        ..KasplexConfig::default()
    };
    let cls = KasplexTierClassifier::new(cfg).unwrap();

    let t1 = cls.classify(&test_wallet()).await.unwrap();
    let t2 = cls.classify(&test_wallet()).await.unwrap();
    assert_eq!(t1, WalletTier::Elite);
    assert_eq!(t2, WalletTier::Elite);
    assert_eq!(cls.cache_len().await, 1);
    // wiremock's `.expect(1)` enforces single-call-per-server on drop.
}

#[tokio::test]
async fn malformed_json_falls_back_to_standard() {
    let (cls, _nft, _krc20) = harness(
        ResponseTemplate::new(200).set_body_string("not json at all"),
        ResponseTemplate::new(200).set_body_string("[]"),
    )
    .await;
    let tier = cls.classify(&test_wallet()).await.expect("classify");
    assert_eq!(
        tier,
        WalletTier::Standard,
        "malformed JSON → Standard fallback"
    );
}

#[tokio::test]
async fn clear_cache_forces_refetch() {
    let nft_server = MockServer::start().await;
    let krc20_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/krc721/mainnet/address/{TEST_WALLET}/NACHO"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "success",
            "result": []
        })))
        .expect(2)
        .mount(&nft_server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/krc721/mainnet/address/{TEST_WALLET}/KATCLAIM"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "success",
            "result": []
        })))
        .expect(2)
        .mount(&nft_server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/krc20/address/{TEST_WALLET}/token/NACHO")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "successful",
            "result": []
        })))
        .expect(2)
        .mount(&krc20_server)
        .await;

    let cfg = KasplexConfig {
        nft_base: nft_server.uri(),
        krc20_base: krc20_server.uri(),
        nft_ticker: "NACHO".to_owned(),
        krc20_ticker: "NACHO".to_owned(),
        elite_krc20_threshold_base_units: 10_000_000_000_000_000,
        ttl: Duration::from_secs(60),
        http_timeout: Duration::from_secs(2),
        ..KasplexConfig::default()
    };
    let cls = KasplexTierClassifier::new(cfg).unwrap();
    let _ = cls.classify(&test_wallet()).await.unwrap();
    cls.clear_cache().await;
    let _ = cls.classify(&test_wallet()).await.unwrap();
    // wiremock's .expect(2) enforces.
}
