//! `cargo xtask lint`: the mechanical rules of `AGENTS.md` as a deterministic gate.
//!
//! Every rule works on a [`Tree`] (the versioned files read into memory), so each one is tested on
//! small fixtures without touching the file system. A finding prints as `file:line rule-id message`.
//! Accepted exceptions live in `xtask/lint_allow.toml`, each with a reason; an exception that no
//! longer matches anything is itself a finding. A last step runs clippy with `indexing_slicing` denied on
//! the numerical crates ([`indexing`]).
//!
//! The contract and the board follow the same gate: [`board`] checks the cards and the iterations
//! (`AGENTS.md` section 9), [`agents`] checks the agent files, the role table of section 8 and the
//! size of the contract itself.

mod agents;
mod allow;
mod board;
mod forbidden;
mod header;
mod indexing;
mod private;
mod tolerance;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::console;
use crate::error::{Result, XtaskError};
use crate::workspace;

/// Where the accepted exceptions are kept, relative to the workspace root.
const ALLOW_FILE: &str = "xtask/lint_allow.toml";

/// One violation of one rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Path relative to the workspace root, with `/` separators.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// Stable rule id, also used by the allow-list.
    pub rule: &'static str,
    pub message: String,
    /// The offending line, matched by the `contains` filter of the allow-list (never printed).
    pub text: String,
}

impl Finding {
    pub fn new(
        file: &str,
        line: usize,
        rule: &'static str,
        message: impl Into<String>,
        text: &str,
    ) -> Self {
        Self {
            file: file.to_owned(),
            line,
            rule,
            message: message.into(),
            text: text.to_owned(),
        }
    }

    fn render(&self) -> String {
        format!("{}:{} {} {}", self.file, self.line, self.rule, self.message)
    }
}

/// The repository as the rules see it.
#[derive(Debug, Default)]
pub struct Tree {
    /// Text files that are tracked or not ignored, by relative path.
    pub files: BTreeMap<String, String>,
    /// Paths tracked by git (`git ls-files`), whatever their content.
    pub tracked: Vec<String>,
    /// Member directories of the Cargo workspace.
    pub members: Vec<String>,
}

/// A Markdown section: the lines under a heading, up to the next `## ` heading or the end of the text.
struct Section<'a> {
    /// 1-based line of the heading.
    line: usize,
    /// The lines under the heading, each with its 1-based line number.
    body: Vec<(usize, &'a str)>,
}

impl<'a> Section<'a> {
    /// The first line of the body that starts with `prefix`.
    fn find(&self, prefix: &str) -> Option<(usize, &'a str)> {
        self.body
            .iter()
            .copied()
            .find(|(_, line)| line.starts_with(prefix))
    }
}

/// The first section whose heading line `is_heading` accepts. Both the board and the agent files are
/// read by their sections, so the cut between two sections is decided once, here.
fn section<'a>(text: &'a str, is_heading: impl Fn(&str) -> bool) -> Option<Section<'a>> {
    let mut lines = text.lines().enumerate();
    let (heading, _) = lines.find(|(_, line)| is_heading(line))?;
    let body = lines
        .take_while(|(_, line)| !line.starts_with("## "))
        .map(|(index, line)| (index + 1, line))
        .collect();
    Some(Section {
        line: heading + 1,
        body,
    })
}

/// Runs every rule, then removes the findings accepted by the allow-list.
pub fn check(tree: &Tree, allow_text: &str) -> Result<Vec<Finding>> {
    let allow = allow::parse(allow_text)?;
    let mut findings = Vec::new();
    findings.extend(forbidden::check(tree));
    findings.extend(private::check(tree));
    findings.extend(header::check(tree));
    findings.extend(tolerance::check(tree));
    findings.extend(board::check(tree));
    findings.extend(agents::check(tree));
    let mut findings = allow::apply(&allow, findings, ALLOW_FILE);
    findings.sort_by(|a, b| (&a.file, a.line, a.rule).cmp(&(&b.file, b.line, b.rule)));
    Ok(findings)
}

/// `cargo xtask lint`: exit status 1 (through the error) when there is any finding.
pub fn run() -> Result<()> {
    let root = workspace::root()?;
    let tree = load_tree(&root)?;
    let allow_path = root.join(ALLOW_FILE);
    let allow_text = fs::read_to_string(&allow_path)
        .map_err(|source| XtaskError::io("read", &allow_path, &source))?;
    let findings = check(&tree, &allow_text)?;
    if findings.is_empty() {
        console::out(&format!("lint: ok ({} files checked)", tree.files.len()));
    } else {
        for finding in &findings {
            console::out(&finding.render());
        }
    }
    // The indexing step always runs, so one invocation reports every kind of finding.
    let indexing = indexing::run();
    if !findings.is_empty() {
        return Err(XtaskError::new(format!(
            "lint: {} finding(s)",
            findings.len()
        )));
    }
    indexing
}

/// Lists the files with git (tracked, plus untracked files that are not ignored) and reads the text ones.
pub fn load_tree(root: &Path) -> Result<Tree> {
    let tracked = git_files(root, &["--cached"])?;
    let candidates = git_files(root, &["--cached", "--others", "--exclude-standard"])?;
    let mut files = BTreeMap::new();
    for path in candidates {
        // A file deleted from the work tree but still in the index, or a file that is not UTF-8
        // (an image), has no text to check.
        if let Ok(bytes) = fs::read(root.join(&path)) {
            if let Ok(text) = String::from_utf8(bytes) {
                files.insert(path, text);
            }
        }
    }
    let members = workspace::load(root)?
        .members
        .into_iter()
        .map(|member| member.dir)
        .collect();
    Ok(Tree {
        files,
        tracked,
        members,
    })
}

fn git_files(root: &Path, flags: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .args(flags)
        .output()
        .map_err(|source| XtaskError::new(format!("cannot run git: {source}")))?;
    if !output.status.success() {
        return Err(XtaskError::new(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let mut paths: Vec<String> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// A tree built from `(path, content)` pairs, for the rule tests. Every file is also tracked.
#[cfg(test)]
pub fn fixture(files: &[(&str, &str)], members: &[&str]) -> Tree {
    Tree {
        files: files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect(),
        tracked: files.iter().map(|(path, _)| (*path).to_owned()).collect(),
        members: members.iter().map(|dir| (*dir).to_owned()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finding_prints_as_file_line_rule_message() {
        let finding = Finding::new("a/b.md", 12, "some-rule", "what is wrong", "text");
        assert_eq!(finding.render(), "a/b.md:12 some-rule what is wrong");
    }

    #[test]
    fn findings_are_sorted_by_file_then_line() {
        let tree = fixture(
            &[("z.md", "x\nWinNonlin\n"), ("a.md", "Certara\n\nPhoenix\n")],
            &[],
        );
        let findings = check(&tree, "").unwrap_or_default();
        // The tree has no AGENTS.md, which is a finding of its own, and not the one under test.
        let places: Vec<(String, usize)> = findings
            .iter()
            .filter(|finding| finding.rule == "forbidden-name")
            .map(|finding| (finding.file.clone(), finding.line))
            .collect();
        assert_eq!(
            places,
            [
                ("a.md".to_owned(), 1),
                ("a.md".to_owned(), 3),
                ("z.md".to_owned(), 2)
            ]
        );
    }

    #[test]
    fn the_agent_rules_run_with_the_others() {
        // No AGENTS.md at all: only `agents::check` can say so.
        let findings = check(&fixture(&[("a.md", "x\n")], &[]), "").unwrap_or_default();
        let rules: Vec<&str> = findings.iter().map(|finding| finding.rule).collect();
        assert_eq!(rules, ["agents-table"]);
    }

    #[test]
    fn a_section_runs_from_its_heading_to_the_next_level_two_heading() {
        let text = "intro\n## One\na\n### Sub\nb\n## Two\nc\n";
        let one = section(text, |line| line == "## One");
        assert_eq!(one.as_ref().map(|s| s.line), Some(2));
        assert_eq!(
            one.map(|s| s.body),
            Some(vec![(3, "a"), (4, "### Sub"), (5, "b")])
        );
        // The last section runs to the end of the text; an absent heading gives no section.
        assert_eq!(
            section(text, |line| line == "## Two").map(|s| s.body),
            Some(vec![(7, "c")])
        );
        assert!(section(text, |line| line == "## Three").is_none());
    }

    #[test]
    fn the_allow_list_of_the_repository_parses() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("lint_allow.toml");
        let text = fs::read_to_string(&path).unwrap_or_default();
        assert!(!text.is_empty());
        assert!(allow::parse(&text).is_ok());
    }
}
