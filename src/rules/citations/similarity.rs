use std::collections::{HashMap, HashSet};

use crate::diagnostic::{Diagnostic, Severity};

use super::BibEntry;

const MIN_TITLE_LEN: usize = 24;
const MIN_LENGTH_RATIO_PERCENT: usize = 85;
const MAX_EDIT_DISTANCE: usize = 4;
const MAX_BUCKET_SIZE: usize = 64;

pub(super) fn find_similar_titles(entries: &[&BibEntry]) -> Vec<Diagnostic> {
    let titled_entries = unique_titled_entries(entries);
    let mut groups = similar_title_groups(&titled_entries);
    groups.sort_by_key(|group| group.iter().map(|index| titled_entries[*index].line).min());

    groups
        .iter()
        .map(|group| diagnostic_for_group(group, &titled_entries))
        .collect()
}

#[derive(Debug)]
struct TitledEntry<'a> {
    entry: &'a BibEntry,
    normalized_title: String,
    tokens: Vec<String>,
    numeric_tokens: Vec<String>,
    signature_tokens: Vec<String>,
    display_title: String,
    line: usize,
}

impl TitledEntry<'_> {
    fn key(&self) -> &str {
        &self.entry.key
    }
}

fn unique_titled_entries<'a>(entries: &[&'a BibEntry]) -> Vec<TitledEntry<'a>> {
    let mut seen = HashSet::new();
    let mut titled_entries = Vec::new();

    for entry in entries {
        let Some(normalized_title) = normalized_title(entry) else {
            continue;
        };
        if !seen.insert((entry.key.as_str(), normalized_title.clone())) {
            continue;
        }

        let tokens = title_tokens(&normalized_title);
        let numeric_tokens = numeric_tokens(&tokens);
        let signature_tokens = signature_tokens(&tokens);

        titled_entries.push(TitledEntry {
            entry,
            display_title: display_title(entry),
            line: entry.line,
            normalized_title,
            tokens,
            numeric_tokens,
            signature_tokens,
        });
    }

    titled_entries
}

fn similar_title_groups(entries: &[TitledEntry]) -> Vec<Vec<usize>> {
    let mut parent: Vec<_> = (0..entries.len()).collect();
    for (left_index, right_index) in candidate_pairs(entries) {
        let left = &entries[left_index];
        let right = &entries[right_index];
        if left.key() == right.key() {
            continue;
        }
        if entries_are_similar(left, right) {
            union(&mut parent, left_index, right_index);
        }
    }

    let mut grouped = HashMap::<usize, Vec<usize>>::new();
    for index in 0..entries.len() {
        let root = find(&mut parent, index);
        grouped.entry(root).or_default().push(index);
    }

    grouped
        .into_values()
        .filter(|group| distinct_key_count(group, entries) > 1)
        .collect()
}

fn candidate_pairs(entries: &[TitledEntry]) -> Vec<(usize, usize)> {
    let mut buckets = HashMap::<&str, Vec<usize>>::new();

    for (index, entry) in entries.iter().enumerate() {
        for token in &entry.signature_tokens {
            buckets.entry(token.as_str()).or_default().push(index);
        }
    }

    let mut seen_pairs = HashSet::new();
    let mut pairs = Vec::new();
    for bucket in buckets.values() {
        if bucket.len() < 2 || bucket.len() > MAX_BUCKET_SIZE {
            continue;
        }

        for (left_position, left_index) in bucket.iter().enumerate() {
            for right_index in bucket.iter().skip(left_position + 1) {
                let pair = ordered_pair(*left_index, *right_index);
                if seen_pairs.insert(pair) {
                    pairs.push(pair);
                }
            }
        }
    }

    pairs
}

fn ordered_pair(left: usize, right: usize) -> (usize, usize) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn diagnostic_for_group(group: &[usize], entries: &[TitledEntry]) -> Diagnostic {
    let anchor_index = *group
        .iter()
        .max_by_key(|index| entries[**index].line)
        .expect("similar title group cannot be empty");
    let anchor = entries[anchor_index].entry;
    let mut keys: Vec<_> = group.iter().map(|index| entries[*index].key()).collect();
    keys.sort_unstable();
    keys.dedup();

    let title = group
        .iter()
        .map(|index| entries[*index].display_title.as_str())
        .find(|title| !title.is_empty())
        .unwrap_or("");

    Diagnostic::new(
        "CIT006",
        Severity::Warning,
        format!(
            "bibliography entries have very similar titles under different keys: {}",
            quoted_list(&keys)
        ),
        &anchor.file,
        anchor.line,
        anchor.column,
    )
    .with_hint(format!(
        "compare entries for duplicate paper metadata: '{title}'"
    ))
}

fn distinct_key_count(group: &[usize], entries: &[TitledEntry]) -> usize {
    group
        .iter()
        .map(|index| entries[*index].key())
        .collect::<HashSet<_>>()
        .len()
}

fn quoted_list(keys: &[&str]) -> String {
    const LIMIT: usize = 6;
    let mut parts: Vec<_> = keys
        .iter()
        .take(LIMIT)
        .map(|key| format!("'{key}'"))
        .collect();
    if keys.len() > LIMIT {
        parts.push(format!("and {} more", keys.len() - LIMIT));
    }
    parts.join(", ")
}

fn find(parent: &mut [usize], index: usize) -> usize {
    if parent[index] != index {
        parent[index] = find(parent, parent[index]);
    }
    parent[index]
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root != right_root {
        parent[right_root] = left_root;
    }
}

fn normalized_title(entry: &BibEntry) -> Option<String> {
    let title = entry.fields.get("title")?;
    let mut normalized = String::new();
    let mut previous_was_space = false;

    for character in title.chars() {
        if character == '\\' || character == '{' || character == '}' {
            continue;
        }

        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_was_space = false;
        } else if (character.is_whitespace()
            || matches!(character, '-' | '_' | ':' | ';' | ',' | '.'))
            && !previous_was_space
            && !normalized.is_empty()
        {
            normalized.push(' ');
            previous_was_space = true;
        }
    }

    let normalized = normalized.trim().to_string();
    if normalized.len() < MIN_TITLE_LEN {
        None
    } else {
        Some(normalized)
    }
}

fn title_tokens(title: &str) -> Vec<String> {
    title
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn numeric_tokens(tokens: &[String]) -> Vec<String> {
    let mut numeric_tokens: Vec<_> = tokens
        .iter()
        .filter(|token| token.chars().any(|character| character.is_ascii_digit()))
        .cloned()
        .collect();
    numeric_tokens.sort();
    numeric_tokens.dedup();
    numeric_tokens
}

fn signature_tokens(tokens: &[String]) -> Vec<String> {
    let mut signature_tokens = Vec::new();
    let mut seen = HashSet::new();

    for token in tokens {
        if is_weak_signature_token(token) || !seen.insert(token.as_str()) {
            continue;
        }

        signature_tokens.push(token.clone());
        if signature_tokens.len() >= 10 {
            break;
        }
    }

    signature_tokens
}

fn is_weak_signature_token(token: &str) -> bool {
    token.len() < 4
        || matches!(
            token,
            "about"
                | "after"
                | "also"
                | "analysis"
                | "based"
                | "between"
                | "data"
                | "deep"
                | "from"
                | "into"
                | "large"
                | "learning"
                | "model"
                | "models"
                | "neural"
                | "paper"
                | "using"
                | "with"
        )
}

fn display_title(entry: &BibEntry) -> String {
    let Some(title) = entry.fields.get("title") else {
        return String::new();
    };

    let mut display = String::new();
    let mut previous_was_space = false;
    for character in title.chars() {
        if character == '{' || character == '}' || character == '\\' {
            continue;
        }

        let next;
        if character.is_whitespace() {
            next = if !previous_was_space && !display.is_empty() {
                Some(' ')
            } else {
                None
            };
            previous_was_space = true;
        } else {
            previous_was_space = false;
            next = Some(character);
        }

        if let Some(next) = next {
            if display.len() + next.len_utf8() > 120 {
                display.push_str("...");
                break;
            }
            display.push(next);
        }
    }

    display.trim().to_string()
}

fn entries_are_similar(left: &TitledEntry, right: &TitledEntry) -> bool {
    if is_publication_url_companion_pair(left.entry, right.entry) {
        return false;
    }

    if left.normalized_title == right.normalized_title {
        return true;
    }

    if left.numeric_tokens != right.numeric_tokens {
        return false;
    }

    if !token_sets_are_close(left, right) {
        return false;
    }

    titles_are_similar(&left.normalized_title, &right.normalized_title)
}

fn is_publication_url_companion_pair(left: &BibEntry, right: &BibEntry) -> bool {
    (is_url_companion_entry(left) && is_publication_entry(right))
        || (is_url_companion_entry(right) && is_publication_entry(left))
}

fn is_url_companion_entry(entry: &BibEntry) -> bool {
    let key = entry.key.to_ascii_lowercase();
    if key.ends_with("_url")
        || key.ends_with("-url")
        || key.contains("github")
        || key.contains("software")
    {
        return true;
    }

    entry.entry_type == "misc"
        && has_any_field(entry, &["url", "howpublished"])
        && !has_any_field(
            entry,
            &[
                "journal",
                "booktitle",
                "eprint",
                "archiveprefix",
                "doi",
                "publisher",
            ],
        )
}

fn is_publication_entry(entry: &BibEntry) -> bool {
    matches!(
        entry.entry_type.as_str(),
        "article" | "inproceedings" | "conference" | "incollection"
    ) || has_any_field(
        entry,
        &["journal", "booktitle", "eprint", "archiveprefix", "doi"],
    )
}

fn has_any_field(entry: &BibEntry, fields: &[&str]) -> bool {
    fields.iter().any(|field| entry.fields.contains_key(*field))
}

fn token_sets_are_close(left: &TitledEntry, right: &TitledEntry) -> bool {
    let left_tokens = left.tokens.iter().collect::<HashSet<_>>();
    let right_tokens = right.tokens.iter().collect::<HashSet<_>>();
    let intersection = left_tokens.intersection(&right_tokens).count();
    let union = left_tokens.union(&right_tokens).count();

    union > 0 && intersection * 100 / union >= 80
}

fn titles_are_similar(left: &str, right: &str) -> bool {
    let shorter = left.len().min(right.len());
    let longer = left.len().max(right.len());
    if shorter < MIN_TITLE_LEN || shorter * 100 / longer < MIN_LENGTH_RATIO_PERCENT {
        return false;
    }

    levenshtein_at_most(left, right, MAX_EDIT_DISTANCE)
}

fn levenshtein_at_most(left: &str, right: &str, max_distance: usize) -> bool {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();

    if left_chars.len().abs_diff(right_chars.len()) > max_distance {
        return false;
    }

    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left_chars.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];

        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution_cost);
            row_min = row_min.min(current[right_index + 1]);
        }

        if row_min > max_distance {
            return false;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()] <= max_distance
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::find_similar_titles;
    use crate::rules::citations::bibtex::parse_bib_entries;

    #[test]
    fn detects_different_keys_with_nearly_same_titles() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={Scaling LLM Test-Time Compute for Reasoning}, journal={J}, year={2024}}
@misc{beta, author={B}, title={Scaling LLM Test Time Compute for Reasoning}, year={2024}, eprint={1}}",
        );

        let scoped_entries = entries.iter().collect::<Vec<_>>();
        let diagnostics = find_similar_titles(&scoped_entries);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "CIT006");
        assert!(diagnostics[0].message.contains("beta"));
        assert!(diagnostics[0].message.contains("alpha"));
        assert!(diagnostics[0]
            .hint
            .as_ref()
            .is_some_and(|hint| hint.contains("Scaling LLM Test-Time Compute for Reasoning")));
    }

    #[test]
    fn ignores_short_similar_titles() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={LLM Survey}, journal={J}, year={2024}}
@misc{beta, author={B}, title={LLM Survey}, year={2024}, eprint={1}}",
        );

        let scoped_entries = entries.iter().collect::<Vec<_>>();
        assert!(find_similar_titles(&scoped_entries).is_empty());
    }

    #[test]
    fn ignores_titles_with_different_numeric_tokens() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@misc{cifar10, author={A}, title={CIFAR-10 (Canadian Institute for Advanced Research)}, year={2009}}
@misc{cifar100, author={B}, title={CIFAR-100 (Canadian Institute for Advanced Research)}, year={2009}}",
        );

        let scoped_entries = entries.iter().collect::<Vec<_>>();
        assert!(find_similar_titles(&scoped_entries).is_empty());
    }

    #[test]
    fn ignores_publication_and_url_companion_entries() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@inproceedings{paper, author={A}, title={Neural RGB-D Surface Reconstruction}, booktitle={CVPR}, year={2022}}
@misc{paper_url, author={A}, title={Neural RGB-D Surface Reconstruction}, howpublished={\url{https://github.com/example/project}}}",
        );

        let scoped_entries = entries.iter().collect::<Vec<_>>();
        assert!(find_similar_titles(&scoped_entries).is_empty());
    }

    #[test]
    fn keeps_conference_and_journal_version_warnings() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@inproceedings{okumura2019priority, title={Priority inheritance with backtracking for iterative multi-agent path finding}, author={Okumura, Keisuke}, booktitle={IJCAI}, year={2019}}
@article{okumura2022priority, title={Priority inheritance with backtracking for iterative multi-agent path finding}, author={Okumura, Keisuke}, journal={Artificial Intelligence}, year={2022}}",
        );

        let scoped_entries = entries.iter().collect::<Vec<_>>();
        let diagnostics = find_similar_titles(&scoped_entries);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("okumura2019priority"));
        assert!(diagnostics[0].message.contains("okumura2022priority"));
    }

    #[test]
    fn collapses_duplicate_entries_and_title_clusters() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={A Very Long Duplicate Paper Title}, journal={J}, year={2024}}
@misc{beta, author={B}, title={A Very Long Duplicate Paper Title}, year={2024}, eprint={1}}
@misc{gamma, author={C}, title={A Very Long Duplicate Paper Title}, year={2024}, eprint={2}}
@misc{beta, author={B}, title={A Very Long Duplicate Paper Title}, year={2024}, eprint={1}}",
        );

        let scoped_entries = entries.iter().collect::<Vec<_>>();
        let diagnostics = find_similar_titles(&scoped_entries);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("alpha"));
        assert!(diagnostics[0].message.contains("beta"));
        assert!(diagnostics[0].message.contains("gamma"));
    }
}
