// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! f90=kova_test. Thin wrapper: delegates to kova::run_test_suite(). Use `kova test` or this binary.

fn main() {
    if let Err(e) = kova::run_test_suite() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
