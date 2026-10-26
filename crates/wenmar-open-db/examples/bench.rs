//! How long decoding takes: one VIN at a time, and batches over the pool.
//!
//!     cargo run --release -p wenmar-open-db --example bench -- DATA_FILE [VINS]
//!
//! The VINs are those of the parity corpus, `data/corpus/nhtsa.json`, or
//! one per line in VINS. Each is decoded once with nothing cached, then
//! many times through the cache, then in batches of 50 spread over four
//! connections. The machine's other work changes the figures; compare runs
//! made one after the other.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use wenmar_open_db::Db;
use wenmar_vin::DecodeOptions;

const OPTIONS: DecodeOptions = DecodeOptions {
    model_year: None,
    current_year: Some(2026),
};
const CONNECTIONS: usize = 4;
const BATCH: usize = 50;

fn corpus(path: Option<String>) -> Vec<String> {
    match path {
        Some(path) => std::fs::read_to_string(path)
            .expect("the VIN list can be read")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
        None => {
            let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/corpus/nhtsa.json");
            let json: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(file).expect("the corpus exists"))
                    .expect("the corpus is JSON");
            json["vins"]
                .as_object()
                .expect("the corpus lists VINs")
                .keys()
                .cloned()
                .collect()
        }
    }
}

/// Decodes every VIN once on one connection.
async fn one_by_one(db: &Db, vins: &Arc<Vec<String>>) -> Duration {
    let vins = Arc::clone(vins);
    db.run(move |worker| {
        let started = Instant::now();
        for vin in vins.iter() {
            let _ = worker.decode(vin, OPTIONS);
        }
        started.elapsed()
    })
    .await
    .expect("the pool answers")
}

fn per_vin(total: Duration, count: usize) -> Duration {
    total / u32::try_from(count.max(1)).unwrap_or(u32::MAX)
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let data = args.next().expect("usage: bench DATA_FILE [VINS]");
    let vins = Arc::new(corpus(args.next()));
    println!("{} VINs, {CONNECTIONS} connections", vins.len());

    let plain = Db::open_with_cache(Path::new(&data), CONNECTIONS, 0)
        .await
        .expect("the data file opens");
    let total = one_by_one(&plain, &vins).await;
    println!(
        "no cache        {:>10.3?} per VIN",
        per_vin(total, vins.len())
    );
    drop(plain);

    let db = Db::open(Path::new(&data), CONNECTIONS)
        .await
        .expect("the data file opens");
    let first = one_by_one(&db, &vins).await;
    println!(
        "cache, first    {:>10.3?} per VIN",
        per_vin(first, vins.len())
    );
    let mut warm = Duration::ZERO;
    for _ in 0..5 {
        warm += one_by_one(&db, &vins).await;
    }
    println!(
        "cache, held     {:>10.3?} per VIN",
        per_vin(warm, vins.len() * 5)
    );

    let started = Instant::now();
    let mut tasks = tokio::task::JoinSet::new();
    let db = Arc::new(db);
    for chunk in vins.chunks(BATCH) {
        for piece in chunk.chunks(BATCH.div_ceil(CONNECTIONS / 2)) {
            let db = Arc::clone(&db);
            let piece = piece.to_vec();
            tasks.spawn(async move {
                db.run(move |worker| {
                    for vin in &piece {
                        let _ = worker.decode(vin, OPTIONS);
                    }
                })
                .await
            });
        }
        while tasks.join_next().await.is_some() {}
    }
    let batches = vins.len().div_ceil(BATCH);
    println!(
        "batches of {BATCH}   {:>10.3?} per batch, split as the API splits them",
        per_vin(started.elapsed(), batches)
    );
    println!("held: {:?}", db.vin_cache_stats());
}
