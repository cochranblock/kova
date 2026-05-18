//! Build script.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

fn main() {
    // LOUD WARNING: xml5ever 0.16.2 future-compat issue
    // Pinned by kalosm 0.4.0 → readability 0.2.0 → markup5ever_rcdom → xml5ever 0.16.2
    // xml5ever uses trailing semicolon in macro expression position (will become error in future Rust).
    // Fix: upstream kalosm needs to bump readability to 0.3.0+ (which uses xml5ever 0.22+).
    // When kalosm 0.5+ ships, remove this note and run: cargo update -p readability -p xml5ever
    println!("cargo:warning=PINNED: xml5ever 0.16.2 future-compat (trailing semicolon in macro). Blocked by kalosm 0.4.0 → readability 0.2.0. Upgrade when kalosm 0.5+ ships.");
}
