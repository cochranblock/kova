//! f90=kova_test. Thin wrapper: delegates to kova::f315(). Use `kova test` or this binary.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

fn main() {
    if let Err(e) = kova::f315() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}