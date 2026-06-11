use crate::diagnostic::{Diagnostic, Severity};

use super::BibEntry;

pub(super) fn find_similar_titles(entries: &[&BibEntry]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let titled_entries: Vec<_> = entries
        .iter()
        .filter_map(|entry| normalized_title(entry).map(|title| (*entry, title)))
        .collect();

    for left_index in 0..titled_entries.len() {
        let (left, left_title) = &titled_entries[left_index];
        for (right, right_title) in titled_entries.iter().skip(left_index + 1) {
            if left.key == right.key {
                continue;
            }

            if titles_are_similar(left_title, right_title) {
                diagnostics.push(Diagnostic::new(
                    "CIT006",
                    Severity::Warning,
                    format!(
                        "bibliography entry '{}' has a title very similar to '{}'",
                        right.key, left.key
                    ),
                    &right.file,
                    right.line,
                    right.column,
                ));
            }
        }
    }

    diagnostics
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
    if normalized.len() < 24 {
        None
    } else {
        Some(normalized)
    }
}

fn titles_are_similar(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }

    let shorter = left.len().min(right.len());
    let longer = left.len().max(right.len());
    if shorter < 24 || shorter * 100 / longer < 85 {
        return false;
    }

    levenshtein_at_most(left, right, 4)
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
}
