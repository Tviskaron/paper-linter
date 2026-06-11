pub(super) fn skip_ascii_whitespace(line: &str, mut offset: usize) -> usize {
    while let Some(byte) = line.as_bytes().get(offset) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        offset += 1;
    }
    offset
}

pub(super) fn balanced_group_end(
    line: &str,
    start: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0;
    let mut escaped = false;

    for (relative, character) in line[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + relative);
            }
        }
    }

    None
}
