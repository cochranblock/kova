// Unlicense — cochranblock.org
//! kova node — Worker daemon for Kova swarm. Cap'n Proto protocol.
//! Phase 1: Schema loaded. Network listener deferred.

pub mod protocol {
    capnp::generated_code!(pub mod kova_protocol_capnp);
}

pub fn run() {
    println!("kova node: schema loaded, daemon stub (Phase 1)");
}
