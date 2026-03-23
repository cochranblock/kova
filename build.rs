//! Build script: Cap'n Proto (daemon), WASM thin client (serve).

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

fn main() {
    // LOUD WARNING: xml5ever 0.16.2 future-compat issue
    // Pinned by kalosm 0.4.0 → readability 0.2.0 → markup5ever_rcdom → xml5ever 0.16.2
    // xml5ever uses trailing semicolon in macro expression position (will become error in future Rust).
    // Fix: upstream kalosm needs to bump readability to 0.3.0+ (which uses xml5ever 0.22+).
    // Tracked: kalosm pinned to 0.4.0, no newer release available.
    // When kalosm 0.5+ ships, remove this note and run: cargo update -p readability -p xml5ever
    println!("cargo:warning=PINNED: xml5ever 0.16.2 future-compat (trailing semicolon in macro). Blocked by kalosm 0.4.0 → readability 0.2.0. Upgrade when kalosm 0.5+ ships.");
    #[cfg(feature = "daemon")]
    {
        capnpc::CompilerCommand::new()
            .src_prefix("schema")
            .file("schema/kova_protocol.capnp")
            .run()
            .expect("Cap'n Proto schema compilation failed. Install capnp: brew install capnp");
    }

    #[cfg(feature = "serve")]
    {
        // KOVA_SKIP_WASM=1 skips WASM build (deploy to nodes with pre-built dist/)
        if std::env::var("KOVA_SKIP_WASM").as_deref() != Ok("1") {
            build_wasm();
        }
    }
}

#[cfg(feature = "serve")]
fn build_wasm() {
    use std::path::Path;
    use std::process::Command;
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wasm_dir = manifest_dir.join("wasm");
    let wasm_manifest = wasm_dir.join("Cargo.toml");
    let dist_dir = wasm_dir.join("dist");

    println!("cargo:rerun-if-changed=src/web_client");
    println!("cargo:rerun-if-changed=wasm/Cargo.toml");

    // Build WASM thin client for wasm32
    let status = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            wasm_manifest.to_str().unwrap(),
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .current_dir(manifest_dir)
        .status()
        .expect("failed to run cargo build for wasm client");

    if !status.success() {
        panic!("WASM build failed. Install wasm32 target: rustup target add wasm32-unknown-unknown");
    }

    // wasm/ has its own [workspace], so output goes to wasm/target/
    let wasm_path = wasm_dir.join("target/wasm32-unknown-unknown/release/kova_web.wasm");

    // Run wasm-bindgen (requires: cargo install wasm-bindgen-cli)
    let status = Command::new("wasm-bindgen")
        .args([
            "--target",
            "web",
            "--out-dir",
            dist_dir.to_str().unwrap(),
            "--out-name",
            "kova_web",
            wasm_path.to_str().unwrap(),
        ])
        .current_dir(manifest_dir)
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(_) => panic!("wasm-bindgen failed"),
        Err(_) => panic!(
            "wasm-bindgen not found. Install with: cargo install wasm-bindgen-cli"
        ),
    }
}