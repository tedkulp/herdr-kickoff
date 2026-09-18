//! Directory filter matching `zoxide query -i`, which runs fzf with
//! `--exact --no-sort`: every space-separated term must appear as a substring,
//! and matches keep zoxide's frecency order rather than being re-ranked.

/// Indices of `candidates` that match `query`, in their original order.
pub fn filter<S: AsRef<str>>(query: &str, candidates: &[S]) -> Vec<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| matches(query, c.as_ref()))
        .map(|(i, _)| i)
        .collect()
}

pub fn matches(query: &str, candidate: &str) -> bool {
    let folded = candidate.to_lowercase();
    query.split_whitespace().all(|term| {
        // Smart case, like fzf: an uppercase letter makes the term case-sensitive.
        if term.chars().any(char::is_uppercase) {
            candidate.contains(term)
        } else {
            folded.contains(term)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything_in_order() {
        let items = ["/a", "/b", "/c"];
        assert_eq!(filter("", &items), [0, 1, 2]);
    }

    #[test]
    fn terms_are_substrings_not_subsequences() {
        let items = [
            "/Users/me/src/napsack-opus4",
            "/Users/me/src/drops",
            "/Users/me/src/old/playbooks",
        ];
        assert_eq!(filter("drops", &items), [1]);
    }

    #[test]
    fn all_terms_must_match_and_order_is_preserved() {
        let items = [
            "/me/src/herdr-plugin-2",
            "/me/notes",
            "/me/src/herdr-plugin",
        ];
        assert_eq!(filter("src herdr", &items), [0, 2]);
        assert!(filter("src nope", &items).is_empty());
    }

    #[test]
    fn smart_case() {
        assert!(matches("src", "/SRC"));
        assert!(!matches("SRC", "/src"));
    }
}
