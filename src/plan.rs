#![allow(non_camel_case_types, non_snake_case, dead_code)]
//! Plan layer. Intent → action DAG. f14.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::{t0, t1};
use std::path::PathBuf;

/// t3 = Plan. DAG of actions. s3=steps, s4=project, s5=approuter_dir, s7=project_hint.
#[derive(Debug, Clone)]
pub struct t3 {
    pub s3: Vec<t4>,
    pub s4: PathBuf,
    pub s5: Option<PathBuf>,
    pub s7: Option<String>,
}

/// t4 = PlanStep. One action in the plan.
#[derive(Debug, Clone)]
pub struct t4 {
    pub s6: t5,
}

/// t5 = Action. What to run.
#[derive(Debug, Clone)]
pub enum t5 {
    CargoCheck,
    CargoBuild { release: bool },
    CargoTest,
    ApprouterUpdateTunnel,
    ApprouterSetupRoguerepo,
    Custom { cmd: String, args: Vec<String> },
}

impl t3 {
    /// f14 = plan_from_intent. Map intent to action DAG.
    pub fn f14(intent: &t0, project: PathBuf, approuter_dir: Option<PathBuf>) -> Self {
        let approuter_dir = approuter_dir.or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("approuter"))
        });
        let mut s3 = Vec::new();
        match &intent.s0 {
            t1::FullPipeline => {
                s3.push(t4 { s6: t5::CargoCheck });
                s3.push(t4 { s6: t5::CargoTest });
            }
            t1::Test => {
                s3.push(t4 { s6: t5::CargoTest });
            }
            t1::Compile {
                release,
                check_only,
            } => {
                if *check_only {
                    s3.push(t4 { s6: t5::CargoCheck });
                } else {
                    s3.push(t4 {
                        s6: t5::CargoBuild { release: *release },
                    });
                }
            }
            t1::TunnelUpdate => {
                if approuter_dir.is_some() {
                    s3.push(t4 {
                        s6: t5::ApprouterUpdateTunnel,
                    });
                }
            }
            t1::SetupRoguerepo => {
                if approuter_dir.is_some() {
                    s3.push(t4 {
                        s6: t5::ApprouterSetupRoguerepo,
                    });
                }
            }
            t1::Custom { cmd, args } => {
                s3.push(t4 {
                    s6: t5::Custom {
                        cmd: cmd.clone(),
                        args: args.clone(),
                    },
                });
            }
            t1::CloudflarePurge | t1::FixWarnings => {}
        }
        t3 {
            s3,
            s4: project,
            s5: approuter_dir,
            s7: intent.s1.clone(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{f62, t0, t1, t2 as Constraint};

    fn project() -> std::path::PathBuf { std::path::PathBuf::from("/tmp/test") }

    #[test]
    fn full_pipeline_produces_check_and_test() {
        let intent = t0 { s0: t1::FullPipeline, s1: None, s2: vec![] };
        let plan = t3::f14(&intent, project(), None);
        let actions: Vec<_> = plan.s3.iter().map(|s| &s.s6).collect();
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], t5::CargoCheck));
        assert!(matches!(actions[1], t5::CargoTest));
    }

    #[test]
    fn test_intent_produces_cargo_test() {
        let intent = t0 { s0: t1::Test, s1: None, s2: vec![] };
        let plan = t3::f14(&intent, project(), None);
        assert_eq!(plan.s3.len(), 1);
        assert!(matches!(plan.s3[0].s6, t5::CargoTest));
    }

    #[test]
    fn compile_debug_produces_cargo_build() {
        let intent = t0 { s0: t1::Compile { release: false, check_only: false }, s1: None, s2: vec![] };
        let plan = t3::f14(&intent, project(), None);
        assert_eq!(plan.s3.len(), 1);
        assert!(matches!(plan.s3[0].s6, t5::CargoBuild { release: false }));
    }

    #[test]
    fn compile_release_produces_cargo_build_release() {
        let intent = t0 { s0: t1::Compile { release: true, check_only: false }, s1: None, s2: vec![] };
        let plan = t3::f14(&intent, project(), None);
        assert!(matches!(plan.s3[0].s6, t5::CargoBuild { release: true }));
    }

    #[test]
    fn compile_check_only_produces_cargo_check() {
        let intent = t0 { s0: t1::Compile { release: false, check_only: true }, s1: None, s2: vec![] };
        let plan = t3::f14(&intent, project(), None);
        assert_eq!(plan.s3.len(), 1);
        assert!(matches!(plan.s3[0].s6, t5::CargoCheck));
    }

    #[test]
    fn project_hint_preserved() {
        let intent = t0 { s0: t1::Test, s1: Some("my_crate".into()), s2: vec![] };
        let plan = t3::f14(&intent, project(), None);
        assert_eq!(plan.s7.as_deref(), Some("my_crate"));
    }

    #[test]
    fn fix_warnings_produces_no_steps() {
        let intent = t0 { s0: t1::FixWarnings, s1: None, s2: vec![] };
        let plan = t3::f14(&intent, project(), None);
        assert!(plan.s3.is_empty());
    }

    #[test]
    fn custom_intent_passthrough() {
        let intent = t0 {
            s0: t1::Custom { cmd: "echo".into(), args: vec!["hi".into()] },
            s1: None,
            s2: vec![],
        };
        let plan = t3::f14(&intent, project(), None);
        assert_eq!(plan.s3.len(), 1);
        assert!(matches!(&plan.s3[0].s6, t5::Custom { cmd, .. } if cmd == "echo"));
    }
}
