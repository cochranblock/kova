// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Categorize cargo check stderr for agentic fix loop. f118=f118, t95=T95.

/// t95=T95. Error category for Mechanic (Fixer) model context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T95 {
    Syntax,
    BorrowChecker,
    Lifetime,
    Type,
    Other,
}

/// f118=f118. Categorize stderr from cargo check. Feeds specific context to the Mechanic.
pub fn f118(stderr: &str) -> T95 {
    let lower = stderr.to_lowercase();
    if lower.contains("expected one of")
        || lower.contains("unexpected end of")
        || lower.contains("expected `;`")
        || lower.contains("expected `,`")
        || lower.contains("expected `)`")
        || lower.contains("expected `{`")
        || lower.contains("missing semicolon")
        || lower.contains("unclosed delimiter")
        || lower.contains("expected expression")
    {
        return T95::Syntax;
    }
    if lower.contains("borrow")
        || lower.contains("cannot borrow")
        || lower.contains("borrowed value")
        || lower.contains("move")
        || lower.contains("use of moved value")
    {
        return T95::BorrowChecker;
    }
    if lower.contains("lifetime")
        || lower.contains("outlives")
        || lower.contains("does not live long enough")
        || lower.contains("'static")
    {
        return T95::Lifetime;
    }
    if lower.contains("expected") && (lower.contains("type") || lower.contains("found"))
        || lower.contains("mismatched types")
        || lower.contains("cannot infer")
    {
        return T95::Type;
    }
    T95::Other
}

fn context_hint(kind: T95) -> &'static str {
    match kind {
        T95::Syntax => "Fix the syntax error (missing semicolon, delimiter, etc).",
        T95::BorrowChecker => "Fix the borrow checker error (ownership, mutability).",
        T95::Lifetime => "Fix the lifetime error (references, scope).",
        T95::Type => "Fix the type mismatch.",
        T95::Other => "Fix the compilation error.",
    }
}

/// Build error block with categorized context for the Mechanic.
pub fn f296(stage: &str, stderr: &str) -> String {
    let kind = f118(stderr);
    let hint = context_hint(kind);
    format!(
        "{} error (category: {:?}). {}\n\nStderr:\n```\n{}\n```",
        stage, kind, hint, stderr
    )
}
