//! Small fzf-style matcher for the directory picker.
//!
//! Space-separated terms must all match as subsequences. Matches score higher
//! when characters are consecutive, start a path segment, or land in the last
//! segment, so `herdr plug` prefers `~/src/herdr-plugin` over deep paths that
//! merely contain those letters.

const SEPARATORS: &[char] = &['/', '-', '_', '.', ' '];

/// Indices of `candidates` that match `query`, best first; ties keep input order.
pub fn filter<S: AsRef<str>>(query: &str, candidates: &[S]) -> Vec<usize> {
    let mut scored: Vec<(usize, i64)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| score(query, c.as_ref()).map(|s| (i, s)))
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, _)| i).collect()
}

pub fn score(query: &str, candidate: &str) -> Option<i64> {
    query
        .split_whitespace()
        .try_fold(0, |total, term| Some(total + score_term(term, candidate)?))
}

fn score_term(term: &str, candidate: &str) -> Option<i64> {
    // Smart case: an uppercase letter in the term makes it case-sensitive.
    let case_sensitive = term.chars().any(char::is_uppercase);
    let fold = |c: char| {
        if case_sensitive {
            c
        } else {
            c.to_ascii_lowercase()
        }
    };
    let chars: Vec<char> = candidate.chars().collect();
    let basename_start = candidate
        .trim_end_matches('/')
        .rfind('/')
        .map_or(0, |i| candidate[..=i].chars().count());

    let mut total = 0;
    let mut pos = 0;
    let mut previous: Option<usize> = None;
    for wanted in term.chars().map(fold) {
        let offset = chars[pos..].iter().position(|&c| fold(c) == wanted)?;
        let at = pos + offset;
        total += 1;
        if previous.is_some_and(|p| p + 1 == at) {
            total += 5;
        } else if let Some(p) = previous {
            total -= ((at - p) as i64).min(5);
        }
        if at == 0 || SEPARATORS.contains(&chars[at - 1]) {
            total += 8;
        }
        if at >= basename_start {
            total += 2;
        }
        previous = Some(at);
        pos = at + 1;
    }
    Some(total)
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
    fn requires_subsequence() {
        assert!(score("hrd", "/src/herdr").is_some());
        assert!(score("xyz", "/src/herdr").is_none());
    }

    #[test]
    fn all_terms_must_match() {
        assert!(score("src herdr", "/Users/me/src/herdr").is_some());
        assert!(score("src nope", "/Users/me/src/herdr").is_none());
    }

    #[test]
    fn basename_and_contiguous_matches_rank_first() {
        let items = [
            "/Users/me/hidden/e/r/d/r/deep",
            "/Users/me/src/herdr-plugin",
            "/Users/me/src/herdr-plugin-2",
        ];
        assert_eq!(filter("herdr", &items), [1, 2, 0]);
    }

    #[test]
    fn smart_case() {
        assert!(score("src", "/SRC").is_some());
        assert!(score("SRC", "/src").is_none());
    }
}
