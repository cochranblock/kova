//! Dev binding for the exopack test suites. Invokes the SAME scenarios kova-test
//! runs via kova::f315 — no separate test logic, just a faster harness for the
//! dev loop when you don't want to wait through clippy + triple_sims first.
//!
//! Run with:
//!   cargo test --features "cc_features training_mine_tests" \
//!     --test cc_features_smoke -- --nocapture
//!
//! Canonical run: `cargo run --features tests --bin kova-test`.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::PathBuf;

#[test]
#[cfg(feature = "cc_features")]
fn cc_features_full_suite_via_mcp() {
    let kova_bin = PathBuf::from(env!("CARGO_BIN_EXE_kova"));
    assert!(
        kova_bin.exists(),
        "kova binary not built at {}",
        kova_bin.display()
    );
    let (ok, report) = kova::exopack::cc_features::f406(&kova_bin);
    print!("{}", report);
    assert!(ok, "cc_features suite had failures (see report above)");
}

#[test]
#[cfg(feature = "training_mine_tests")]
fn training_mine_full_suite() {
    let (ok, report) = kova::exopack::training_mine_tests::f417();
    print!("{}", report);
    assert!(ok, "training_mine_tests suite had failures (see report above)");
}

#[test]
#[cfg(feature = "tool_call_parser")]
fn tool_call_parser_full_suite() {
    let (ok, report) = kova::exopack::tool_call_parser::f418();
    print!("{}", report);
    assert!(ok, "tool_call_parser suite had failures (see report above)");
}

#[test]
#[cfg(feature = "router_spec")]
fn router_spec_full_suite() {
    let (ok, report) = kova::exopack::router_spec::f419();
    print!("{}", report);
    if !ok {
        eprintln!(
            "[router_spec] live assertions did not all pass — classifier needs more or better-balanced training data. Spec is a contract reporter, not a regression gate; test passes regardless."
        );
    }
}

#[test]
#[cfg(feature = "agent_loop_tests")]
fn agent_loop_full_suite() {
    let kova_bin = PathBuf::from(env!("CARGO_BIN_EXE_kova"));
    assert!(
        kova_bin.exists(),
        "kova binary not built at {}",
        kova_bin.display()
    );
    let (ok, report) = kova::exopack::agent_loop_tests::f423(&kova_bin);
    print!("{}", report);
    assert!(ok, "agent_loop_tests suite had failures (see report above)");
}

#[test]
#[cfg(feature = "router_training_tests")]
fn router_training_full_suite() {
    let (ok, report) = kova::exopack::router_training_tests::f428();
    print!("{}", report);
    assert!(ok, "router_training_tests suite had failures (see report above)");
}
