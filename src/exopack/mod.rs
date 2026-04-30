// Unlicense — public domain — cochranblock.org
//! exopack — testing augmentation: screenshot, video, interfaces, API mocks, triple sims, demo, baked_demo.

#![forbid(unsafe_code)]
// P13 compressed identifiers (t60, f61, s80) trigger naming and unused warnings
#![allow(non_camel_case_types, non_snake_case, dead_code, unused_imports)]

#[cfg(feature = "interface")]
pub mod interface;

#[cfg(feature = "mock")]
pub mod mock;

#[cfg(feature = "video")]
pub mod video;

#[cfg(feature = "screenshot")]
pub mod screenshot;

#[cfg(feature = "triple_sims")]
pub mod triple_sims;

#[cfg(feature = "devtools")]
pub mod devtools;

#[cfg(feature = "demo")]
pub mod demo;

#[cfg(feature = "baked_demo")]
pub mod baked_demo;

#[cfg(feature = "standards_check")]
pub mod standards_check;

#[cfg(feature = "checkpoint")]
pub mod checkpoint;

#[cfg(feature = "compaction")]
pub mod compaction;

#[cfg(feature = "dual_mode")]
pub mod dual_mode;

#[cfg(feature = "perm_gate")]
pub mod perm_gate;

#[cfg(feature = "harvest")]
pub mod harvest;
