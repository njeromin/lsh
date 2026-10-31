use super::types::CompletionItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    pub score: i64,
    pub indices: Vec<usize>,
}

fn is_word_boundary(cand: &[char], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    let prev = cand[idx - 1];
    prev == '_'
        || prev == '-'
        || prev == '.'
        || prev == '/'
        || prev == '\\'
        || prev == ' '
        || prev == ':'
        || (!prev.is_uppercase() && cand[idx].is_uppercase())
}

pub fn fuzzy_match(candidate: &str, query: &str) -> Option<FuzzyMatch> {
    if query.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            indices: Vec::new(),
        });
    }

    let cand_chars: Vec<char> = candidate.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();

    if query_chars.len() > cand_chars.len() {
        return None;
    }

    let mut indices: Vec<usize> = Vec::with_capacity(query_chars.len());
    let mut search_from = 0;

    for &qc in &query_chars {
        let mut best_pos = None;

        for pos in search_from..cand_chars.len() {
            if cand_chars[pos].to_lowercase().eq(qc.to_lowercase()) {
                // Consecutive match to previous index takes highest priority
                if indices.last().is_some_and(|&prev| pos == prev + 1) {
                    best_pos = Some(pos);
                    break;
                }
                // Word boundary match takes priority over non-boundary match
                let is_boundary = is_word_boundary(&cand_chars, pos);
                if is_boundary {
                    best_pos = Some(pos);
                    break;
                } else if best_pos.is_none() {
                    best_pos = Some(pos);
                }
            }
        }

        match best_pos {
            Some(pos) => {
                indices.push(pos);
                search_from = pos + 1;
            }
            None => return None,
        }
    }

    let mut score: i64 = 0;
    let mut prev_idx: Option<usize> = None;

    for (q_idx, &c_idx) in indices.iter().enumerate() {
        score += 10;

        if c_idx == 0 {
            score += 35;
        } else if is_word_boundary(&cand_chars, c_idx) {
            score += 25;
        }

        if let Some(prev) = prev_idx {
            if c_idx == prev + 1 {
                score += 15;
            } else {
                let gap = c_idx - prev - 1;
                score -= (gap as i64).min(10);
            }
        }

        if cand_chars[c_idx] == query_chars[q_idx] {
            score += 5;
        }

        prev_idx = Some(c_idx);
    }

    score -= (cand_chars.len() as i64).min(20);

    Some(FuzzyMatch { score, indices })
}

pub fn filter_completion_items(items: &[CompletionItem], query: &str) -> Vec<CompletionItem> {
    if query.is_empty() {
        return items
            .iter()
            .map(|it| {
                let mut c = it.clone();
                c.match_indices.clear();
                c
            })
            .collect();
    }

    let mut scored: Vec<(CompletionItem, i64)> = items
        .iter()
        .filter_map(|it| {
            if let Some(m) = fuzzy_match(&it.display, query) {
                let mut item = it.clone();
                item.match_indices = m.indices;
                Some((item, m.score))
            } else if let Some(m) = fuzzy_match(&it.replacement, query) {
                let mut item = it.clone();
                item.match_indices = Vec::new();
                Some((item, m.score))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by_key(|b| std::cmp::Reverse(b.1));

    scored.into_iter().map(|(item, _)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::menu::types::{CompletionItem, CompletionKind};

    #[test]
    fn fuzzy_match_detects_subsequences_and_word_boundaries() {
        // "ct" matches Cargo.toml ('C' at 0, 't' at 6)
        let ct_cargo = fuzzy_match("Cargo.toml", "ct").unwrap();
        assert_eq!(ct_cargo.indices, vec![0, 6]);

        // "ct" matches cat ('c' at 0, 't' at 2)
        let ct_cat = fuzzy_match("cat", "ct").unwrap();
        assert_eq!(ct_cat.indices, vec![0, 2]);

        // "dco" matches docker-compose ('d' at 0, 'c' at 7, 'o' at 8)
        let dco = fuzzy_match("docker-compose", "dco").unwrap();
        assert_eq!(dco.indices, vec![0, 7, 8]);

        // "mrs" matches menu.rs ('m' at 0, 'r' at 5, 's' at 6)
        let mrs = fuzzy_match("menu.rs", "mrs").unwrap();
        assert_eq!(mrs.indices, vec![0, 5, 6]);

        // Non-matching query
        assert!(fuzzy_match("cargo", "xyz").is_none());
    }

    #[test]
    fn fuzzy_match_ranks_exact_prefix_higher_than_subsequence() {
        let prefix_match = fuzzy_match("cargo", "car").unwrap();
        let subseq_match = fuzzy_match("clear", "car").unwrap();
        assert!(prefix_match.score > subseq_match.score);
    }

    #[test]
    fn filter_completion_items_narrows_candidates_dynamically() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cat", "cat", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        // Empty query keeps all items
        assert_eq!(filter_completion_items(&items, "").len(), 3);

        // "ca" keeps cargo and cat (cat scores higher due to shorter length)
        let ca_matches = filter_completion_items(&items, "ca");
        assert_eq!(ca_matches.len(), 2);
        assert_eq!(ca_matches[0].replacement, "cat");
        assert_eq!(ca_matches[1].replacement, "cargo");

        // "car" keeps cargo
        let car_matches = filter_completion_items(&items, "car");
        assert_eq!(car_matches.len(), 1);
        assert_eq!(car_matches[0].replacement, "cargo");

        // "card" matches nothing
        assert_eq!(filter_completion_items(&items, "card").len(), 0);

        // Case-insensitive query matches
        let upper_matches = filter_completion_items(&items, "CD");
        assert_eq!(upper_matches.len(), 1);
        assert_eq!(upper_matches[0].replacement, "cd");
    }

    #[test]
    fn filter_completion_items_supports_fuzzy_subsequences() {
        let items = vec![
            CompletionItem::new("Cargo.toml", "Cargo.toml", CompletionKind::File),
            CompletionItem::new("cat", "cat", CompletionKind::Binary),
            CompletionItem::new("docker-compose", "docker-compose", CompletionKind::Binary),
        ];

        // "ct" matches both Cargo.toml and cat
        let ct_matches = filter_completion_items(&items, "ct");
        assert_eq!(ct_matches.len(), 2);
        assert_eq!(ct_matches[0].match_indices.len(), 2);

        // "dco" matches docker-compose
        let dco_matches = filter_completion_items(&items, "dco");
        assert_eq!(dco_matches.len(), 1);
        assert_eq!(dco_matches[0].display, "docker-compose");
        assert_eq!(dco_matches[0].match_indices, vec![0, 7, 8]);
    }
}
