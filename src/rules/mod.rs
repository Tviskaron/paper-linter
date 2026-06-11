mod cap001;
pub(crate) mod citations;
mod env001;
mod fig001;
mod fig002;
mod fmt001;
mod fmt002;
mod lbl001;
mod sec001;
mod sec002;
mod tab001;
mod tex001;
mod txt001;
mod txt002;
mod ws001;

use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;

pub trait Rule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic>;
}

pub trait ProjectRule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic>;
}

static ENV001_RULE: env001::EnvironmentMismatch = env001::EnvironmentMismatch;
static FMT001_RULE: fmt001::MissingFinalNewline = fmt001::MissingFinalNewline;
static FMT002_RULE: fmt002::RepeatedBlankLines = fmt002::RepeatedBlankLines;
static SEC001_RULE: sec001::SkippedSectionLevel = sec001::SkippedSectionLevel;
static SEC002_RULE: sec002::EmptySection = sec002::EmptySection;
static TEX001_RULE: tex001::MissingNonBreakingSpace = tex001::MissingNonBreakingSpace;
static TXT001_RULE: txt001::PlaceholderText = txt001::PlaceholderText;
static TXT002_RULE: txt002::RepeatedWords = txt002::RepeatedWords;
static WS001_RULE: ws001::TrailingWhitespace = ws001::TrailingWhitespace;
static RULES: [&dyn Rule; 9] = [
    &ENV001_RULE,
    &FMT001_RULE,
    &FMT002_RULE,
    &SEC001_RULE,
    &SEC002_RULE,
    &TEX001_RULE,
    &TXT001_RULE,
    &TXT002_RULE,
    &WS001_RULE,
];

static FIG001_RULE: fig001::MissingAsset = fig001::MissingAsset;
static CAP001_RULE: cap001::MissingCaption = cap001::MissingCaption;
static FIG002_RULE: fig002::OrphanFigure = fig002::OrphanFigure;
static TAB001_RULE: tab001::OrphanTable = tab001::OrphanTable;
static LBL001_RULE: lbl001::UnusedLabel = lbl001::UnusedLabel;
static PROJECT_RULES: [&dyn ProjectRule; 5] = [
    &FIG001_RULE,
    &CAP001_RULE,
    &FIG002_RULE,
    &TAB001_RULE,
    &LBL001_RULE,
];

pub fn all_rules() -> &'static [&'static dyn Rule] {
    &RULES
}

pub fn all_project_rules() -> &'static [&'static dyn ProjectRule] {
    &PROJECT_RULES
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleInfo {
    pub code: &'static str,
    pub name: &'static str,
    pub default_severity: Severity,
    pub summary: &'static str,
    pub why: &'static str,
    pub fix: &'static str,
}

static RULE_INFOS: [RuleInfo; 20] = [
    RuleInfo {
        code: "CAP001",
        name: "caption missing",
        default_severity: Severity::Warning,
        summary: "A figure or table float has no caption.",
        why: "Captions make floats understandable on their own and are required by most paper styles.",
        fix: "Add a \\caption{...} inside the float.",
    },
    RuleInfo {
        code: "CIT001",
        name: "missing citation key",
        default_severity: Severity::Error,
        summary: "A citation key used in TeX is not present in the active bibliography sources.",
        why: "Missing keys usually become failed LaTeX builds or unresolved references in the final PDF.",
        fix: "Add the entry to a .bib file, fix the key spelling, or ensure the .bbl/thebibliography source is reachable.",
    },
    RuleInfo {
        code: "CIT002",
        name: "unused bibliography entry",
        default_severity: Severity::Warning,
        summary: "A bibliography entry is present in the active bibliography set but is not cited from reachable TeX sources.",
        why: "Unused references make paper bibliographies noisy and often signal stale or accidentally duplicated entries.",
        fix: "Remove the entry, cite it, or use \\nocite{key} / \\nocite{*} when the uncited entry is intentional.",
    },
    RuleInfo {
        code: "CIT003",
        name: "missing bibliography file",
        default_severity: Severity::Error,
        summary: "A declared bibliography file could not be found on disk.",
        why: "The paper cannot be checked or built reliably when declared bibliography inputs are missing.",
        fix: "Fix the \\bibliography or \\addbibresource path, add the missing .bib file, or pass the .bib path explicitly.",
    },
    RuleInfo {
        code: "CIT004",
        name: "missing required bibliography fields",
        default_severity: Severity::Warning,
        summary: "An active bibliography entry is missing required author/editor, year/date, or venue information.",
        why: "Incomplete entries produce weak citations and can violate venue bibliography requirements.",
        fix: "Fill the missing fields in the .bib entry. For arXiv/preprint entries, eprint/archive fields can satisfy venue information.",
    },
    RuleInfo {
        code: "CIT005",
        name: "duplicate bibliography key",
        default_severity: Severity::Warning,
        summary: "The same bibliography key is defined more than once.",
        why: "Duplicate keys make citation resolution order-dependent and can hide the reference that will actually be rendered.",
        fix: "Keep one definition for the key, or rename distinct papers to distinct keys and update citations.",
    },
    RuleInfo {
        code: "CIT006",
        name: "similar bibliography titles with different keys",
        default_severity: Severity::Warning,
        summary: "Two active bibliography entries have very similar normalized titles but different keys.",
        why: "Near-duplicate titles often come from merged bibliographies and can leave multiple keys for the same paper.",
        fix: "Compare the entries, delete the duplicate if they describe the same paper, or make the titles/metadata precise if they are different papers.",
    },
    RuleInfo {
        code: "ENV001",
        name: "environment begin/end mismatch",
        default_severity: Severity::Error,
        summary: "A LaTeX environment is opened or closed inconsistently.",
        why: "Mismatched environments often break compilation or move large parts of the paper into the wrong scope.",
        fix: "Pair every \\begin{...} with the matching \\end{...} in the same logical block.",
    },
    RuleInfo {
        code: "FIG001",
        name: "missing figure asset",
        default_severity: Severity::Error,
        summary: "A figure includes an asset path that cannot be resolved on disk.",
        why: "Missing assets break paper builds and leave figures absent from the rendered PDF.",
        fix: "Fix the \\includegraphics path, add the missing file, or use a supported extension for extensionless paths.",
    },
    RuleInfo {
        code: "FIG002",
        name: "orphan figure",
        default_severity: Severity::Warning,
        summary: "A figure label is not referenced from reachable TeX sources.",
        why: "Unreferenced figures are often stale draft material or missing narrative links in the paper.",
        fix: "Reference the figure with \\ref{...} or remove the unused figure/label.",
    },
    RuleInfo {
        code: "FMT001",
        name: "missing final newline",
        default_severity: Severity::Warning,
        summary: "A checked file does not end with a newline.",
        why: "Final newlines keep diffs cleaner and avoid toolchain edge cases around the last line.",
        fix: "Add a newline at the end of the file.",
    },
    RuleInfo {
        code: "FMT002",
        name: "repeated blank lines",
        default_severity: Severity::Warning,
        summary: "A file contains more than two consecutive blank lines.",
        why: "Large blank runs usually come from editing leftovers and make source harder to scan.",
        fix: "Collapse repeated blank lines to the intended section break.",
    },
    RuleInfo {
        code: "LBL001",
        name: "unused label",
        default_severity: Severity::Warning,
        summary: "A non-float label is never referenced from reachable TeX sources.",
        why: "Unused labels usually indicate stale anchors or missing references.",
        fix: "Reference the label with \\ref{...} or remove it.",
    },
    RuleInfo {
        code: "SEC001",
        name: "skipped section hierarchy level",
        default_severity: Severity::Warning,
        summary: "A section heading jumps over an intermediate level.",
        why: "Skipped levels can make the paper outline harder to navigate and can produce odd generated structure.",
        fix: "Insert the missing intermediate heading or change the heading command to the next expected level.",
    },
    RuleInfo {
        code: "SEC002",
        name: "empty section",
        default_severity: Severity::Warning,
        summary: "A section heading has no meaningful content before the next peer or parent section.",
        why: "Empty sections are usually placeholders that should not survive into a paper draft.",
        fix: "Add content under the heading or remove the empty section.",
    },
    RuleInfo {
        code: "TAB001",
        name: "orphan table",
        default_severity: Severity::Warning,
        summary: "A table label is not referenced from reachable TeX sources.",
        why: "Unreferenced tables are often stale draft material or missing narrative links in the paper.",
        fix: "Reference the table with \\ref{...} or remove the unused table/label.",
    },
    RuleInfo {
        code: "TEX001",
        name: "missing non-breaking space before reference or citation",
        default_severity: Severity::Warning,
        summary: "Text uses a normal space before a reference or citation command where a non-breaking space is expected.",
        why: "A non-breaking space keeps references and citations attached to the preceding word in the rendered PDF.",
        fix: "Replace the space before commands such as \\cite, \\ref, \\eqref, or \\cref with ~.",
    },
    RuleInfo {
        code: "TXT001",
        name: "placeholder text",
        default_severity: Severity::Warning,
        summary: "The source contains placeholder markers such as TODO, TBD, FIXME, lorem text, or similar phrases.",
        why: "Placeholder text is easy to miss during review and should not appear in submitted papers.",
        fix: "Resolve the placeholder, replace it with final text, or remove it.",
    },
    RuleInfo {
        code: "TXT002",
        name: "repeated word",
        default_severity: Severity::Warning,
        summary: "A prose line contains the same word twice in a row.",
        why: "Repeated words are common editing mistakes and are rarely intentional in paper prose.",
        fix: "Remove the extra word or rewrite the phrase.",
    },
    RuleInfo {
        code: "WS001",
        name: "trailing whitespace",
        default_severity: Severity::Warning,
        summary: "A line ends with extra spaces or tabs.",
        why: "Trailing whitespace creates noisy diffs and can interact badly with some LaTeX formatting patterns.",
        fix: "Remove the spaces or tabs at the end of the line.",
    },
];

pub fn rule_infos() -> &'static [RuleInfo] {
    &RULE_INFOS
}

pub fn find_rule_info(code: &str) -> Option<&'static RuleInfo> {
    rule_infos()
        .iter()
        .find(|rule| rule.code.eq_ignore_ascii_case(code))
}

#[cfg(test)]
mod tests {
    use super::{all_project_rules, all_rules, find_rule_info, rule_infos, ProjectRule, Rule};

    #[test]
    fn rule_registry_contains_rules() {
        let codes: Vec<_> = all_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec![
                "ENV001", "FMT001", "FMT002", "SEC001", "SEC002", "TEX001", "TXT001", "TXT002",
                "WS001"
            ]
        );
    }

    #[test]
    fn project_rule_registry_contains_figures_and_tables_rules() {
        let codes: Vec<_> = all_project_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec!["FIG001", "CAP001", "FIG002", "TAB001", "LBL001"]
        );
    }

    fn assert_rule_trait_object(_: &dyn Rule) {}

    fn assert_project_rule_trait_object(_: &dyn ProjectRule) {}

    #[test]
    fn registry_rules_are_trait_objects() {
        assert_rule_trait_object(all_rules()[0]);
        assert_project_rule_trait_object(all_project_rules()[0]);
    }

    #[test]
    fn rule_info_catalog_contains_all_known_codes() {
        let codes: Vec<_> = rule_infos().iter().map(|rule| rule.code).collect();

        assert_eq!(
            codes,
            vec![
                "CAP001", "CIT001", "CIT002", "CIT003", "CIT004", "CIT005", "CIT006", "ENV001",
                "FIG001", "FIG002", "FMT001", "FMT002", "LBL001", "SEC001", "SEC002", "TAB001",
                "TEX001", "TXT001", "TXT002", "WS001"
            ]
        );
    }

    #[test]
    fn finds_rule_info_case_insensitively() {
        assert_eq!(
            find_rule_info("cit002").map(|rule| rule.code),
            Some("CIT002")
        );
    }
}
