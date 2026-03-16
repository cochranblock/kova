//! Build script: Cap'n Proto (daemon), kova-web WASM (serve).

use std::path::Path;
use std::process::Command;

fn main() {
    #[cfg(feature = "daemon")]
    {
        capnpc::CompilerCommand::new()
            .src_prefix("schema")
            .file("schema/kova_protocol.capnp")
            .run()
            .expect("Cap'n Proto schema compilation failed. Install capnp: brew install capnp");
    }

    #[cfg(feature = "serve")]
    build_kova_web();
}

#[cfg(feature = "serve")]
fn build_kova_web() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let kova_web_dir = manifest_dir.join("kova-web");
    let kova_web_manifest = kova_web_dir.join("Cargo.toml");
    let dist_dir = kova_web_dir.join("dist");
    let workspace_root = manifest_dir.parent().expect("kova must be under workspace root");

    println!("cargo:rerun-if-changed={}", kova_web_dir.display());

    // Build kova-web for wasm32
    let status = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            kova_web_manifest.to_str().unwrap(),
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .current_dir(workspace_root)
        .status()
        .expect("failed to run cargo build for kova-web");

    if !status.success() {
        panic!("kova-web build failed. Install wasm32 target: rustup target add wasm32-unknown-unknown");
    }

    // kova-web has its own [workspace], so output goes to kova-web/target/
    let wasm_path = kova_web_dir
        .join("target/wasm32-unknown-unknown/release/kova_web.wasm");

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
        .current_dir(workspace_root)
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(_) => panic!("wasm-bindgen failed"),
        Err(_) => panic!(
            "wasm-bindgen not found. Install with: cargo install wasm-bindgen-cli"
        ),
    }
}
