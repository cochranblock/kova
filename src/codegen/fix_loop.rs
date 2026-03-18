// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Shared fix loop logic. Categorize errors, call fixer model, return fixed code.
//! Used by pipeline and strategies. One copy.

use std::path::Path;

use crate::pipeline::f296;

/// Fix and retry: f118 stderr, feed context to fixer, return fixed code.
/// Delegates to local Kalosm inference.
pub async fn fix_and_retry_local(
    fix_path: &Path,
    project_dir: &Path,
    stage: &str,
    stderr: &str,
    code: &str,
) -> Result<String, String> {
    let error_block = f296(stage, stderr);
    let fix_prompt = format!(
        "Fix this Rust code. {}\n\nCode:\n```rust\n{}\n```\n\nReply with only the fixed code in ```rust ... ```.",
        error_block, code
    );
    let cursor = crate::cursor_prompts::f111(project_dir);
    let fix_system = if cursor.is_empty() {
        format!("You fix Rust {} errors.", stage)
    } else {
        format!(
            "You fix Rust {} errors.\n\n--- Cursor rules ---\n{}",
            stage, cursor
        )
    };
    crate::inference::f80(fix_path, &fix_system, &fix_prompt)
        .await
        .map_err(|e| format!("Error on fix: {}", e))
}

/// Fix and retry via cluster dispatch. For factory/moe/academy.
pub fn f331(
    cluster: &crate::inference::cluster::T193,
    system: &str,
    code: &str,
    error: &str,
    stuck: bool,
    prev_errors: &[String],
    num_ctx: u32,
) -> Result<String, String> {
    use crate::inference::cluster::T191;

    let fix_prompt = if stuck {
        format!(
            "IMPORTANT: Your previous fix attempt did NOT resolve this error. The same error occurred again.\n\
            You must use a DIFFERENT approach this time.\n\n\
            The compiler error is:\n```\n{}\n```\n\n\
            Previous error history ({} attempts):\n{}\n\n\
            Current code:\n```rust\n{}\n```\n\n\
            Think carefully about the root cause. The error type and line number are exact.\n\
            Return ONLY the complete fixed code in a ```rust block.",
            error,
            prev_errors.len(),
            prev_errors.iter().enumerate()
                .map(|(i, e)| format!("  attempt {}: {}", i + 1, crate::cargo::f308(e, 100)))
                .collect::<Vec<_>>().join("\n"),
            code
        )
    } else {
        format!(
            "Fix this Rust code. The compiler error is:\n```\n{}\n```\n\n\
            Code:\n```rust\n{}\n```\n\n\
            Return ONLY the complete fixed code in a ```rust block. No explanation.",
            error, code
        )
    };

    let dispatch_result = if stuck {
        cluster.speculative_dispatch(
            T191::FixCompile,
            system,
            &fix_prompt,
            Some(num_ctx),
        )
    } else {
        cluster.dispatch(
            T191::FixCompile,
            system,
            &fix_prompt,
            Some(num_ctx),
        )
    };

    dispatch_result.map(|(_, response)| {
        crate::cargo::f309(&response).unwrap_or(response)
    })
}
