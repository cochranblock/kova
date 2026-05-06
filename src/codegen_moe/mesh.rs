// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! mesh — Sponge Mesh correction layer for pyramid inference.
//!
//! No known prior art for error-aware retry mesh in hierarchical MoE inference.
//!
//! Pattern:
//!   1. Dispatch inference call to expert
//!   2. If output fails validation (compile error, empty, malformed):
//!      a. Collect the error
//!      b. Re-dispatch with error context appended to prompt
//!      c. Exponential backoff between retries
//!   3. After max_retries: escalate (return Exhausted, assembler handles)
//!
//! The mesh absorbs inference failures like a sponge — soaks them up,
//! wrings them out on retry with error context. The expert learns from
//! its own failure within the same inference chain.
//!
//! This is NOT training-time load balancing (Switch Transformer / Expert Choice).
//! This is runtime inference correction — the expert produced bad output,
//! retry with "you produced X which failed because Y, try again."

use super::LayerOutput;
use std::time::{Duration, SystemTime};

/// Result from a Sponge Mesh dispatch.
#[derive(Debug)]
pub enum MeshResult {
    /// Expert produced valid output (possibly after retries).
    Success(LayerOutput),
    /// Expert failed after all retries.
    Exhausted {
        retries: u32,
        last_error: String,
    },
}

/// Sponge Mesh — error-aware retry with exponential backoff for inference.
pub struct SpongeMesh {
    max_retries: u32,
    base_backoff: Duration,
}

impl SpongeMesh {
    pub fn new(max_retries: u32, base_backoff: Duration) -> Self {
        Self {
            max_retries,
            base_backoff,
        }
    }

    /// Add jitter to backoff so parallel experts don't retry in lockstep.
    /// Uses system time nanos as cheap entropy — no rand crate needed.
    fn jittered_backoff(&self, retry: u32) -> Duration {
        let base = self.base_backoff * retry;
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        // Jitter: ±25% of base backoff
        let jitter_ms = (nanos % (base.as_millis().max(1) as u32 / 2)) as u64;
        if nanos % 2 == 0 {
            base + Duration::from_millis(jitter_ms)
        } else {
            base.saturating_sub(Duration::from_millis(jitter_ms))
        }
    }

    /// Dispatch an inference call with Sponge Mesh retry.
    /// The closure produces a LayerOutput. If the output indicates failure
    /// (compile error, empty code), we retry with the error context.
    pub fn dispatch<F>(&self, f: F) -> MeshResult
    where
        F: Fn() -> LayerOutput,
    {
        let mut last_output = f();
        let mut retries = 0;

        while retries < self.max_retries {
            // Check if output is valid
            if self.validate(&last_output) {
                last_output.retry_count = retries;
                return MeshResult::Success(last_output);
            }

            retries += 1;
            let backoff = self.jittered_backoff(retries);

            eprintln!(
                "[mesh] retry {}/{} for {} — backoff {}ms (jittered) — error: {}",
                retries,
                self.max_retries,
                last_output.source_expert,
                backoff.as_millis(),
                last_output
                    .error_output
                    .as_deref()
                    .unwrap_or("empty/malformed output")
            );

            std::thread::sleep(backoff);

            // Retry — the closure runs again
            // TODO: pass error context back into the prompt for the retry
            // so the expert can correct based on its own failure
            last_output = f();
        }

        // Final check after last retry
        if self.validate(&last_output) {
            last_output.retry_count = retries;
            return MeshResult::Success(last_output);
        }

        MeshResult::Exhausted {
            retries,
            last_error: last_output
                .error_output
                .unwrap_or_else(|| "empty output after all retries".to_string()),
        }
    }

    /// Dispatch with error context fed back into each retry.
    /// This is the full Sponge Mesh — the expert sees its own failure.
    /// Flywheel: successful retries save (bad, error, good) training pairs.
    pub fn dispatch_with_context<F>(&self, f: F) -> MeshResult
    where
        F: Fn(Option<&str>) -> LayerOutput,
    {
        let mut last_output = f(None);
        let mut retries = 0;
        let mut prev_bad_code: Option<String> = None;
        let mut prev_error: Option<String> = None;

        while retries < self.max_retries {
            if self.validate(&last_output) {
                last_output.retry_count = retries;

                // Flywheel: if this was a retry, save the correction as a training pair
                if retries > 0 {
                    if let (Some(bad), Some(err)) = (&prev_bad_code, &prev_error) {
                        if let Ok(kind) = last_output.source_expert.parse::<String>() {
                            super::compiler_teacher::save_pair(
                                bad,
                                err,
                                &last_output.code,
                                &expert_kind_from_str(&kind),
                            );
                        }
                    }
                }

                return MeshResult::Success(last_output);
            }

            // Save the bad output before retrying
            prev_bad_code = Some(last_output.code.clone());
            prev_error = last_output.error_output.clone();

            retries += 1;
            let backoff = self.jittered_backoff(retries);

            let error_ctx = last_output.error_output.as_deref()
                .unwrap_or("output was empty or malformed");

            eprintln!(
                "[mesh] retry {}/{} for {} — backoff {}ms (jittered) — feeding error back — {}",
                retries,
                self.max_retries,
                last_output.source_expert,
                backoff.as_millis(),
                truncate(error_ctx, 80)
            );

            std::thread::sleep(backoff);

            last_output = f(Some(error_ctx));
        }

        if self.validate(&last_output) {
            last_output.retry_count = retries;

            // Flywheel: save the final successful correction
            if let (Some(bad), Some(err)) = (&prev_bad_code, &prev_error) {
                super::compiler_teacher::save_pair(
                    bad,
                    err,
                    &last_output.code,
                    &expert_kind_from_str(&last_output.source_expert),
                );
            }

            return MeshResult::Success(last_output);
        }

        MeshResult::Exhausted {
            retries,
            last_error: last_output
                .error_output
                .unwrap_or_else(|| "exhausted after all retries".to_string()),
        }
    }

    /// Validate an expert's output. Returns true if acceptable.
    fn validate(&self, output: &LayerOutput) -> bool {
        // Empty output = failure
        if output.code.trim().is_empty() {
            return false;
        }

        // Error comment only = failure
        if output.code.starts_with("// ") && output.code.contains("error:") && output.code.lines().count() <= 2 {
            return false;
        }

        // If compile was attempted and failed = failure
        if output.compile_ok == Some(false) {
            return false;
        }

        // Passed all checks
        true
    }
}

/// Map expert name string back to ExpertKind. Default to RustBoilerplate for unknowns.
fn expert_kind_from_str(s: &str) -> super::ExpertKind {
    use super::ExpertKind::*;
    match s {
        "CargoToml" => CargoToml,
        "ClaudeConfig" => ClaudeConfig,
        "StructDef" => StructDef,
        "EnumDef" => EnumDef,
        "ThiserrorEnum" => ThiserrorEnum,
        "AxumRouter" => AxumRouter,
        "AxumHandler" => AxumHandler,
        "ClapParser" => ClapParser,
        "ClapSubcommand" => ClapSubcommand,
        "SledRead" => SledRead,
        "SledWrite" => SledWrite,
        "BincodeSerialize" => BincodeSerialize,
        "ZstdCompress" => ZstdCompress,
        "ReqwestClient" => ReqwestClient,
        "CandleModelLoad" => CandleModelLoad,
        "CandleForward" => CandleForward,
        "NanosignVerify" => NanosignVerify,
        "TestUnit" => TestUnit,
        "TestIntegration" => TestIntegration,
        "ExopackGate" => ExopackGate,
        _ => RustBoilerplate,
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
