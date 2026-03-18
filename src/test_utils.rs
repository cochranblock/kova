//! Test helpers. kova_test! for traceability, assert_matches for patterns.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

/// Test with fN traceability. Adds #[test] and /// fN=traceability doc.
/// Use: `kova_test!(f62, full_pipeline, { ... })` then `use crate::kova_test;` in test mod.
#[macro_export]
macro_rules! kova_test {
    ($token:ident, $name:ident, $body:block) => {
        #[test]
        #[doc = concat!(stringify!($token), "=traceability")]
        fn $name() $body
    };
}