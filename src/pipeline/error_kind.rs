//! Categorize cargo check stderr for agentic fix loop. f118=f118, t95=T95.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f118_syntax_keywords() {
        assert_eq!(f118("expected one of `{`, `;`, or `(`"), T95::Syntax);
        assert_eq!(f118("unexpected end of file"), T95::Syntax);
        assert_eq!(f118("expected `;`"), T95::Syntax);
        assert_eq!(f118("missing semicolon"), T95::Syntax);
        assert_eq!(f118("unclosed delimiter"), T95::Syntax);
        assert_eq!(f118("expected expression, found `}`"), T95::Syntax);
    }

    #[test]
    fn f118_borrow_checker_keywords() {
        assert_eq!(f118("cannot borrow `x` as mutable"), T95::BorrowChecker);
        assert_eq!(f118("borrowed value does not live long enough"), T95::BorrowChecker);
        assert_eq!(f118("use of moved value: `v`"), T95::BorrowChecker);
        assert_eq!(f118("value moved here"), T95::BorrowChecker);
    }

    #[test]
    fn f118_lifetime_keywords() {
        assert_eq!(f118("lifetime `'a` does not match"), T95::Lifetime);
        assert_eq!(f118("reference outlives the data it points to"), T95::Lifetime);
        assert_eq!(f118("does not live long enough"), T95::Lifetime);
    }

    #[test]
    fn f118_type_keywords() {
        assert_eq!(f118("mismatched types"), T95::Type);
        assert_eq!(f118("cannot infer type for variable"), T95::Type);
        assert_eq!(f118("expected type `u32`, found `i32`"), T95::Type);
    }

    #[test]
    fn f118_other_fallback() {
        assert_eq!(f118("error[E0599]: no method named `foo` found"), T95::Other);
        assert_eq!(f118("unresolved import"), T95::Other);
        assert_eq!(f118(""), T95::Other);
    }

    #[test]
    fn f118_case_insensitive() {
        assert_eq!(f118("EXPECTED ONE OF"), T95::Syntax);
        assert_eq!(f118("CANNOT BORROW"), T95::BorrowChecker);
        assert_eq!(f118("LIFETIME"), T95::Lifetime);
        assert_eq!(f118("MISMATCHED TYPES"), T95::Type);
    }

    #[test]
    fn f296_includes_stage_and_stderr() {
        let out = f296("compile", "mismatched types: expected u32, found i32");
        assert!(out.contains("compile error"));
        assert!(out.contains("Type"));
        assert!(out.contains("mismatched types: expected u32, found i32"));
        assert!(out.contains("Fix the type mismatch."));
    }

    #[test]
    fn f296_syntax_block_structure() {
        let out = f296("check", "expected `;`");
        assert!(out.starts_with("check error"));
        assert!(out.contains("Syntax"));
        assert!(out.contains("```"));
    }
}