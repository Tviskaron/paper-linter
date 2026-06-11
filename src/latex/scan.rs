use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    fn new(file: impl Into<PathBuf>, line: usize, column: usize) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatKind {
    Figure,
    Table,
}

impl FloatKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FloatKind::Figure => "figure",
            FloatKind::Table => "table",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    Figure,
    Table,
    Other,
}

impl From<Option<FloatKind>> for LabelKind {
    fn from(kind: Option<FloatKind>) -> Self {
        match kind {
            Some(FloatKind::Figure) => LabelKind::Figure,
            Some(FloatKind::Table) => LabelKind::Table,
            None => LabelKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub key: String,
    pub kind: LabelKind,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    pub key: String,
    pub command: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graphic {
    pub raw_path: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsPath {
    pub raw_path: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caption {
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Include {
    pub raw_path: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentClass {
    pub name: String,
    pub options: Vec<String>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageImport {
    pub name: String,
    pub options: Vec<String>,
    pub command: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatEnv {
    pub kind: FloatKind,
    pub env_name: String,
    pub location: SourceLocation,
    pub labels: Vec<Label>,
    pub captions: Vec<Caption>,
    pub graphics: Vec<Graphic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub labels: Vec<Label>,
    pub refs: Vec<Ref>,
    pub graphics: Vec<Graphic>,
    pub graphics_paths: Vec<GraphicsPath>,
    pub includes: Vec<Include>,
    pub document_classes: Vec<DocumentClass>,
    pub packages: Vec<PackageImport>,
    pub floats: Vec<FloatEnv>,
}

#[derive(Debug, Clone)]
struct ActiveEnv {
    env_name: String,
    kind: Option<FloatKind>,
    ignored: bool,
    location: SourceLocation,
    labels: Vec<Label>,
    captions: Vec<Caption>,
    graphics: Vec<Graphic>,
}

pub fn scan_latex(file: impl Into<PathBuf>, content: &str) -> ScanResult {
    let file = file.into();
    let bytes = content.as_bytes();
    let mut result = ScanResult::default();
    let mut stack: Vec<ActiveEnv> = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && !is_escaped(bytes, index) {
            index = skip_comment(bytes, index);
            continue;
        }

        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }

        let command_start = index;
        let Some((command, after_command)) = parse_command_name(content, index) else {
            index += 1;
            continue;
        };
        let (line, column) = line_column(content, command_start);
        let location = SourceLocation::new(file.clone(), line, column);
        let in_ignored = stack.iter().any(|env| env.ignored);

        match command.as_str() {
            "begin" => {
                if let Some((env_name, end)) = parse_required_arg(content, after_command) {
                    stack.push(ActiveEnv {
                        kind: float_kind(&env_name),
                        ignored: ignored_env(&env_name),
                        env_name,
                        location,
                        labels: Vec::new(),
                        captions: Vec::new(),
                        graphics: Vec::new(),
                    });
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "end" => {
                if let Some((env_name, end)) = parse_required_arg(content, after_command) {
                    if let Some(position) = stack.iter().rposition(|env| env.env_name == env_name) {
                        let env = stack.remove(position);
                        finish_env(env, &mut result);
                    }
                    index = end;
                } else {
                    index = after_command;
                }
            }
            _ if in_ignored => {
                index = after_command;
            }
            "includegraphics" => {
                let arg_start = skip_optional_args(content, after_command);
                if let Some((raw_path, end)) = parse_required_arg(content, arg_start) {
                    let graphic = Graphic { raw_path, location };
                    attach_graphic(&mut stack, graphic.clone());
                    result.graphics.push(graphic);
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "graphicspath" => {
                if let Some((raw_paths, end)) = parse_required_arg(content, after_command) {
                    result
                        .graphics_paths
                        .extend(
                            parse_graphics_paths(&raw_paths)
                                .into_iter()
                                .map(|raw_path| GraphicsPath {
                                    raw_path,
                                    location: location.clone(),
                                }),
                        );
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "caption" => {
                let arg_start = skip_optional_args(content, after_command);
                if let Some((_text, end)) = parse_required_arg(content, arg_start) {
                    let caption = Caption { location };
                    attach_caption(&mut stack, caption.clone());
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "label" => {
                if let Some((key, end)) = parse_required_arg(content, after_command) {
                    let kind = LabelKind::from(current_float_kind(&stack));
                    let label = Label {
                        key,
                        kind,
                        location,
                    };
                    attach_label(&mut stack, label.clone());
                    result.labels.push(label);
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "ref" | "autoref" | "cref" | "Cref" | "pageref" | "nameref" => {
                if let Some((keys, end)) = parse_required_arg(content, after_command) {
                    for key in keys.split(',').map(str::trim).filter(|key| !key.is_empty()) {
                        result.refs.push(Ref {
                            key: key.to_string(),
                            command: command.clone(),
                            location: location.clone(),
                        });
                    }
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "input" | "include" => {
                if let Some((raw_path, end)) = parse_required_arg(content, after_command) {
                    result.includes.push(Include { raw_path, location });
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "documentclass" => {
                let (options, arg_start) = parse_optional_arg_values(content, after_command);
                if let Some((name, end)) = parse_required_arg(content, arg_start) {
                    result.document_classes.push(DocumentClass {
                        name,
                        options,
                        location,
                    });
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "usepackage" | "RequirePackage" => {
                let (options, arg_start) = parse_optional_arg_values(content, after_command);
                if let Some((names, end)) = parse_required_arg(content, arg_start) {
                    result
                        .packages
                        .extend(
                            split_comma_list(&names)
                                .into_iter()
                                .map(|name| PackageImport {
                                    name,
                                    options: options.clone(),
                                    command: command.clone(),
                                    location: location.clone(),
                                }),
                        );
                    index = end;
                } else {
                    index = after_command;
                }
            }
            _ => {
                index = after_command;
            }
        }
    }

    while let Some(env) = stack.pop() {
        finish_env(env, &mut result);
    }

    result
}

fn finish_env(env: ActiveEnv, result: &mut ScanResult) {
    if let Some(kind) = env.kind {
        result.floats.push(FloatEnv {
            kind,
            env_name: env.env_name,
            location: env.location,
            labels: env.labels,
            captions: env.captions,
            graphics: env.graphics,
        });
    }
}

fn attach_label(stack: &mut [ActiveEnv], label: Label) {
    if let Some(env) = stack.iter_mut().rev().find(|env| env.kind.is_some()) {
        env.labels.push(label);
    }
}

fn attach_caption(stack: &mut [ActiveEnv], caption: Caption) {
    if let Some(env) = stack.iter_mut().rev().find(|env| env.kind.is_some()) {
        env.captions.push(caption);
    }
}

fn attach_graphic(stack: &mut [ActiveEnv], graphic: Graphic) {
    if let Some(env) = stack.iter_mut().rev().find(|env| env.kind.is_some()) {
        env.graphics.push(graphic);
    }
}

fn current_float_kind(stack: &[ActiveEnv]) -> Option<FloatKind> {
    stack.iter().rev().find_map(|env| env.kind)
}

fn parse_command_name(content: &str, start: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    if bytes.get(start) != Some(&b'\\') {
        return None;
    }

    let mut end = start + 1;
    while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
        end += 1;
    }

    if end == start + 1 {
        return None;
    }

    Some((content[start + 1..end].to_string(), end))
}

fn parse_required_arg(content: &str, start: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    let open = skip_ws(bytes, start);
    if bytes.get(open) != Some(&b'{') {
        return None;
    }

    let mut depth = 1usize;
    let mut index = open + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((content[open + 1..index].trim().to_string(), index + 1));
                }
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    None
}

fn parse_graphics_paths(raw_paths: &str) -> Vec<String> {
    let bytes = raw_paths.as_bytes();
    let mut paths = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }

        if bytes[index] != b'{' {
            break;
        }

        let start = index + 1;
        let mut depth = 1usize;
        index += 1;

        while index < bytes.len() {
            match bytes[index] {
                b'\\' => {
                    index = (index + 2).min(bytes.len());
                }
                b'{' => {
                    depth += 1;
                    index += 1;
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        let path = raw_paths[start..index].trim();
                        if !path.is_empty() {
                            paths.push(path.to_string());
                        }
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                _ => index += 1,
            }
        }
    }

    if paths.is_empty() {
        let path = raw_paths.trim();
        if !path.is_empty() {
            paths.push(path.to_string());
        }
    }

    paths
}

fn skip_optional_args(content: &str, start: usize) -> usize {
    parse_optional_arg_values(content, start).1
}

fn parse_optional_arg_values(content: &str, start: usize) -> (Vec<String>, usize) {
    let bytes = content.as_bytes();
    let mut index = start;
    let mut values = Vec::new();

    loop {
        index = skip_ws(bytes, index);
        if bytes.get(index) != Some(&b'[') {
            return (values, index);
        }

        let Some(end) = skip_optional_arg(bytes, index) else {
            return (values, index);
        };
        values.extend(split_comma_list(&content[index + 1..end - 1]));
        index = end;
    }
}

fn split_comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn skip_optional_arg(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut index = start + 1;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            b'[' => {
                depth += 1;
                index += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
                index += 1;
            }
            _ => index += 1,
        }
    }

    None
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn skip_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut count = 0usize;
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        count += 1;
        cursor -= 1;
    }
    count % 2 == 1
}

fn line_column(content: &str, target: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;

    for (index, character) in content.char_indices() {
        if index >= target {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

fn float_kind(env_name: &str) -> Option<FloatKind> {
    match env_name {
        "figure" | "figure*" | "wrapfigure" | "sidewaysfigure" | "subfigure" => {
            Some(FloatKind::Figure)
        }
        "table" | "table*" | "sidewaystable" | "subtable" => Some(FloatKind::Table),
        _ => None,
    }
}

fn ignored_env(env_name: &str) -> bool {
    matches!(env_name, "verbatim" | "Verbatim" | "minted" | "lstlisting")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{scan_latex, FloatKind, LabelKind};

    #[test]
    fn ignores_commands_inside_comments() {
        let scan = scan_latex(
            Path::new("paper.tex"),
            "% \\label{fig:ignored}\n\\ref{fig:used}\n",
        );

        assert!(scan.labels.is_empty());
        assert_eq!(scan.refs.len(), 1);
        assert_eq!(scan.refs[0].key, "fig:used");
    }

    #[test]
    fn collects_figure_contents() {
        let scan = scan_latex(
            Path::new("paper.tex"),
            "\\begin{figure}\n\\includegraphics[width=.5\\textwidth]{figures/model}\n\\caption[Short]{Long}\n\\label{fig:model}\n\\end{figure}\n",
        );

        assert_eq!(scan.floats.len(), 1);
        assert_eq!(scan.floats[0].kind, FloatKind::Figure);
        assert_eq!(scan.floats[0].graphics[0].raw_path, "figures/model");
        assert_eq!(scan.floats[0].captions.len(), 1);
        assert_eq!(scan.floats[0].labels[0].key, "fig:model");
        assert_eq!(scan.labels[0].kind, LabelKind::Figure);
    }

    #[test]
    fn collects_table_contents() {
        let scan = scan_latex(
            Path::new("paper.tex"),
            "\\begin{table}\n\\caption{Rows}\n\\label{tab:rows}\n\\end{table}\n",
        );

        assert_eq!(scan.floats.len(), 1);
        assert_eq!(scan.floats[0].kind, FloatKind::Table);
        assert_eq!(scan.floats[0].captions.len(), 1);
        assert_eq!(scan.labels[0].kind, LabelKind::Table);
    }

    #[test]
    fn malformed_float_does_not_panic() {
        let scan = scan_latex(
            Path::new("paper.tex"),
            "\\begin{figure}\n\\label{fig:open}\n\\includegraphics{oops}\n",
        );

        assert_eq!(scan.floats.len(), 1);
        assert_eq!(scan.floats[0].labels[0].key, "fig:open");
        assert_eq!(scan.floats[0].graphics[0].raw_path, "oops");
    }

    #[test]
    fn records_inputs_and_includes() {
        let scan = scan_latex(
            Path::new("paper.tex"),
            "\\input{sections/method}\n\\include{sections/results}\n",
        );

        let paths: Vec<_> = scan
            .includes
            .iter()
            .map(|include| include.raw_path.as_str())
            .collect();
        assert_eq!(paths, vec!["sections/method", "sections/results"]);
    }

    #[test]
    fn records_document_class_and_packages() {
        let scan = scan_latex(
            Path::new("paper.tex"),
            "\\documentclass[twocolumn,10pt]{article}\n\\usepackage[sort&compress]{natbib,graphicx}\n\\RequirePackage{amsmath}\n",
        );

        assert_eq!(scan.document_classes.len(), 1);
        assert_eq!(scan.document_classes[0].name, "article");
        assert_eq!(scan.document_classes[0].options, vec!["twocolumn", "10pt"]);
        assert_eq!(scan.packages.len(), 3);
        assert_eq!(scan.packages[0].name, "natbib");
        assert_eq!(scan.packages[0].options, vec!["sort&compress"]);
        assert_eq!(scan.packages[0].command, "usepackage");
        assert_eq!(scan.packages[1].name, "graphicx");
        assert_eq!(scan.packages[2].name, "amsmath");
        assert_eq!(scan.packages[2].command, "RequirePackage");
    }

    #[test]
    fn records_graphics_paths() {
        let scan = scan_latex(
            Path::new("paper.tex"),
            "\\graphicspath{{images/}{../shared figures/}}\n",
        );

        let paths: Vec<_> = scan
            .graphics_paths
            .iter()
            .map(|path| path.raw_path.as_str())
            .collect();
        assert_eq!(paths, vec!["images/", "../shared figures/"]);
    }
}
