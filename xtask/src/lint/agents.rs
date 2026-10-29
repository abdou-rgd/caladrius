//! The agent files and the size of the contract (`AGENTS.md` sections 8 and 9).
//!
//! An agent file is `.claude/agents/<role>.md`, a direct child of that folder. The orchestrator is the
//! main session and has no file.
//!
//! - `agents-size`: an agent file has at most 200 lines, so that it is read whole.
//! - `agents-front-matter`: the file opens with a `---` front matter that gives `name` (the file name),
//!   `description`, `model`, `effort` and `tools`, the model and the effort among the values Claude Code knows.
//! - `agents-structure`: the body opens with `You are`, carries the motto, has the headings `## What you do`,
//!   `## How` and `## Report` in this order, and the report asks for a bolder next step.
//! - `agents-table`: the table of section 8 of `AGENTS.md` has a row for every agent file, a file for every
//!   role but the orchestrator, and the same model and effort on both sides.
//! - `contract-size`: `AGENTS.md` has fewer than 200 lines, so that every agent reads it whole.

use std::collections::BTreeMap;

use super::{Finding, Tree, section};

const AGENTS_DIR: &str = ".claude/agents/";
const CONTRACT: &str = "AGENTS.md";

/// The longest an agent file may be.
const MAX_AGENT_LINES: usize = 200;
/// `AGENTS.md` stays under this many lines.
const CONTRACT_LINES: usize = 200;

const SIZE: &str = "agents-size";
const FRONT_MATTER: &str = "agents-front-matter";
const STRUCTURE: &str = "agents-structure";
const TABLE: &str = "agents-table";

/// The keys of the front matter, all required.
const KEYS: [&str; 5] = ["name", "description", "model", "effort", "tools"];
const MODELS: [&str; 4] = ["haiku", "sonnet", "opus", "fable"];
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

const MOTTO: &str = "**Motto: we should be more ambitious.**";
/// The headings every agent file has, in this order.
const HEADINGS: [&str; 3] = ["## What you do", "## How", "## Report"];

/// One `.claude/agents/<role>.md`.
struct Agent<'a> {
    path: &'a str,
    /// The file name without `.md`.
    role: &'a str,
    text: &'a str,
    front: Option<FrontMatter<'a>>,
}

/// The `key: value` lines between the two `---` lines.
struct FrontMatter<'a> {
    /// Each key with the 1-based line of its first occurrence and its trimmed value.
    values: BTreeMap<&'a str, (usize, &'a str)>,
    /// 1-based line of the closing `---`.
    end: usize,
}

impl<'a> Agent<'a> {
    fn finding(&self, line: usize, rule: &'static str, message: impl Into<String>) -> Finding {
        Finding::new(self.path, line, rule, message, "")
    }

    fn value(&self, key: &str) -> Option<&'a str> {
        let front = self.front.as_ref()?;
        front.values.get(key).map(|(_, value)| *value)
    }
}

pub fn check(tree: &Tree) -> Vec<Finding> {
    let agents = agent_files(tree);
    let mut findings = Vec::new();
    for agent in &agents {
        findings.extend(check_size(agent));
        findings.extend(check_front_matter(agent));
        findings.extend(check_structure(agent));
    }
    findings.extend(check_table(tree, &agents));
    findings.extend(check_contract_size(tree));
    findings
}

fn agent_files(tree: &Tree) -> Vec<Agent<'_>> {
    tree.files
        .iter()
        .filter_map(|(path, text)| {
            let role = path.strip_prefix(AGENTS_DIR)?.strip_suffix(".md")?;
            (!role.is_empty() && !role.contains('/')).then(|| Agent {
                path,
                role,
                text,
                front: front_matter(text),
            })
        })
        .collect()
}

/// `None` when the text does not start with `---` or never closes it.
fn front_matter(text: &str) -> Option<FrontMatter<'_>> {
    let mut lines = text.lines().enumerate();
    let (_, first) = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let mut values = BTreeMap::new();
    for (index, line) in lines {
        if line.trim_end() == "---" {
            return Some(FrontMatter {
                values,
                end: index + 1,
            });
        }
        // An indented line continues the value above it.
        if let Some((key, value)) = line
            .split_once(':')
            .filter(|_| !line.starts_with([' ', '\t']))
        {
            values
                .entry(key.trim())
                .or_insert((index + 1, value.trim()));
        }
    }
    None
}

fn check_size(agent: &Agent) -> Option<Finding> {
    let count = agent.text.lines().count();
    (count > MAX_AGENT_LINES).then(|| {
        agent.finding(
            MAX_AGENT_LINES + 1,
            SIZE,
            format!("{count} lines, an agent file has at most {MAX_AGENT_LINES}"),
        )
    })
}

fn check_front_matter(agent: &Agent) -> Vec<Finding> {
    let Some(front) = &agent.front else {
        return vec![agent.finding(
            1,
            FRONT_MATTER,
            "the file must start with a `---` line and close its front matter with another `---` line",
        )];
    };
    let mut findings = Vec::new();
    for key in KEYS {
        match front.values.get(key) {
            None => findings.push(agent.finding(
                1,
                FRONT_MATTER,
                format!("the front matter has no `{key}:`"),
            )),
            Some((line, value)) => {
                if let Some(problem) = problem_with(key, value, agent.role) {
                    findings.push(agent.finding(*line, FRONT_MATTER, problem));
                }
            }
        }
    }
    findings
}

/// What is wrong with the value of a front matter key, if anything.
fn problem_with(key: &str, value: &str, role: &str) -> Option<String> {
    match key {
        "name" => (value != role).then(|| format!("`name` is `{value}`, the file is `{role}.md`")),
        "model" => (!MODELS.contains(&value)).then(|| {
            format!(
                "`model` is `{value}`, one of {} is expected",
                MODELS.join(", ")
            )
        }),
        "effort" => (!EFFORTS.contains(&value)).then(|| {
            format!(
                "`effort` is `{value}`, one of {} is expected",
                EFFORTS.join(", ")
            )
        }),
        _ => value.is_empty().then(|| format!("`{key}` is empty")),
    }
}

fn check_structure(agent: &Agent) -> Vec<Finding> {
    let mut findings = Vec::new();
    // `end` is the 1-based line of the closing `---`, so it is also the 0-based index of the next line.
    let body = agent.front.as_ref().map_or(0, |front| front.end);
    let opening = agent
        .text
        .lines()
        .enumerate()
        .skip(body)
        .find(|(_, line)| !line.trim().is_empty());
    match opening {
        Some((_, line)) if line.starts_with("You are") => {}
        Some((index, _)) => findings.push(agent.finding(
            index + 1,
            STRUCTURE,
            "the body must open with `You are ...`",
        )),
        None => findings.push(agent.finding(
            1,
            STRUCTURE,
            "the file has no body, it must open with `You are ...`",
        )),
    }
    if !agent.text.lines().any(|line| line.starts_with(MOTTO)) {
        findings.push(agent.finding(1, STRUCTURE, format!("no line starts with `{MOTTO}`")));
    }
    findings.extend(check_headings(agent));
    findings.extend(check_report(agent));
    findings
}

fn check_headings(agent: &Agent) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut lines = Vec::new();
    for heading in HEADINGS {
        match agent
            .text
            .lines()
            .position(|line| line.trim_end() == heading)
        {
            None => findings.push(agent.finding(1, STRUCTURE, format!("no `{heading}` heading"))),
            Some(index) => lines.push(index + 1),
        }
    }
    // Only the headings that exist are compared, so a missing one is reported once.
    let too_early = lines
        .iter()
        .zip(lines.iter().skip(1))
        .find(|(before, after)| before > after);
    if let Some((_, line)) = too_early {
        findings.push(agent.finding(
            *line,
            STRUCTURE,
            format!("the headings come in this order: {}", HEADINGS.join(", ")),
        ));
    }
    findings
}

/// The report is what the orchestrator reads, so it is where the bolder next step is asked for.
fn check_report(agent: &Agent) -> Option<Finding> {
    // Without the heading, `check_headings` has already said so.
    let report = section(agent.text, |line| line.trim_end() == "## Report")?;
    let asks = report
        .body
        .iter()
        .any(|(_, line)| line.to_lowercase().contains("bolder next step"));
    (!asks).then(|| {
        agent.finding(
            report.line,
            STRUCTURE,
            "the Report does not ask for a bolder next step",
        )
    })
}

/// A row of the role table: a role in backticks, then `<Model>, <effort>`.
struct Row<'a> {
    line: usize,
    text: &'a str,
    role: &'a str,
    /// The second cell, `Opus, high`.
    model_effort: &'a str,
}

fn parse_row(line: usize, text: &str) -> Option<Row<'_>> {
    if !text.starts_with("| `") {
        return None;
    }
    let mut cells = text
        .trim()
        .trim_start_matches('|')
        .split('|')
        .map(str::trim);
    Some(Row {
        line,
        text,
        role: cells.next()?.trim_matches('`'),
        model_effort: cells.next().unwrap_or_default(),
    })
}

fn check_table(tree: &Tree, agents: &[Agent]) -> Vec<Finding> {
    let Some(contract) = tree.files.get(CONTRACT) else {
        return vec![Finding::new(
            CONTRACT,
            1,
            TABLE,
            "AGENTS.md is missing, so the role table cannot be checked",
            "",
        )];
    };
    let Some(roles) = section(contract, |line| line.starts_with("## 8.")) else {
        return vec![Finding::new(
            CONTRACT,
            1,
            TABLE,
            "AGENTS.md has no section 8 (`## 8. ...`) with the role table",
            "",
        )];
    };
    let rows: Vec<Row> = roles
        .body
        .iter()
        .filter_map(|(line, text)| parse_row(*line, text))
        .collect();
    let mut findings = Vec::new();
    // The orchestrator is the main session: it has a row and no file.
    for row in rows.iter().filter(|row| row.role != "orchestrator") {
        findings.extend(check_row(row, agents));
    }
    for agent in agents {
        if !rows.iter().any(|row| row.role == agent.role) {
            findings.push(agent.finding(
                1,
                TABLE,
                "no row for this role in the table of AGENTS.md section 8",
            ));
        }
    }
    findings
}

/// One row against the file of its role.
fn check_row(row: &Row, agents: &[Agent]) -> Vec<Finding> {
    let role = row.role;
    let finding = |message: String| Finding::new(CONTRACT, row.line, TABLE, message, row.text);
    let Some((model, effort)) = row.model_effort.split_once(',') else {
        return vec![finding(format!(
            "`{role}`: the second cell `{}` must read `<Model>, <effort>`",
            row.model_effort
        ))];
    };
    let Some(agent) = agents.iter().find(|agent| agent.role == role) else {
        return vec![finding(format!(
            "`{role}` has no file {AGENTS_DIR}{role}.md"
        ))];
    };
    let (model, effort) = (model.trim().to_lowercase(), effort.trim().to_lowercase());
    let mut findings = Vec::new();
    for (key, expected) in [("model", model), ("effort", effort)] {
        // A file without the key is already a finding of `agents-front-matter`.
        if let Some(actual) = agent.value(key).filter(|actual| *actual != expected) {
            findings.push(finding(format!(
                "`{role}`: {key} is `{expected}` here and `{actual}` in {}",
                agent.path
            )));
        }
    }
    findings
}

fn check_contract_size(tree: &Tree) -> Option<Finding> {
    // A missing contract is reported once, by `check_table`.
    let count = tree.files.get(CONTRACT)?.lines().count();
    (count >= CONTRACT_LINES).then(|| {
        Finding::new(
            CONTRACT,
            CONTRACT_LINES,
            "contract-size",
            format!("{count} lines, the contract stays under {CONTRACT_LINES} so that every agent reads it whole"),
            "",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    /// A well-formed agent file: the front matter is lines 1 to 7, the opening line 9, the motto 11,
    /// `## What you do` 13, `## How` 17 and `## Report` 23.
    const ENGINE: &str = "---\nname: engine\ndescription: Numerical code.\nmodel: opus\neffort: high\ntools: Read, Edit\n---\n\nYou are the `engine` agent.\n\n**Motto: we should be more ambitious.** Never claim more than the tests show.\n\n## What you do\n\n- the numerical crates\n\n## How\n\n### Steps\n\n1. read AGENTS.md\n\n## Report\n\nAt most ten lines, ending with one bolder next step.\n";

    /// Section 8 on lines 5 to 13 (the rows on lines 9 and 10), and a row outside it on line 16.
    const CONTRACT_TEXT: &str = "# AGENTS.md\n\n## 7. UI\n\n## 8. Agent organization\n\n| Role | Model, effort | Does |\n|---|---|---|\n| `orchestrator` | Opus, max | plans |\n| `engine` | Opus, high | numbers |\n\nRules:\n\n## 9. The board\n\n| `ghost` | Haiku, low | outside section 8 |\n";

    fn tree(files: &[(&str, &str)]) -> Tree {
        fixture(files, &[])
    }

    /// The findings on the contract and one engine file.
    fn findings_for(contract: &str, engine: &str) -> Vec<Finding> {
        check(&tree(&[
            (CONTRACT, contract),
            (".claude/agents/engine.md", engine),
        ]))
    }

    fn engine_with(from: &str, to: &str) -> String {
        assert!(ENGINE.contains(from), "fixture lost `{from}`");
        ENGINE.replace(from, to)
    }

    fn contract_with(from: &str, to: &str) -> String {
        assert!(CONTRACT_TEXT.contains(from), "fixture lost `{from}`");
        CONTRACT_TEXT.replace(from, to)
    }

    /// `(file, line)` of the findings of one rule.
    fn places(findings: &[Finding], rule: &str) -> Vec<(String, usize)> {
        findings
            .iter()
            .filter(|finding| finding.rule == rule)
            .map(|finding| (finding.file.clone(), finding.line))
            .collect()
    }

    fn engine_place(line: usize) -> (String, usize) {
        (".claude/agents/engine.md".to_owned(), line)
    }

    fn rules(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(|finding| finding.rule).collect()
    }

    #[test]
    fn a_consistent_contract_and_agent_file_give_no_finding() {
        assert!(findings_for(CONTRACT_TEXT, ENGINE).is_empty());
    }

    #[test]
    fn only_direct_children_named_md_are_agent_files() {
        let files = [
            (CONTRACT, CONTRACT_TEXT),
            (".claude/agents/engine.md", ENGINE),
            (".claude/agents/notes/x.md", "no front matter\n"),
            (".claude/agents/README.txt", "no front matter\n"),
            (".claude/agents/.md", "no front matter\n"),
            (".claude/other/y.md", "no front matter\n"),
        ];
        assert!(check(&tree(&files)).is_empty());
    }

    #[test]
    fn an_agent_file_has_at_most_200_lines() {
        let engine_lines = ENGINE.lines().count();
        let padded = |lines: usize| format!("{ENGINE}{}", "more\n".repeat(lines - engine_lines));
        assert!(findings_for(CONTRACT_TEXT, &padded(200)).is_empty());
        let findings = findings_for(CONTRACT_TEXT, &padded(201));
        assert_eq!(places(&findings, "agents-size"), [engine_place(201)]);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn a_file_without_a_closed_front_matter_is_a_finding() {
        let without = ENGINE.replacen("---\n", "", 1);
        let unclosed = ENGINE.replacen("\n---\n", "\n", 1);
        for text in [without.as_str(), unclosed.as_str()] {
            let findings = findings_for(CONTRACT_TEXT, text);
            assert_eq!(places(&findings, "agents-front-matter"), [engine_place(1)]);
        }
    }

    #[test]
    fn each_key_of_the_front_matter_is_checked() {
        for (from, to, line, word) in [
            ("name: engine", "name: motor", 2, "motor"),
            (
                "description: Numerical code.",
                "description:",
                3,
                "description",
            ),
            ("model: opus", "model: gpt", 4, "gpt"),
            ("effort: high", "effort: huge", 5, "huge"),
            ("tools: Read, Edit", "tools:   ", 6, "tools"),
        ] {
            let findings = findings_for(CONTRACT_TEXT, &engine_with(from, to));
            let ours: Vec<&Finding> = findings
                .iter()
                .filter(|finding| finding.rule == "agents-front-matter")
                .collect();
            assert_eq!(ours.len(), 1, "{word}");
            assert_eq!(ours[0].line, line, "{word}");
            assert!(ours[0].message.contains(word), "{word}");
        }
    }

    #[test]
    fn a_missing_key_is_reported_once_on_the_first_line() {
        let findings = findings_for(CONTRACT_TEXT, &engine_with("tools: Read, Edit\n", ""));
        assert_eq!(places(&findings, "agents-front-matter"), [engine_place(1)]);
        assert!(findings[0].message.contains("`tools:`"));
    }

    #[test]
    fn every_model_and_effort_of_claude_code_is_accepted() {
        for model in MODELS {
            for effort in EFFORTS {
                let text = engine_with("model: opus", &format!("model: {model}"))
                    .replace("effort: high", &format!("effort: {effort}"));
                let findings = findings_for(CONTRACT_TEXT, &text);
                assert!(places(&findings, "agents-front-matter").is_empty());
            }
        }
    }

    #[test]
    fn an_indented_line_continues_the_value_above_it() {
        let text = engine_with(
            "description: Numerical code.\n",
            "description: Numerical code,\n  model: gpt\n",
        );
        assert!(
            findings_for(CONTRACT_TEXT, &text)
                .iter()
                .all(|f| f.line != 4)
        );
    }

    #[test]
    fn the_body_opens_with_you_are_and_carries_the_motto() {
        let opening = engine_with("You are the `engine` agent.", "Hello.");
        let findings = findings_for(CONTRACT_TEXT, &opening);
        assert_eq!(places(&findings, "agents-structure"), [engine_place(9)]);
        let motto = engine_with("**Motto: we should be more ambitious.**", "**Motto: go.**");
        let findings = findings_for(CONTRACT_TEXT, &motto);
        assert_eq!(places(&findings, "agents-structure"), [engine_place(1)]);
        assert!(findings[0].message.contains("Motto"));
    }

    #[test]
    fn a_file_with_a_front_matter_and_nothing_else_has_no_body() {
        let header = ENGINE.lines().take(7).collect::<Vec<_>>().join("\n");
        let findings = findings_for(CONTRACT_TEXT, &header);
        assert!(findings.iter().any(|f| f.message.contains("no body")));
    }

    #[test]
    fn the_three_headings_exist() {
        for heading in ["What you do", "How", "Report"] {
            let text = engine_with(&format!("## {heading}\n"), "## Elsewhere\n");
            let findings = findings_for(CONTRACT_TEXT, &text);
            let missing: Vec<&Finding> = findings
                .iter()
                .filter(|f| f.message.contains(&format!("no `## {heading}` heading")))
                .collect();
            assert_eq!(missing.len(), 1, "{heading}");
        }
    }

    #[test]
    fn the_three_headings_come_in_order() {
        // `## How` and `## Report` swap places; `### Steps` stays under the first of them.
        let text = engine_with("## How", "@@")
            .replace("## Report", "## How")
            .replace("@@", "## Report");
        let findings = findings_for(CONTRACT_TEXT, &text);
        let order: Vec<&Finding> = findings
            .iter()
            .filter(|f| f.message.contains("in this order"))
            .collect();
        assert_eq!(order.len(), 1);
        assert_eq!(order[0].line, 17);
    }

    #[test]
    fn a_subheading_between_two_headings_is_fine() {
        let text = engine_with("## How\n", "## How\n\n### First\n\n### Second\n");
        assert!(findings_for(CONTRACT_TEXT, &text).is_empty());
    }

    #[test]
    fn the_report_asks_for_a_bolder_next_step_in_any_case() {
        let none = engine_with("ending with one bolder next step", "and nothing else");
        let findings = findings_for(CONTRACT_TEXT, &none);
        assert_eq!(places(&findings, "agents-structure"), [engine_place(23)]);
        let capitals = engine_with("one bolder next step", "One Bolder Next Step");
        assert!(findings_for(CONTRACT_TEXT, &capitals).is_empty());
        // Said under `## How`, it is not in the report.
        let elsewhere = engine_with("1. read AGENTS.md", "1. propose a bolder next step")
            .replace("ending with one bolder next step", "and nothing else");
        let findings = findings_for(CONTRACT_TEXT, &elsewhere);
        assert_eq!(places(&findings, "agents-structure"), [engine_place(23)]);
    }

    #[test]
    fn the_model_and_the_effort_of_a_row_are_those_of_the_file() {
        let model = engine_with("model: opus", "model: sonnet");
        let findings = findings_for(CONTRACT_TEXT, &model);
        assert_eq!(
            places(&findings, "agents-table"),
            [(CONTRACT.to_owned(), 10)]
        );
        assert!(
            findings[0]
                .message
                .contains("model is `opus` here and `sonnet`")
        );
        let effort = engine_with("effort: high", "effort: max");
        let findings = findings_for(CONTRACT_TEXT, &effort);
        assert_eq!(
            places(&findings, "agents-table"),
            [(CONTRACT.to_owned(), 10)]
        );
        assert!(
            findings[0]
                .message
                .contains("effort is `high` here and `max`")
        );
        // Both differ: two findings on the row.
        let both = model.replace("effort: high", "effort: max");
        assert_eq!(
            places(&findings_for(CONTRACT_TEXT, &both), "agents-table").len(),
            2
        );
    }

    #[test]
    fn the_cell_is_compared_lowercased_and_trimmed() {
        let contract = contract_with("| Opus, high |", "|OPUS,   High |");
        assert!(findings_for(&contract, ENGINE).is_empty());
    }

    #[test]
    fn a_cell_that_is_not_model_comma_effort_is_a_finding() {
        let contract = contract_with("| Opus, high |", "| Opus |");
        let findings = findings_for(&contract, ENGINE);
        assert_eq!(
            places(&findings, "agents-table"),
            [(CONTRACT.to_owned(), 10)]
        );
        assert!(findings[0].message.contains("<Model>, <effort>"));
    }

    #[test]
    fn a_role_of_the_table_has_a_file() {
        let contract = contract_with(
            "| `engine` | Opus, high | numbers |\n",
            "| `engine` | Opus, high | numbers |\n| `reader` | Sonnet, high | specs |\n",
        );
        let findings = findings_for(&contract, ENGINE);
        assert_eq!(
            places(&findings, "agents-table"),
            [(CONTRACT.to_owned(), 11)]
        );
        assert!(findings[0].message.contains(".claude/agents/reader.md"));
    }

    #[test]
    fn a_file_has_a_row_in_the_table() {
        let janitor = ENGINE.replace("engine", "janitor");
        let files = [
            (CONTRACT, CONTRACT_TEXT),
            (".claude/agents/engine.md", ENGINE),
            (".claude/agents/janitor.md", janitor.as_str()),
        ];
        let findings = check(&tree(&files));
        assert_eq!(rules(&findings), ["agents-table"]);
        assert_eq!(findings[0].file, ".claude/agents/janitor.md");
    }

    #[test]
    fn the_orchestrator_has_a_row_and_no_file() {
        // Nothing is asked of the model and the effort of a file that the table lists for it.
        let with_file = [
            (CONTRACT, CONTRACT_TEXT),
            (".claude/agents/engine.md", ENGINE),
            (".claude/agents/orchestrator.md", "---\n"),
        ];
        assert!(places(&check(&tree(&with_file)), "agents-table").is_empty());
    }

    #[test]
    fn rows_outside_section_8_are_not_read() {
        // CONTRACT_TEXT holds a `ghost` row under section 9, with no file: no finding.
        assert!(findings_for(CONTRACT_TEXT, ENGINE).is_empty());
    }

    #[test]
    fn a_missing_contract_or_section_8_is_one_finding() {
        let alone = check(&tree(&[(".claude/agents/engine.md", ENGINE)]));
        assert_eq!(rules(&alone), ["agents-table"]);
        assert_eq!(alone[0].file, CONTRACT);
        let findings = findings_for("# AGENTS.md\n\n## 9. The board\n", ENGINE);
        assert_eq!(rules(&findings), ["agents-table"]);
        assert!(findings[0].message.contains("section 8"));
    }

    #[test]
    fn the_contract_has_fewer_than_200_lines() {
        let padded = |lines: usize| {
            let missing = lines - CONTRACT_TEXT.lines().count();
            format!("{CONTRACT_TEXT}{}", "more\n".repeat(missing))
        };
        assert!(findings_for(&padded(199), ENGINE).is_empty());
        let findings = findings_for(&padded(200), ENGINE);
        assert_eq!(
            places(&findings, "contract-size"),
            [(CONTRACT.to_owned(), 200)]
        );
        assert_eq!(findings.len(), 1);
    }
}
