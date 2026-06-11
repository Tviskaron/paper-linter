#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatexCommand {
    pub name: String,
    pub line: usize,
    pub column: usize,
}

pub fn commands_in_line(line: &str, line_number: usize) -> Vec<LatexCommand> {
    let mut commands = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some((index, character)) = chars.next() {
        if character == '%' {
            break;
        }

        if character != '\\' {
            continue;
        }

        let Some((_, next)) = chars.peek().copied() else {
            continue;
        };

        if !next.is_ascii_alphabetic() {
            chars.next();
            continue;
        }

        let mut end = index + character.len_utf8();
        while let Some((next_index, next_character)) = chars.peek().copied() {
            if !next_character.is_ascii_alphabetic() {
                break;
            }

            chars.next();
            end = next_index + next_character.len_utf8();
        }

        commands.push(LatexCommand {
            name: line[index + 1..end].to_string(),
            line: line_number,
            column: byte_to_column(line, index),
        });
    }

    commands
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use super::commands_in_line;

    #[test]
    fn finds_commands_before_comment() {
        let commands = commands_in_line(r"See \ref{fig:a}. % \cite{ignored}", 1);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "ref");
        assert_eq!(commands[0].column, 5);
    }

    #[test]
    fn ignores_escaped_non_letter_commands() {
        let commands = commands_in_line(r"100\% complete and \LaTeX{}", 1);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "LaTeX");
    }
}
