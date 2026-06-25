//! Minimal stratum-protocol CPU miner for Phase 1 acceptance smoke runs.
//!
//! The public ecosystem does not ship a maintained CPU stratum client
//! for post-Crescendo Kaspa: `kaspanet/cpuminer` (Michael Sutton's
//! recommended CPU miner) and `elichai/kaspa-miner` are both *solo*
//! miners that talk gRPC directly to kaspad, bypassing any stratum
//! layer. To validate the bridge's stratum + share-handling surface
//! end-to-end we therefore need a custom client. This file is that
//! client.
//!
//! Design notes:
//!
//! * The PoW math is **identical** to what the bridge itself runs in
//!   `share_handler::handle_submit`. We reuse `kaspa_pow::matrix::Matrix`
//!   and `kaspa_hashes::PowHash` directly — these are the same crate
//!   versions pinned in our workspace lockfile, so we cannot drift
//!   from what the bridge expects on the verification side.
//! * Stratum framing follows the same JSON-RPC line-delimited
//!   convention the bridge already understands (matching the upstream
//!   v1.1.0 stratum-bridge behaviour and Bitmain/IceRiver ASIC
//!   defaults).
//! * The `mining.notify` parser only handles the "Legacy" job-data
//!   format (array of 4 u64 + timestamp number). That is what the
//!   bridge emits when neither IceRiver heuristics nor BzMiner big-job
//!   heuristics fire, which is exactly the path triggered by this
//!   example's `mining.subscribe` payload.
//!
//! Run with:
//!
//! ```sh
//! cargo run --release --example cpu_stratum_miner -p kaspa-stratum-bridge -- \
//!     --stratum 127.0.0.1:5559 \
//!     --wallet  kaspatest:qpaaslz6kn4untywu50v59zkxztwlkwsulna78d7g4rt7elgly2az5jv3fxwz \
//!     --worker  smoke-rig \
//!     --threads 8 \
//!     --duration-secs 60
//! ```

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use kaspa_hashes::{Hash, PowHash};
use kaspa_math::Uint256;
use kaspa_pow::matrix::Matrix;
use serde_json::{Value, json};

struct Args {
    stratum: String,
    wallet: String,
    worker: String,
    threads: usize,
    duration_secs: u64,
}

fn parse_args() -> Args {
    let mut stratum = "127.0.0.1:5559".to_string();
    let mut wallet = String::new();
    let mut worker = "smoke-rig".to_string();
    let mut threads: usize = num_cpus_or(8);
    let mut duration_secs: u64 = 60;
    let mut iter = env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--stratum" => stratum = iter.next().expect("--stratum value"),
            "--wallet" => wallet = iter.next().expect("--wallet value"),
            "--worker" => worker = iter.next().expect("--worker value"),
            "--threads" => threads = iter.next().expect("--threads value").parse().expect("threads u32"),
            "--duration-secs" => duration_secs = iter.next().expect("--duration-secs value").parse().expect("duration u64"),
            "--help" | "-h" => {
                eprintln!(
                    "usage: cpu_stratum_miner [--stratum HOST:PORT] [--wallet kaspatest:...] [--worker NAME] [--threads N] [--duration-secs N]"
                );
                std::process::exit(0);
            }
            other => panic!("unknown arg: {other}"),
        }
    }
    if wallet.is_empty() {
        panic!("--wallet is required (use `cargo run --example gen_testnet_addr` to generate one)");
    }
    Args { stratum, wallet, worker, threads, duration_secs }
}

fn num_cpus_or(default: usize) -> usize {
    thread::available_parallelism().map(std::num::NonZero::get).unwrap_or(default)
}

/// One job derived from a `mining.notify` message. Cheap to clone so
/// every worker thread can hold its own copy without locking.
#[derive(Clone)]
struct Job {
    job_id: String,
    pre_pow_hash: Hash,
    timestamp: u64,
}

/// Shared mutable state every worker thread reads from.
struct Shared {
    current_job: Mutex<Option<Job>>,
    /// Pool difficulty target as set by the last `mining.set_difficulty`.
    pool_target: Mutex<Option<Uint256>>,
    stop: AtomicBool,
    shares_submitted: AtomicU64,
    hashes: AtomicU64,
}

fn main() {
    let args = parse_args();
    let started = Instant::now();

    let stream = TcpStream::connect(&args.stratum).expect("connect to stratum");
    stream.set_nodelay(true).expect("nodelay");
    let stream_writer = Arc::new(Mutex::new(stream.try_clone().expect("clone stream")));
    let reader = BufReader::new(stream.try_clone().expect("clone stream"));

    // Subscribe + authorize. We do not request extranonce — the bridge
    // is happy without and that keeps the Legacy job-data format
    // active (no IceRiver / BzMiner heuristics get triggered).
    send_request(&stream_writer, 1, "mining.subscribe", json!(["katpool-cpu-stratum-miner/0.1"]));
    send_request(&stream_writer, 2, "mining.authorize", json!([format!("{}.{}", args.wallet, args.worker)]));

    let shared = Arc::new(Shared {
        current_job: Mutex::new(None),
        pool_target: Mutex::new(None),
        stop: AtomicBool::new(false),
        shares_submitted: AtomicU64::new(0),
        hashes: AtomicU64::new(0),
    });

    // Spawn the worker pool. Each thread picks a disjoint nonce range
    // via `(i, nstep)` striping and re-syncs every time a new job
    // lands.
    let mut workers = Vec::with_capacity(args.threads);
    for i in 0..args.threads {
        let shared = Arc::clone(&shared);
        let stream_writer = Arc::clone(&stream_writer);
        let nstep = args.threads as u64;
        let worker_label = args.worker.clone();
        let wallet = args.wallet.clone();
        workers.push(thread::spawn(move || mine_loop(i as u64, nstep, shared, stream_writer, &worker_label, &wallet)));
    }

    // Spawn the reader; it owns the inbound JSON-RPC stream and
    // mutates `shared` as new jobs/difficulties arrive.
    let reader_shared = Arc::clone(&shared);
    let reader_writer = Arc::clone(&stream_writer);
    let reader_thread = thread::spawn(move || read_loop(reader, reader_shared, reader_writer));

    // Periodic progress dump so the runtime can be visually monitored
    // during long unattended runs.
    let progress_shared = Arc::clone(&shared);
    let progress_thread = thread::spawn(move || {
        let start = Instant::now();
        loop {
            thread::sleep(Duration::from_secs(5));
            if progress_shared.stop.load(Ordering::Relaxed) {
                break;
            }
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let shares = progress_shared.shares_submitted.load(Ordering::Relaxed);
            let hashes = progress_shared.hashes.load(Ordering::Relaxed);
            eprintln!(
                "[progress] elapsed={:.1}s shares={} hashrate={:.2} MH/s",
                elapsed,
                shares,
                (hashes as f64 / elapsed) / 1_000_000.0,
            );
        }
    });

    // Drive the test for the requested wall-clock duration.
    thread::sleep(Duration::from_secs(args.duration_secs));
    shared.stop.store(true, Ordering::Release);

    for w in workers {
        let _ = w.join();
    }
    // The reader loop is blocked on TCP; closing the writer (which is
    // the same socket fd) terminates the read. Best-effort drop.
    drop(stream_writer);
    let _ = reader_thread.join();
    let _ = progress_thread.join();

    let elapsed = started.elapsed().as_secs_f64();
    let shares = shared.shares_submitted.load(Ordering::Relaxed);
    let hashes = shared.hashes.load(Ordering::Relaxed);
    println!(
        "{{\"elapsed_secs\":{:.2},\"threads\":{},\"hashes\":{},\"shares_submitted\":{},\"hashrate_mhs\":{:.2}}}",
        elapsed,
        args.threads,
        hashes,
        shares,
        (hashes as f64 / elapsed.max(0.001)) / 1_000_000.0
    );
}

fn send_request(writer: &Mutex<TcpStream>, id: u64, method: &str, params: Value) {
    let frame = json!({ "id": id, "method": method, "params": params, "jsonrpc": "2.0" });
    let mut line = frame.to_string();
    line.push('\n');
    let mut w = writer.lock().expect("writer poisoned");
    w.write_all(line.as_bytes()).expect("write frame");
}

fn send_share(writer: &Mutex<TcpStream>, id: u64, wallet: &str, worker: &str, job_id: &str, nonce: u64) {
    let nonce_hex = format!("{:016x}", nonce);
    let identity = format!("{wallet}.{worker}");
    send_request(writer, id, "mining.submit", json!([identity, job_id, nonce_hex]));
}

fn read_loop(reader: BufReader<TcpStream>, shared: Arc<Shared>, writer: Arc<Mutex<TcpStream>>) {
    for line in reader.lines() {
        if shared.stop.load(Ordering::Relaxed) {
            break;
        }
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[reader] parse error: {e}: {line}");
                continue;
            }
        };
        let method = v.get("method").and_then(Value::as_str);
        match method {
            Some("mining.notify") => {
                if let Some(job) = parse_notify(&v) {
                    *shared.current_job.lock().expect("job mutex") = Some(job.clone());
                    eprintln!("[reader] new job id={} ts={} pre_pow_hash={}", job.job_id, job.timestamp, job.pre_pow_hash);
                }
            }
            Some("mining.set_difficulty") => {
                if let Some(diff) = v.get("params").and_then(|p| p.get(0)).and_then(Value::as_f64) {
                    let target = diff_to_target(diff);
                    *shared.pool_target.lock().expect("target mutex") = Some(target);
                    eprintln!("[reader] set_difficulty diff={diff} target={target:x}");
                }
            }
            _ => {
                // Responses (id present, no method) — pass through silently.
                let _ = &writer;
            }
        }
    }
}

fn parse_notify(v: &Value) -> Option<Job> {
    let params = v.get("params")?.as_array()?;
    if params.len() < 3 {
        return None;
    }
    let job_id = params[0].as_str()?.to_string();
    // Legacy format: params[1] = [u64, u64, u64, u64] (LE chunks of pre_pow_hash)
    let chunks = params[1].as_array()?;
    if chunks.len() < 4 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in chunks.iter().take(4).enumerate() {
        let v = chunk.as_u64()?;
        bytes[i * 8..i * 8 + 8].copy_from_slice(&v.to_le_bytes());
    }
    let timestamp = params[2].as_u64()?;
    Some(Job { job_id, pre_pow_hash: Hash::from_bytes(bytes), timestamp })
}

/// Pool-difficulty target uses the same formula as
/// `bridge::hasher::diff_to_target_standard`. We re-derive it inline
/// to keep the example self-contained (the function is `pub(crate)`
/// in the bridge library and not re-exported).
fn diff_to_target(diff: f64) -> Uint256 {
    use num_bigint::BigUint;
    use num_traits::Num;
    const MAX_TARGET_HEX: &str = "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF";
    let max_target: BigUint = <BigUint as Num>::from_str_radix(MAX_TARGET_HEX, 16).expect("max target hex");
    let diff_scaled = (diff.max(f64::MIN_POSITIVE) * 1e18) as u128;
    let big = (max_target * BigUint::from(1_000_000_000_000_000_000u128)) / BigUint::from(diff_scaled);
    let mut be = big.to_bytes_be();
    if be.len() < 32 {
        let mut padded = vec![0u8; 32 - be.len()];
        padded.extend_from_slice(&be);
        be = padded;
    } else if be.len() > 32 {
        be = be[be.len() - 32..].to_vec();
    }
    Uint256::from_be_bytes(be.try_into().expect("32 bytes"))
}

fn mine_loop(worker_idx: u64, nstep: u64, shared: Arc<Shared>, writer: Arc<Mutex<TcpStream>>, worker_label: &str, wallet: &str) {
    let mut local_id: u64 = 100 + worker_idx;
    let mut last_job_id: Option<String> = None;
    let mut hasher: Option<PowHash> = None;
    let mut matrix: Option<Matrix> = None;
    let mut nonce: u64 = worker_idx;

    loop {
        if shared.stop.load(Ordering::Relaxed) {
            break;
        }
        let job_opt = shared.current_job.lock().expect("job mutex").clone();
        let target = match *shared.pool_target.lock().expect("target mutex") {
            Some(t) => t,
            None => {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        let job = match job_opt {
            Some(j) => j,
            None => {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        };

        if last_job_id.as_ref() != Some(&job.job_id) {
            // New job: rebuild the PowHash and Matrix once, then strip-mine.
            hasher = Some(PowHash::new(job.pre_pow_hash, job.timestamp));
            matrix = Some(Matrix::generate(job.pre_pow_hash));
            last_job_id = Some(job.job_id.clone());
            nonce = worker_idx;
        }

        // Mine a chunk of nonces, then re-check the job (so a new
        // mining.notify is picked up within ~1 ms).
        let h = hasher.as_ref().expect("hasher");
        let m = matrix.as_ref().expect("matrix");
        let mut local_hashes: u64 = 0;
        for _ in 0..4096_u64 {
            let raw = h.clone().finalize_with_nonce(nonce);
            let heavy = m.heavy_hash(raw);
            local_hashes += 1;
            let pow_value = Uint256::from_le_bytes(heavy.as_bytes());
            if pow_value <= target {
                send_share(&writer, local_id, wallet, worker_label, &job.job_id, nonce);
                local_id += 1024;
                shared.shares_submitted.fetch_add(1, Ordering::Relaxed);
            }
            nonce = nonce.wrapping_add(nstep);
        }
        shared.hashes.fetch_add(local_hashes, Ordering::Relaxed);
    }
}
