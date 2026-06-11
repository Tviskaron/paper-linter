mod cap001;
mod cap002;
pub(crate) mod citations;
mod cmt001;
mod env001;
mod fig001;
mod fig002;
mod fig003;
mod fig004;
mod fig005;
mod fig006;
mod fmt001;
mod fmt002;
mod lat001;
mod lat002;
mod lbl001;
mod mth001;
mod mth002;
mod prj001;
mod prj002;
mod prj003;
mod prj004;
mod ref001;
mod sec001;
mod sec002;
mod sec003;
mod sec004;
mod tab001;
mod tab002;
mod tex001;
mod txt001;
mod txt002;
mod txt003;
mod txt004;
mod txt005;
mod ws001;

use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::project_graph::ProjectGraph;

pub trait Rule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic>;

    fn strict_only(&self) -> bool {
        false
    }
}

pub trait ProjectRule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic>;

    fn strict_only(&self) -> bool {
        false
    }
}

pub trait GraphProjectRule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_graph(&self, graph: &ProjectGraph) -> Vec<Diagnostic>;
}

static ENV001_RULE: env001::EnvironmentMismatch = env001::EnvironmentMismatch;
static FMT001_RULE: fmt001::MissingFinalNewline = fmt001::MissingFinalNewline;
static FMT002_RULE: fmt002::RepeatedBlankLines = fmt002::RepeatedBlankLines;
static SEC001_RULE: sec001::SkippedSectionLevel = sec001::SkippedSectionLevel;
static SEC002_RULE: sec002::EmptySection = sec002::EmptySection;
static SEC003_RULE: sec003::SingletonSubdivision = sec003::SingletonSubdivision;
static SEC004_RULE: sec004::StackedHeadings = sec004::StackedHeadings;
static MTH001_RULE: mth001::DoubleDollarDisplayMath = mth001::DoubleDollarDisplayMath;
static MTH002_RULE: mth002::UnbracedMathScript = mth002::UnbracedMathScript;
static LAT002_RULE: lat002::PrimitiveTex = lat002::PrimitiveTex;
static TEX001_RULE: tex001::MissingNonBreakingSpace = tex001::MissingNonBreakingSpace;
static TXT001_RULE: txt001::PlaceholderText = txt001::PlaceholderText;
static TXT002_RULE: txt002::RepeatedWords = txt002::RepeatedWords;
static TXT003_RULE: txt003::LongSentence = txt003::LongSentence;
static TXT004_RULE: txt004::FillerWords = txt004::FillerWords;
static TXT005_RULE: txt005::PassiveVoice = txt005::PassiveVoice;
static CMT001_RULE: cmt001::EditorialComment = cmt001::EditorialComment;
static WS001_RULE: ws001::TrailingWhitespace = ws001::TrailingWhitespace;
static RULES: [&dyn Rule; 18] = [
    &ENV001_RULE,
    &FMT001_RULE,
    &FMT002_RULE,
    &SEC001_RULE,
    &SEC002_RULE,
    &SEC003_RULE,
    &SEC004_RULE,
    &MTH001_RULE,
    &MTH002_RULE,
    &LAT002_RULE,
    &TEX001_RULE,
    &TXT001_RULE,
    &TXT002_RULE,
    &TXT003_RULE,
    &TXT004_RULE,
    &TXT005_RULE,
    &CMT001_RULE,
    &WS001_RULE,
];

static FIG001_RULE: fig001::MissingAsset = fig001::MissingAsset;
static CAP001_RULE: cap001::MissingCaption = cap001::MissingCaption;
static CAP002_RULE: cap002::CaptionPunctuation = cap002::CaptionPunctuation;
static FIG002_RULE: fig002::OrphanFigure = fig002::OrphanFigure;
static FIG003_RULE: fig003::AssetCaseMismatch = fig003::AssetCaseMismatch;
static FIG004_RULE: fig004::MissingFigureLabel = fig004::MissingFigureLabel;
static FIG005_RULE: fig005::UnsafeGraphicPath = fig005::UnsafeGraphicPath;
static FIG006_RULE: fig006::ImageFormatPolicy = fig006::ImageFormatPolicy;
static TAB001_RULE: tab001::OrphanTable = tab001::OrphanTable;
static TAB002_RULE: tab002::MissingTableLabel = tab002::MissingTableLabel;
static LAT001_RULE: lat001::LegacyLatex = lat001::LegacyLatex;
static LBL001_RULE: lbl001::UnusedLabel = lbl001::UnusedLabel;
static REF001_RULE: ref001::MissingReferenceTarget = ref001::MissingReferenceTarget;
static PROJECT_RULES: [&dyn ProjectRule; 13] = [
    &FIG001_RULE,
    &CAP001_RULE,
    &CAP002_RULE,
    &FIG002_RULE,
    &FIG003_RULE,
    &FIG004_RULE,
    &FIG005_RULE,
    &FIG006_RULE,
    &TAB001_RULE,
    &TAB002_RULE,
    &LAT001_RULE,
    &REF001_RULE,
    &LBL001_RULE,
];

static PRJ001_RULE: prj001::MissingInclude = prj001::MissingInclude;
static PRJ002_RULE: prj002::AmbiguousRoot = prj002::AmbiguousRoot;
static PRJ003_RULE: prj003::RootNotFound = prj003::RootNotFound;
static PRJ004_RULE: prj004::OrphanTex = prj004::OrphanTex;
static GRAPH_PROJECT_RULES: [&dyn GraphProjectRule; 4] =
    [&PRJ001_RULE, &PRJ002_RULE, &PRJ003_RULE, &PRJ004_RULE];

pub fn all_rules() -> &'static [&'static dyn Rule] {
    &RULES
}

pub fn all_project_rules() -> &'static [&'static dyn ProjectRule] {
    &PROJECT_RULES
}

pub fn all_graph_project_rules() -> &'static [&'static dyn GraphProjectRule] {
    &GRAPH_PROJECT_RULES
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

static RULE_INFOS: [RuleInfo; 46] = [
    RuleInfo {
        code: "CMT001",
        name: "editorial comment",
        default_severity: Severity::Warning,
        summary: "A LaTeX comment contains editorial markers such as TODO, FIXME, or REVIEW.",
        why: "Editorial comments are easy to miss during review and should not appear in submitted papers.",
        fix: "Resolve the note or remove the comment before submission.",
    },
    RuleInfo {
        code: "CAP001",
        name: "caption missing",
        default_severity: Severity::Warning,
        summary: "A figure or table float has no caption.",
        why: "Captions make floats understandable on their own and are required by most paper styles.",
        fix: "Add a \\caption{...} inside the float.",
    },
    RuleInfo {
        code: "CAP002",
        name: "caption punctuation",
        default_severity: Severity::Warning,
        summary: "A figure or table caption does not end with sentence punctuation.",
        why: "Some venues and paper style guides expect captions to read as complete punctuated sentences.",
        fix: "End the caption with '.', '?', or '!'.",
    },
    RuleInfo {
        code: "BIB001",
        name: "bibliography identifier syntax",
        default_severity: Severity::Warning,
        summary: "A bibliography entry has a malformed DOI, URL, or arXiv identifier.",
        why: "Malformed bibliography identifiers break publisher metadata, links, and citation exports.",
        fix: "Fix the DOI, URL, or arXiv identifier syntax in the .bib entry.",
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
        code: "CIT007",
        name: "duplicate bibliography declaration",
        default_severity: Severity::Warning,
        summary: "The same bibliography file is declared more than once from reachable TeX sources.",
        why: "Repeated bibliography declarations can duplicate rendered references or hide which bibliography source is intended.",
        fix: "Remove the duplicate \\bibliography or \\addbibresource declaration, unless a package such as chapterbib intentionally manages multiple bibliographies.",
    },
    RuleInfo {
        code: "CIT008",
        name: "punctuation before citation",
        default_severity: Severity::Warning,
        summary: "Sentence punctuation appears before a citation command instead of after it.",
        why: "Most paper styles place punctuation after the citation so the citation belongs to the sentence.",
        fix: "Move punctuation after the citation, for example text~\\cite{key}.",
    },
    RuleInfo {
        code: "CIT009",
        name: "collapsible consecutive citations",
        default_severity: Severity::Warning,
        summary: "Adjacent compatible citation commands can be collapsed into one command.",
        why: "Merged citations are shorter, easier to edit, and usually render more cleanly.",
        fix: "Merge adjacent citations with the same command, for example \\cite{a,b}.",
    },
    RuleInfo {
        code: "CIT010",
        name: "mixed citation command families",
        default_severity: Severity::Warning,
        summary: "A document mixes explicit natbib-style and biblatex-style citation commands.",
        why: "Mixing citation command families often comes from merged drafts and can make citation style package-dependent.",
        fix: "Use one citation package command family consistently.",
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
        code: "FIG003",
        name: "asset case mismatch",
        default_severity: Severity::Error,
        summary: "A figure asset path differs from an existing filename only by letter case.",
        why: "Case mismatches compile on some local filesystems but fail on case-sensitive CI, Linux, or arXiv environments.",
        fix: "Match the case in the \\includegraphics path to the actual asset filename.",
    },
    RuleInfo {
        code: "FIG004",
        name: "figure label missing",
        default_severity: Severity::Warning,
        summary: "A figure-like float has content but no label.",
        why: "Unlabeled figures cannot be referenced robustly and often lead authors to hard-code figure numbers.",
        fix: "Add a \\label{fig:...} near the figure caption.",
    },
    RuleInfo {
        code: "FIG005",
        name: "unsafe graphic path",
        default_severity: Severity::Warning,
        summary: "A graphics path is absolute, traverses parent directories, or uses a platform-specific drive prefix.",
        why: "Non-portable graphics paths break CI, collaborators' checkouts, arXiv bundles, and camera-ready packaging.",
        fix: "Use a project-relative path inside the repository.",
    },
    RuleInfo {
        code: "FIG006",
        name: "image format",
        default_severity: Severity::Warning,
        summary: "A figure uses an explicit image extension outside the supported format set.",
        why: "Unsupported or unusual image formats often fail in CI, TeX engines, arXiv, or camera-ready packaging.",
        fix: "Use pdf, png, jpg, jpeg, eps, or svg.",
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
        code: "LAT001",
        name: "legacy latex",
        default_severity: Severity::Warning,
        summary: "A file uses legacy LaTeX commands, environments, or packages.",
        why: "Legacy constructs often interact poorly with modern packages, templates, and publisher tooling.",
        fix: "Use the modern LaTeX replacement suggested by the diagnostic hint.",
    },
    RuleInfo {
        code: "LAT002",
        name: "primitive tex",
        default_severity: Severity::Warning,
        summary: "A file uses low-level TeX primitives in LaTeX source.",
        why: "Primitive TeX commands are harder to maintain and can bypass LaTeX package/template conventions.",
        fix: "Use a LaTeX-level replacement where possible.",
    },
    RuleInfo {
        code: "MTH001",
        name: "double-dollar display math",
        default_severity: Severity::Warning,
        summary: "A file uses TeX-style $$ display math delimiters.",
        why: "In LaTeX, \\[...\\] or named display math environments are safer and avoid spacing/package edge cases.",
        fix: "Replace $$...$$ with \\[...\\] or an appropriate display math environment.",
    },
    RuleInfo {
        code: "MTH002",
        name: "unbraced math script",
        default_severity: Severity::Warning,
        summary: "A math subscript or superscript has multiple characters without braces.",
        why: "Unbraced multi-character scripts often render only the first character as the script and leave the rest at baseline.",
        fix: "Wrap the script in braces, for example x^{10} or a_{ij}.",
    },
    RuleInfo {
        code: "PRJ001",
        name: "missing include",
        default_severity: Severity::Error,
        summary: "An \\input, \\include, or \\subfile target cannot be resolved on disk.",
        why: "Missing include targets usually break compilation or omit entire sections from the paper.",
        fix: "Fix the path, add the missing .tex file, or remove the include command.",
    },
    RuleInfo {
        code: "PRJ002",
        name: "ambiguous root",
        default_severity: Severity::Warning,
        summary: "Multiple candidate root .tex files were found in the project directory.",
        why: "Ambiguous roots make project-wide checks unreliable when linting a directory.",
        fix: "Add 00README.json, a magic root comment, or rename the intended root file clearly.",
    },
    RuleInfo {
        code: "PRJ003",
        name: "root not found",
        default_severity: Severity::Error,
        summary: "No root .tex file could be resolved in a directory that contains .tex sources.",
        why: "Without a root document, project-aware checks cannot follow the intended paper structure.",
        fix: "Add a \\documentclass root file, 00README.json, or a %! root = main.tex comment.",
    },
    RuleInfo {
        code: "PRJ004",
        name: "orphan tex",
        default_severity: Severity::Warning,
        summary: "A .tex file in the project directory is not reachable from the resolved root document.",
        why: "Orphan files are often stale drafts or accidentally added sources that will not be compiled.",
        fix: "Include the file from the root document or remove the stray .tex file.",
    },
    RuleInfo {
        code: "REF001",
        name: "missing reference target",
        default_severity: Severity::Error,
        summary: "A reference command points to a label that is not defined in reachable TeX sources.",
        why: "Missing reference targets usually become unresolved references in the rendered PDF and often indicate a typo or missing included file.",
        fix: "Add the matching \\label{...}, fix the reference key, or make sure the file defining the label is reachable from the project root.",
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
        code: "SEC003",
        name: "singleton subdivision",
        default_severity: Severity::Warning,
        summary: "A section or subsection has exactly one direct child subdivision.",
        why: "A single subdivision usually means the outline can be flattened or needs another peer subdivision.",
        fix: "Merge the subdivision into the parent or add a peer subdivision.",
    },
    RuleInfo {
        code: "SEC004",
        name: "stacked headings",
        default_severity: Severity::Warning,
        summary: "Two section headings appear without meaningful text between them.",
        why: "Stacked headings often indicate an outline placeholder or a section that should be flattened.",
        fix: "Add introductory text between the headings or remove the extra heading.",
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
        code: "TAB002",
        name: "table label missing",
        default_severity: Severity::Warning,
        summary: "A table-like float has a caption but no label.",
        why: "Unlabeled tables cannot be referenced robustly and often lead authors to hard-code table numbers.",
        fix: "Add a \\label{tab:...} near the table caption.",
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
        code: "TXT003",
        name: "long sentence",
        default_severity: Severity::Warning,
        summary: "A prose sentence exceeds the configured word-count threshold.",
        why: "Very long sentences are harder to read and often hide multiple ideas that should be split.",
        fix: "Split the sentence or rewrite it more concisely.",
    },
    RuleInfo {
        code: "TXT004",
        name: "filler word",
        default_severity: Severity::Warning,
        summary: "The prose contains a common filler or weasel word.",
        why: "Filler words weaken scientific writing and are rarely needed in paper prose.",
        fix: "Remove the filler word or replace it with more precise wording.",
    },
    RuleInfo {
        code: "TXT005",
        name: "passive voice",
        default_severity: Severity::Warning,
        summary: "The prose may use passive-voice phrasing.",
        why: "Active voice is often clearer in scientific writing, though passive voice is sometimes appropriate.",
        fix: "Rewrite with an explicit subject if active voice improves clarity.",
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
    use super::{
        all_graph_project_rules, all_project_rules, all_rules, find_rule_info, rule_infos,
        GraphProjectRule, ProjectRule, Rule,
    };

    #[test]
    fn rule_registry_contains_rules() {
        let codes: Vec<_> = all_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec![
                "ENV001", "FMT001", "FMT002", "SEC001", "SEC002", "SEC003", "SEC004", "MTH001",
                "MTH002", "LAT002", "TEX001", "TXT001", "TXT002", "TXT003", "TXT004", "TXT005",
                "CMT001", "WS001"
            ]
        );
    }

    #[test]
    fn project_rule_registry_contains_figures_and_tables_rules() {
        let codes: Vec<_> = all_project_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec![
                "FIG001", "CAP001", "CAP002", "FIG002", "FIG003", "FIG004", "FIG005", "FIG006",
                "TAB001", "TAB002", "LAT001", "REF001", "LBL001"
            ]
        );
    }

    #[test]
    fn graph_rule_registry_contains_project_rules() {
        let codes: Vec<_> = all_graph_project_rules()
            .iter()
            .map(|rule| rule.code())
            .collect();
        assert_eq!(codes, vec!["PRJ001", "PRJ002", "PRJ003", "PRJ004"]);
    }

    fn assert_rule_trait_object(_: &dyn Rule) {}

    fn assert_project_rule_trait_object(_: &dyn ProjectRule) {}

    fn assert_graph_project_rule_trait_object(_: &dyn GraphProjectRule) {}

    #[test]
    fn registry_rules_are_trait_objects() {
        assert_rule_trait_object(all_rules()[0]);
        assert_project_rule_trait_object(all_project_rules()[0]);
        assert_graph_project_rule_trait_object(all_graph_project_rules()[0]);
    }

    #[test]
    fn rule_info_catalog_contains_all_known_codes() {
        let codes: Vec<_> = rule_infos().iter().map(|rule| rule.code).collect();

        assert_eq!(
            codes,
            vec![
                "CMT001", "CAP001", "CAP002", "BIB001", "CIT001", "CIT002", "CIT003", "CIT004",
                "CIT005", "CIT006", "CIT007", "CIT008", "CIT009", "CIT010", "ENV001", "FIG001",
                "FIG002", "FIG003", "FIG004", "FIG005", "FIG006", "FMT001", "FMT002", "LBL001",
                "LAT001", "LAT002", "MTH001", "MTH002", "PRJ001", "PRJ002", "PRJ003", "PRJ004",
                "REF001", "SEC001", "SEC002", "SEC003", "SEC004", "TAB001", "TAB002", "TEX001",
                "TXT001", "TXT002", "TXT003", "TXT004", "TXT005", "WS001"
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
