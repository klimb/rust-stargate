use divan::{Bencher, black_box};
use sg_seq::sgmain;
use sgcore::benchmark::run_util_function;

/// Benchmark simple integer sequence
#[divan::bench]
fn seq_integers(bencher: Bencher) {
    bencher.bench(|| {
        black_box(run_util_function(sgmain, &["1", "1000000"]));
    });
}

/// Benchmark sequence with custom separator
#[divan::bench]
fn seq_custom_separator(bencher: Bencher) {
    bencher.bench(|| {
        black_box(run_util_function(sgmain, &["-s", ",", "1", "1000000"]));
    });
}

/// Benchmark sequence with step
#[divan::bench]
fn seq_with_step(bencher: Bencher) {
    bencher.bench(|| {
        black_box(run_util_function(sgmain, &["1", "2", "1000000"]));
    });
}

/// Benchmark formatted output
#[divan::bench]
fn seq_formatted(bencher: Bencher) {
    bencher.bench(|| {
        black_box(run_util_function(
            sgmain,
            &["-f", "%.3f", "1", "0.1", "10000"]
        ));
    });
}

fn main() {
    divan::main();
}
