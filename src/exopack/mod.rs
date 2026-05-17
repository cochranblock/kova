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

#[cfg(feature = "ats_fixtures")]
pub mod ats_fixtures;

#[cfg(feature = "cc_features")]
pub mod cc_features;

#[cfg(feature = "training_mine_tests")]
pub mod training_mine_tests;

#[cfg(feature = "tool_call_parser")]
pub mod tool_call_parser;

#[cfg(feature = "router_spec")]
pub mod router_spec;

#[cfg(feature = "agent_loop_tests")]
pub mod agent_loop_tests;

#[cfg(feature = "router_training_tests")]
pub mod router_training_tests;
