//! Catalogue search and ranking (pure; no GTK, no I/O). Ported 1:1 from the
//! verified experiments/catalogue/search.js scorer so results match the HTML
//! viewer: exact attr/pname = 100, prefix = 30, contains = 10, description = 3.
use crate::model::SearchText;

/// Split on anything that is not [a-z0-9.+-], lowercase.
pub fn tokenize(q: &str) -> Vec<String> {
    q.to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-')))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Weighted score of one entry against the query terms (mirrors search.js).
pub fn score(t: &SearchText, terms: &[String]) -> i64 {
    let mut s = 0i64;
    for term in terms {
        if term.len() < 2 {
            continue;
        }
        if t.name == *term || t.pn == *term {
            s += 100;
        } else if t.name.starts_with(term) || t.pn.starts_with(term) {
            s += 30;
        } else if t.name.contains(term.as_str()) || t.pn.contains(term.as_str()) {
            s += 10;
        }
        if t.desc.contains(term.as_str()) {
            s += 3;
        }
    }
    s
}

/// Ranked indices of the matching entries (score desc), capped at `max`.
/// Empty for a blank query; the caller decides the browse behaviour.
pub fn rank(query: &str, texts: &[SearchText], max: usize) -> Vec<usize> {
    let terms = tokenize(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(i64, usize)> = texts
        .iter()
        .enumerate()
        .map(|(i, t)| (score(t, &terms), i))
        .filter(|(s, _)| *s > 0)
        .collect();
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    scored.into_iter().take(max).map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::IndexEntry;

    fn text(name: &str, pn: &str, desc: &str) -> SearchText {
        SearchText {
            name: name.to_string(),
            pn: pn.to_string(),
            desc: desc.to_string(),
        }
    }

    #[test]
    fn tokenize_splits_and_lowercases() {
        assert_eq!(
            tokenize("Firefox 160-esr +"),
            vec![
                "firefox".to_string(),
                "160-esr".to_string(),
                "+".to_string()
            ]
        );
        assert_eq!(tokenize("::"), Vec::<String>::new());
    }

    #[test]
    fn scoring_matches_search_js_rules() {
        let ff = text("firefox", "", "");
        assert_eq!(score(&ff, &["firefox".into()]), 100); // exact
        assert_eq!(score(&ff, &["fire".into()]), 30); // prefix
        assert_eq!(score(&ff, &["efo".into()]), 10); // contains
        assert_eq!(score(&ff, &["f".into()]), 0); // single-char terms skipped
        assert_eq!(score(&ff, &["firefox".into(), "efo".into()]), 110); // sums
        let htop = text("htop", "", "an interactive process viewer");
        assert_eq!(score(&htop, &["viewer".into()]), 3); // description bonus only
    }

    #[test]
    fn rank_orders_by_score_and_caps() {
        let texts = vec![
            text("firefox", "", ""),                      // exact -> 100
            text("firefox-bin", "", ""),                  // prefix -> 30
            text("something-else", "", "firefox helper"), // desc bonus -> 3
        ];
        assert_eq!(rank("firefox", &texts, 10), vec![0, 1, 2]);
        let capped = rank("firefox", &texts, 2);
        assert_eq!(capped.len(), 2);
    }

    #[test]
    fn blank_query_ranks_nothing() {
        assert!(rank("", &[text("firefox", "", "")], 10).is_empty());
    }

    #[test]
    fn rank_caps_hard_at_max_when_many_match() {
        let texts: Vec<SearchText> = (0..200)
            .map(|i| text(&format!("pkg-{i}"), "", "shared description"))
            .collect();
        let got = rank("shared", &texts, 60);
        assert_eq!(got.len(), 60);
        // top result is deterministic, not index-order:
        // all score equally (desc bonus only), then equal scores keep order
        assert_eq!(got, (0..60).collect::<Vec<_>>());
    }

    #[test]
    fn rank_no_match_and_single_char_return_empty() {
        let texts = vec![text("firefox", "", "web browser")];
        assert!(rank("zzz-no-such-term", &texts, 10).is_empty());
        // terms under 2 chars are skipped, so nothing ever scores
        assert!(rank("a", &texts, 10).is_empty());
        assert!(rank("   ", &texts, 10).is_empty());
    }

    #[test]
    fn search_text_is_prelowered() {
        let e = IndexEntry {
            attr: "Firefox".into(),
            pname: Some("Firefox".into()),
            version: None,
            description: Some("Web Browser".into()),
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: crate::model::Class::App,
            tier: crate::model::Tier::Basic,
            install: None,
        };
        let t = SearchText::build(&e);
        assert_eq!(t.name, "firefox");
        assert_eq!(t.desc, "web browser");
    }
}
