//! Deterministic token estimation for context budgeting.
//!
//! The compiler must not require a generative model or a tokenizer tied to a
//! specific provider, so budgeting uses the common ~4 characters/token
//! heuristic. Good enough for budget enforcement; not for billing.

pub fn estimate_tokens(text: &str) -> i64 {
    (text.chars().count() as u64).div_ceil(4) as i64
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn estimates_are_conservative_and_stable() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }
}
