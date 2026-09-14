//! Explicit scale check for the local change index cache.
//!
//! It is ignored by default so CI keeps no timing threshold; run it directly:
//! `cargo test --release --test scale -- --ignored --nocapture`
//!
//! The fixture uses empty change directories: it isolates index enumeration cost,
//! not full work-package validation, and never measures a real project.
mod common;
use common::{INDEX_CACHE, Sandbox};
use std::{fs, time::Duration, time::Instant};

const SIZES: [usize; 3] = [100, 1_000, 10_000];
const REPEATS: usize = 3;

fn history(s: &Sandbox, from: usize, to: usize) {
    for index in from..to {
        fs::create_dir_all(s.path(&format!("doco/changes/archived/history-{index:05}"))).unwrap();
    }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn without_cache(s: &Sandbox) {
    match fs::remove_file(s.path(INDEX_CACHE)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("cannot drop the cache: {error}"),
    }
}

#[test]
#[ignore = "explicit scale benchmark; run with --ignored --nocapture"]
fn hot_cache_avoids_rescanning_history() {
    let s = Sandbox::new();
    s.init();
    println!(
        "release run: {REPEATS} samples per size, empty historical directories;\n\
         cold samples measure the full directory scan, warm samples a cache hit"
    );
    let mut created = 0;
    let mut results = Vec::new();
    for size in SIZES {
        history(&s, created, size);
        created = size;
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for repeat in 0..REPEATS {
            without_cache(&s);
            let id = format!("cold-{size}-{repeat}");
            let start = Instant::now();
            s.ok(&["new", &id]);
            cold.push(start.elapsed());
            fs::remove_dir_all(s.path(&format!("doco/changes/active/{id}"))).unwrap();

            // Removing the previous sample invalidated the stamps, so rebuild the
            // cache outside the measurement before every warm sample.
            s.ok(&["fix"]);
            let id = format!("warm-{size}-{repeat}");
            let start = Instant::now();
            s.ok(&["new", &id]);
            warm.push(start.elapsed());
            fs::remove_dir_all(s.path(&format!("doco/changes/active/{id}"))).unwrap();
        }
        let (cold, warm) = (median(cold), median(warm));
        println!(
            "{size:>6} changes: cold new {cold:?}, warm new {warm:?} \
             (includes reading and rewriting the whole cache)"
        );
        results.push((size, cold, warm));
    }

    // Structural proof that a warm `new` never re-enumerated the directories:
    // a deliberately mislabeled record that only an authoritative scan would
    // correct must survive the next warm write unchanged.
    without_cache(&s);
    s.ok(&["fix"]);
    assert_eq!(
        s.index_changes().len(),
        created,
        "the cache must list every change"
    );
    let text = s.index_text().unwrap().replace(
        "change,history-00000,archived",
        "change,history-00000,active",
    );
    s.write(INDEX_CACHE, &text);
    s.ok(&["new", "after-forged-cache"]);
    assert!(
        s.index_changes()
            .contains(&"history-00000=active".to_string()),
        "a warm write rescanned the directories instead of trusting the cache"
    );

    let (size, cold, warm) = *results.last().unwrap();
    assert!(
        warm * 2 < cold,
        "the cache must avoid the historical scan at {size} changes: warm {warm:?} vs cold {cold:?}"
    );
}
