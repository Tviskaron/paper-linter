use crate::latex::scan::ScanAliases;

pub fn infer_aliases_from_content(content: &str) -> ScanAliases {
    let mut aliases = ScanAliases::default();
    let mut offset = 0;

    while let Some(relative) = content[offset..].find("\\newcommand") {
        let start = offset + relative;
        if let Some((name, body)) = parse_newcommand_alias(content, start) {
            if let Some(kind) = classify_wrapper_body(&body) {
                match kind {
                    AliasKind::Ref => aliases.refs.push(name),
                    AliasKind::Cite => aliases.cites.push(name),
                    AliasKind::Input => aliases.inputs.push(name),
                    AliasKind::Graphic => aliases.graphics.push(name),
                }
            }
        }
        offset = start + "\\newcommand".len();
    }

    aliases
}

pub fn infer_aliases_from_sources(contents: &[&str]) -> ScanAliases {
    let mut aliases = ScanAliases::default();
    for content in contents {
        aliases.merge_missing(&infer_aliases_from_content(content));
    }
    aliases
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AliasKind {
    Ref,
    Cite,
    Graphic,
    Input,
}

fn classify_wrapper_body(body: &str) -> Option<AliasKind> {
    let trimmed = body.trim();
    if trimmed == r"\ref{#1}" || trimmed == r"\eqref{#1}" || trimmed == r"\autoref{#1}" {
        return Some(AliasKind::Ref);
    }
    if trimmed == r"\cite{#1}"
        || trimmed == r"\citep{#1}"
        || trimmed == r"\citet{#1}"
        || trimmed == r"\parencite{#1}"
        || trimmed == r"\textcite{#1}"
    {
        return Some(AliasKind::Cite);
    }
    if trimmed == r"\input{#1}" || trimmed == r"\include{#1}" {
        return Some(AliasKind::Input);
    }
    if trimmed == r"\includegraphics{#1}" || trimmed == r"\includegraphics[width=\linewidth]{#1}" {
        return Some(AliasKind::Graphic);
    }
    None
}

fn parse_newcommand_alias(content: &str, start: usize) -> Option<(String, String)> {
    let mut index = start + "\\newcommand".len();
    index = skip_ws(content, index);
    if content.as_bytes().get(index) != Some(&b'{') {
        return None;
    }
    let (name, mut index) = read_braced_token(content, index)?;
    index = skip_ws(content, index);
    if content.as_bytes().get(index) != Some(&b'[') {
        return None;
    }
    let (_, mut index) = read_bracket_token(content, index)?;
    index = skip_ws(content, index);
    let (body, _) = read_braced_token(content, index)?;
    Some((name.trim_start_matches('\\').to_string(), body))
}

fn read_braced_token(content: &str, start: usize) -> Option<(String, usize)> {
    if content.as_bytes().get(start) != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let bytes = content.as_bytes();
    for index in start..bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((content[start + 1..index].to_string(), index + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn read_bracket_token(content: &str, start: usize) -> Option<(String, usize)> {
    if content.as_bytes().get(start) != Some(&b'[') {
        return None;
    }
    let end = content[start + 1..].find(']')? + start + 1;
    Some((content[start + 1..end].to_string(), end + 1))
}

fn skip_ws(content: &str, mut index: usize) -> usize {
    while let Some(ch) = content[index..].chars().next() {
        if ch.is_whitespace() {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    index
}

#[cfg(test)]
mod tests {
    use super::infer_aliases_from_content;

    #[test]
    fn infers_ref_and_cite_wrappers() {
        let aliases = infer_aliases_from_content(
            "\\newcommand{\\figref}[1]{\\ref{#1}}\n\\newcommand{\\mycite}[1]{\\cite{#1}}\n",
        );
        assert!(aliases.refs.contains(&"figref".to_string()));
        assert!(aliases.cites.contains(&"mycite".to_string()));
    }

    #[test]
    fn ignores_non_wrapper_macros() {
        let aliases = infer_aliases_from_content("\\newcommand{\\foo}[1]{Figure~\\ref{#1}}\n");
        assert!(aliases.refs.is_empty());
    }
}
