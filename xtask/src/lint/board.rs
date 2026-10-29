//! Board hygiene (`AGENTS.md` section 9).
//!
//! - `board-state`: every `board/tasks/T-*.md` has a `- **State:** <state>` line in its header and the
//!   state is one of the allowed ones (the five of the contract, plus `split` for an umbrella card).
//! - `board-index`: every card has a row in `board/INDEX.md` with the same state, and every row has a card.
//! - `board-log`: every non-empty line of `board/LOG.md` that is not a heading starts with `- YYYY-MM-DD `.
//! - `board-iteration`: an iteration is a card with `- **Kind:** iteration` (section 9, "one session = one
//!   iteration"), and `Kind` has no other value. Its title reads `# T-xxx: Iteration <n>: <name>`; its `## Brief`
//!   holds the five items of the brief; once it is `done`, its `## Outcome` holds a bolder next step and a link to
//!   the learning page, and none of its cards is still `todo`, `in progress` or `to review`. A card that says
//!   `- **Iteration:** T-xxx` must name an existing iteration card.

use std::collections::BTreeMap;

use super::{Finding, Tree, section};

/// The five states of `AGENTS.md` section 9, plus `split` (an umbrella card whose work moved to its sub-cards).
const STATES: [&str; 6] = [
    "todo",
    "in progress",
    "to review",
    "done",
    "blocked",
    "split",
];

const INDEX: &str = "board/INDEX.md";
const LOG: &str = "board/LOG.md";

/// The items the `## Brief` of an iteration states before any work (`AGENTS.md` section 9, step 1).
const BRIEF_ITEMS: [&str; 5] = [
    "**Goal.**",
    "**Deliverables.**",
    "**Claims touched.**",
    "**Done when.**",
    "**Plan.**",
];

/// The states in which a card of an iteration keeps it from being `done`.
const OPEN_STATES: [&str; 3] = ["todo", "in progress", "to review"];

pub fn check(tree: &Tree) -> Vec<Finding> {
    let mut findings = Vec::new();
    let index = parse_index(
        tree.files
            .get(INDEX)
            .map(String::as_str)
            .unwrap_or_default(),
    );
    let mut cards: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for (path, text) in &tree.files {
        let Some(id) = card_id(path) else { continue };
        match card_state(text) {
            None => findings.push(Finding::new(
                path,
                1,
                "board-state",
                "no `- **State:** <state>` line in the card header",
                "",
            )),
            Some((line, state)) if !STATES.contains(&state.as_str()) => {
                findings.push(Finding::new(
                    path,
                    line,
                    "board-state",
                    format!(
                        "`{state}` is not a valid state (one of: {})",
                        STATES.join(", ")
                    ),
                    &state,
                ));
            }
            Some((line, state)) => {
                cards.insert(id.to_owned(), (line, state));
            }
        }
    }
    for (id, (line, state)) in &cards {
        let path = format!("board/tasks/{id}.md");
        match index.get(id) {
            None => findings.push(Finding::new(
                &path,
                *line,
                "board-index",
                format!("{id} is not listed in {INDEX}"),
                "",
            )),
            Some((_, indexed, _)) if indexed != state => findings.push(Finding::new(
                &path,
                *line,
                "board-index",
                format!("state `{state}` differs from `{indexed}` in {INDEX}"),
                "",
            )),
            Some(_) => {}
        }
    }
    for (id, (line, _, text)) in &index {
        let has_card = tree.files.contains_key(&format!("board/tasks/{id}.md"));
        if !has_card {
            findings.push(Finding::new(
                INDEX,
                *line,
                "board-index",
                format!("{id} has no card in board/tasks/"),
                text,
            ));
        }
    }
    findings.extend(check_iterations(tree));
    findings.extend(check_log(
        tree.files.get(LOG).map(String::as_str).unwrap_or_default(),
    ));
    findings
}

/// `board/tasks/T-042.md` gives `T-042`.
fn card_id(path: &str) -> Option<&str> {
    let name = path.strip_prefix("board/tasks/")?.strip_suffix(".md")?;
    let rest = name.strip_prefix("T-")?;
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_lowercase());
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then_some(name)
}

/// The first `- **<name>:** ...` line of the card (1-based line, value without trailing dot).
fn header_field(text: &str, name: &str) -> Option<(usize, String)> {
    let marker = format!("- **{name}:**");
    text.lines().enumerate().find_map(|(index, line)| {
        let value = line.trim_start().strip_prefix(marker.as_str())?;
        Some((index + 1, value.trim().trim_end_matches('.').to_owned()))
    })
}

fn card_state(text: &str) -> Option<(usize, String)> {
    header_field(text, "State")
}

/// One card of the tree, for the iteration rules.
struct Card<'a> {
    path: &'a str,
    text: &'a str,
}

/// The cards by id.
type Cards<'a> = BTreeMap<&'a str, Card<'a>>;

impl Card<'_> {
    fn finding(&self, line: usize, message: impl Into<String>) -> Finding {
        Finding::new(self.path, line, "board-iteration", message, "")
    }

    fn is_iteration(&self) -> bool {
        header_field(self.text, "Kind").is_some_and(|(_, kind)| kind == "iteration")
    }
}

fn check_iterations(tree: &Tree) -> Vec<Finding> {
    let cards: Cards = tree
        .files
        .iter()
        .filter_map(|(path, text)| Some((card_id(path)?, Card { path, text })))
        .collect();
    let mut findings = Vec::new();
    for (id, card) in &cards {
        match header_field(card.text, "Kind") {
            None => {}
            Some((line, kind)) if kind != "iteration" => findings.push(card.finding(
                line,
                format!("`Kind` is `{kind}`, the only kind is `iteration`"),
            )),
            Some(_) => findings.extend(check_iteration(id, card, &cards)),
        }
        findings.extend(check_member(card, &cards));
    }
    findings
}

/// The rules of the card of an iteration: its title, its brief, and its outcome once it is `done`.
fn check_iteration(id: &str, card: &Card, cards: &Cards) -> Vec<Finding> {
    let mut findings = Vec::new();
    let title = card.text.lines().next().unwrap_or_default();
    if !is_iteration_title(title, id) {
        findings.push(card.finding(
            1,
            format!("the title must read `# {id}: Iteration <n>: <name>`"),
        ));
    }
    findings.extend(check_brief(card));
    // The outcome and the members are written last, so they are only asked of a finished iteration.
    let done = card_state(card.text).filter(|(_, state)| state == "done");
    if let Some((state_line, _)) = done {
        findings.extend(check_outcome(card, state_line));
        findings.extend(open_members(id, card, state_line, cards));
    }
    findings
}

/// `# T-042: Iteration 3: Sceau`, for the card `T-042`.
fn is_iteration_title(title: &str, id: &str) -> bool {
    let rest = title
        .strip_prefix("# ")
        .and_then(|rest| rest.strip_prefix(id))
        .and_then(|rest| rest.strip_prefix(": Iteration "));
    rest.and_then(|rest| rest.split_once(": "))
        .is_some_and(|(number, name)| {
            !number.is_empty()
                && number.chars().all(|c| c.is_ascii_digit())
                && !name.trim().is_empty()
        })
}

fn check_brief(card: &Card) -> Vec<Finding> {
    let Some(brief) = section(card.text, |line| line.trim_end() == "## Brief") else {
        return vec![card.finding(1, "an iteration card needs a `## Brief` section")];
    };
    BRIEF_ITEMS
        .iter()
        .filter(|item| brief.find(item).is_none())
        .map(|item| {
            card.finding(
                brief.line,
                format!("the Brief has no line starting with `{item}`"),
            )
        })
        .collect()
}

/// The outcome carries a date when it is written (`## Outcome (2026-11-02)`), so only the start is matched.
fn check_outcome(card: &Card, state_line: usize) -> Vec<Finding> {
    let Some(outcome) = section(card.text, |line| line.starts_with("## Outcome")) else {
        return vec![card.finding(
            state_line,
            "a `done` iteration needs an `## Outcome` section",
        )];
    };
    let mut findings = Vec::new();
    if outcome.find("**Bolder next step.**").is_none() {
        findings.push(card.finding(
            outcome.line,
            "the Outcome has no line starting with `**Bolder next step.**`",
        ));
    }
    match outcome.find("**Learning page.**") {
        None => findings.push(card.finding(
            outcome.line,
            "the Outcome has no line starting with `**Learning page.**`",
        )),
        Some((line, text)) if !text.contains("](http") => findings.push(card.finding(
            line,
            "the learning page line holds no link `[title](https://...)`",
        )),
        Some(_) => {}
    }
    findings
}

/// One finding for each card of the iteration `id` that is still open.
fn open_members(id: &str, card: &Card, state_line: usize, cards: &Cards) -> Vec<Finding> {
    cards
        .iter()
        .filter(|(_, member)| {
            header_field(member.text, "Iteration").is_some_and(|(_, value)| value == id)
        })
        .filter_map(|(member_id, member)| {
            let (_, state) = card_state(member.text)?;
            OPEN_STATES.contains(&state.as_str()).then(|| {
                card.finding(
                    state_line,
                    format!("{id} is `done` but {member_id} is `{state}`"),
                )
            })
        })
        .collect()
}

/// A card that says `- **Iteration:** T-xxx` names an existing iteration card.
fn check_member(card: &Card, cards: &Cards) -> Option<Finding> {
    let (line, iteration) = header_field(card.text, "Iteration")?;
    match cards.get(iteration.as_str()) {
        None => Some(card.finding(line, format!("`{iteration}` is not a card of board/tasks/"))),
        Some(target) if !target.is_iteration() => Some(card.finding(
            line,
            format!(
                "{iteration} is not an iteration card (no `- **Kind:** iteration` in its header)"
            ),
        )),
        Some(_) => None,
    }
}

/// A row of the index table: line number, state, text of the line.
type IndexRow = (usize, String, String);

/// The rows of the index table, by id.
fn parse_index(text: &str) -> BTreeMap<String, IndexRow> {
    let mut rows = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let Some(row) = line.trim().strip_prefix('|') else {
            continue;
        };
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        let (Some(id), Some(state)) = (cells.first(), cells.get(3)) else {
            continue;
        };
        if card_id(&format!("board/tasks/{id}.md")).is_some() {
            rows.insert(
                (*id).to_owned(),
                (index + 1, (*state).to_owned(), line.to_owned()),
            );
        }
    }
    rows
}

fn check_log(text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let starts_with_date = trimmed.strip_prefix("- ").is_some_and(starts_with_date);
        if !starts_with_date {
            findings.push(Finding::new(
                LOG,
                index + 1,
                "board-log",
                "a log entry must start with `- YYYY-MM-DD `",
                line,
            ));
        }
    }
    findings
}

/// `YYYY-MM-DD` followed by a space.
fn starts_with_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |from: usize, to: usize| -> Option<u32> {
        let part = bytes.get(from..to)?;
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(part).ok()?.parse().ok()
    };
    let (Some(year), Some(month), Some(day)) = (digits(0, 4), digits(5, 7), digits(8, 10)) else {
        return false;
    };
    year >= 2000
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes.get(10) == Some(&b' ')
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    const INDEX_OK: &str = "# Board index\n\n| id | title | role | state | depends on |\n|---|---|---|---|---|\n| T-001 | a | interface | done | |\n| T-002a | b | engine | in progress | T-001 |\n";
    const CARD_DONE: &str = "# T-001: a\n\n- **Role:** interface\n- **State:** done\n\n## Thread\n\n- 2026-10-09 x: State: todo.\n";
    const CARD_PROGRESS: &str = "# T-002a\n\n- **State:** in progress\n";
    const LOG_OK: &str = "# Log\n\n- 2026-10-08 T-001 (interface): done.\n";

    fn tree(files: &[(&str, &str)]) -> Tree {
        fixture(files, &[])
    }

    fn good() -> Vec<(&'static str, &'static str)> {
        vec![
            (INDEX, INDEX_OK),
            (LOG, LOG_OK),
            ("board/tasks/T-001.md", CARD_DONE),
            ("board/tasks/T-002a.md", CARD_PROGRESS),
        ]
    }

    fn rules(findings: &[Finding]) -> Vec<(&str, &'static str)> {
        findings.iter().map(|f| (f.file.as_str(), f.rule)).collect()
    }

    #[test]
    fn a_consistent_board_gives_no_finding() {
        assert!(check(&tree(&good())).is_empty());
    }

    #[test]
    fn a_card_without_a_state_or_with_an_unknown_state_is_a_finding() {
        let mut files = good();
        files[2] = ("board/tasks/T-001.md", "# T-001\n\nno state here\n");
        files[3] = ("board/tasks/T-002a.md", "# x\n- **State:** doing\n");
        let findings = check(&tree(&files));
        assert_eq!(
            rules(&findings),
            [
                ("board/tasks/T-001.md", "board-state"),
                ("board/tasks/T-002a.md", "board-state")
            ]
        );
        assert_eq!(findings[1].line, 2);
    }

    #[test]
    fn the_state_in_a_thread_note_is_not_the_card_state() {
        // CARD_DONE has a "State: todo." note below the header; the header says done.
        assert!(check(&tree(&good())).is_empty());
    }

    #[test]
    fn the_five_states_and_split_are_valid() {
        for state in [
            "todo",
            "in progress",
            "to review",
            "done",
            "blocked",
            "split",
        ] {
            let card = format!("- **State:** {state}\n");
            let index = format!("| T-001 | a | r | {state} | |\n");
            let files = [
                (INDEX, index.as_str()),
                (LOG, LOG_OK),
                ("board/tasks/T-001.md", card.as_str()),
            ];
            assert!(check(&tree(&files)).is_empty(), "{state}");
        }
    }

    #[test]
    fn a_state_that_differs_from_the_index_is_a_finding() {
        let mut files = good();
        files[0] = (
            INDEX,
            "| T-001 | a | interface | todo | |\n| T-002a | b | e | in progress | |\n",
        );
        let findings = check(&tree(&files));
        assert_eq!(rules(&findings), [("board/tasks/T-001.md", "board-index")]);
        assert_eq!(findings[0].line, 4);
        assert!(findings[0].message.contains("`todo`"));
    }

    #[test]
    fn a_card_missing_from_the_index_and_a_row_without_card_are_findings() {
        let mut files = good();
        files[0] = (
            INDEX,
            "| T-001 | a | interface | done | |\n| T-009 | z | e | done | |\n",
        );
        let findings = check(&tree(&files));
        assert_eq!(
            rules(&findings),
            [
                ("board/tasks/T-002a.md", "board-index"),
                (INDEX, "board-index")
            ]
        );
        assert_eq!(findings[1].line, 2);
    }

    #[test]
    fn log_lines_must_start_with_a_date() {
        let log = "# Log\n\n- 2026-10-08 T-001 ok\n- T-002 no date\nstray text\n- 2026-13-01 bad month\n- 2026-10-0 short\n";
        let mut files = good();
        files[1] = (LOG, log);
        let findings = check(&tree(&files));
        let lines: Vec<usize> = findings.iter().map(|f| f.line).collect();
        assert_eq!(lines, [4, 5, 6, 7]);
        assert!(findings.iter().all(|f| f.rule == "board-log"));
    }

    #[test]
    fn other_files_of_the_board_directory_are_not_cards() {
        let mut files = good();
        files.push(("board/tasks/notes.md", "no state\n"));
        files.push(("board/messages/2026-10-09-a-b-c.md", "no state\n"));
        assert!(check(&tree(&files)).is_empty());
    }

    // ---- board-iteration ----

    /// A finished iteration: line 1 is the title, line 4 the kind, line 5 the state.
    const ITERATION_CARD: &str = "# T-010: Iteration 1: Sceau\n\n- **Role:** orchestrator\n- **Kind:** iteration\n- **State:** done\n\n## Brief\n\n**Goal.** g\n\n**Deliverables.** d\n\n**Claims touched.** none\n\n**Done when.** w\n\n**Plan.** p\n\n## Outcome (2026-11-02)\n\n**Results.** 5 of 5\n\n**Bolder next step.** more\n\n**Learning page.** [Sceau](https://claude.ai/artifact/x)\n\n## Thread\n\n- 2026-11-02 x: done.\n";
    const MEMBER_CARD: &str = "# T-011: a\n\n- **State:** done\n- **Iteration:** T-010\n";

    /// The findings of `board-iteration` on cards given as `(id, text)`.
    fn iteration_findings(cards: &[(&str, &str)]) -> Vec<Finding> {
        let paths: Vec<String> = cards
            .iter()
            .map(|(id, _)| format!("board/tasks/{id}.md"))
            .collect();
        let files: Vec<(&str, &str)> = paths
            .iter()
            .zip(cards)
            .map(|(path, (_, text))| (path.as_str(), *text))
            .collect();
        check_iterations(&tree(&files))
    }

    /// The same finished iteration with some text replaced.
    fn iteration_with(from: &str, to: &str) -> String {
        assert!(ITERATION_CARD.contains(from), "fixture lost `{from}`");
        ITERATION_CARD.replace(from, to)
    }

    fn lines(findings: &[Finding]) -> Vec<usize> {
        findings.iter().map(|f| f.line).collect()
    }

    #[test]
    fn a_consistent_iteration_gives_no_finding() {
        let cards = [("T-010", ITERATION_CARD), ("T-011", MEMBER_CARD)];
        assert!(iteration_findings(&cards).is_empty());
        // Through the whole gate, so that `check` runs the rule.
        let index = "| T-001 | a | r | done | |\n| T-002a | b | r | in progress | |\n| T-010 | i | orchestrator | done | |\n| T-011 | a | r | done | |\n";
        let mut files = good();
        files[0] = (INDEX, index);
        files.push(("board/tasks/T-010.md", ITERATION_CARD));
        files.push(("board/tasks/T-011.md", MEMBER_CARD));
        assert!(check(&tree(&files)).is_empty());
        files[5] = (
            "board/tasks/T-011.md",
            "- **State:** done\n- **Iteration:** T-999\n",
        );
        let findings = check(&tree(&files));
        assert_eq!(
            rules(&findings),
            [("board/tasks/T-011.md", "board-iteration")]
        );
    }

    #[test]
    fn a_kind_other_than_iteration_is_a_finding() {
        let card = iteration_with("- **Kind:** iteration", "- **Kind:** sprint");
        let findings = iteration_findings(&[("T-010", card.as_str())]);
        assert_eq!(lines(&findings), [4]);
        assert!(findings[0].message.contains("`sprint`"));
        // Without a `Kind` line a card is an ordinary card, and nothing else is asked of it.
        let plain = iteration_with("- **Kind:** iteration\n", "");
        assert!(iteration_findings(&[("T-010", plain.as_str())]).is_empty());
    }

    #[test]
    fn the_title_of_an_iteration_names_its_id_its_number_and_its_name() {
        for title in [
            "# T-010: Sceau",
            "# T-010: Iteration one: Sceau",
            "# T-010: Iteration : Sceau",
            "# T-010: Iteration 1:",
            "# T-010: Iteration 1",
            "# T-011: Iteration 1: Sceau",
            "# T-01: Iteration 1: Sceau",
            "T-010: Iteration 1: Sceau",
        ] {
            let card = iteration_with("# T-010: Iteration 1: Sceau", title);
            let findings = iteration_findings(&[("T-010", card.as_str())]);
            assert_eq!(lines(&findings), [1], "{title}");
        }
        let long = iteration_with("Iteration 1: Sceau", "Iteration 12: Sceau: second wind");
        assert!(iteration_findings(&[("T-010", long.as_str())]).is_empty());
    }

    #[test]
    fn an_iteration_needs_a_brief_with_its_five_items() {
        let none = iteration_with("## Brief", "## Plan of work");
        let findings = iteration_findings(&[("T-010", none.as_str())]);
        assert_eq!(lines(&findings), [1]);
        assert!(findings[0].message.contains("## Brief"));
        // One finding for each missing item, on the heading of the Brief (line 7).
        let card =
            iteration_with("**Claims touched.** none\n\n", "").replace("**Plan.** p\n\n", "");
        let findings = iteration_findings(&[("T-010", card.as_str())]);
        assert_eq!(lines(&findings), [7, 7]);
        assert!(findings[0].message.contains("**Claims touched.**"));
        assert!(findings[1].message.contains("**Plan.**"));
    }

    #[test]
    fn the_items_of_the_brief_are_read_in_the_brief_only() {
        // `**Done when.**` moved under the Outcome does not count for the Brief.
        let card = iteration_with("**Done when.** w\n\n", "")
            .replace("**Results.**", "**Done when.** w\n\n**Results.**");
        let findings = iteration_findings(&[("T-010", card.as_str())]);
        assert_eq!(lines(&findings), [7]);
        assert!(findings[0].message.contains("**Done when.**"));
    }

    #[test]
    fn a_done_iteration_needs_an_outcome_with_a_bolder_step_and_a_learning_page_link() {
        let no_section = iteration_with("## Outcome (2026-11-02)", "## Results");
        let findings = iteration_findings(&[("T-010", no_section.as_str())]);
        assert_eq!(lines(&findings), [5]);
        let no_step = iteration_with("**Bolder next step.** more\n\n", "");
        assert_eq!(
            lines(&iteration_findings(&[("T-010", no_step.as_str())])),
            [19]
        );
        let no_page = iteration_with(
            "**Learning page.** [Sceau](https://claude.ai/artifact/x)\n\n",
            "",
        );
        assert_eq!(
            lines(&iteration_findings(&[("T-010", no_page.as_str())])),
            [19]
        );
        let no_link = iteration_with("[Sceau](https://claude.ai/artifact/x)", "to come");
        let findings = iteration_findings(&[("T-010", no_link.as_str())]);
        assert_eq!(lines(&findings), [25]);
        assert!(findings[0].message.contains("link"));
    }

    #[test]
    fn the_outcome_may_carry_a_date_or_none() {
        let plain = iteration_with("## Outcome (2026-11-02)", "## Outcome");
        assert!(iteration_findings(&[("T-010", plain.as_str())]).is_empty());
    }

    #[test]
    fn an_iteration_that_is_not_done_needs_no_outcome() {
        let started = iteration_with("- **State:** done", "- **State:** in progress")
            .replace("## Outcome (2026-11-02)", "## Thread");
        assert!(iteration_findings(&[("T-010", started.as_str())]).is_empty());
        // The brief is written before any work, so it is asked of a started iteration.
        let no_brief = iteration_with("- **State:** done", "- **State:** todo")
            .replace("## Brief", "## Notes");
        assert_eq!(
            lines(&iteration_findings(&[("T-010", no_brief.as_str())])),
            [1]
        );
    }

    #[test]
    fn a_done_iteration_has_no_card_still_open() {
        for (state, open) in [
            ("todo", true),
            ("in progress", true),
            ("to review", true),
            ("done", false),
            ("blocked", false),
            ("split", false),
        ] {
            let member = format!("# T-011: a\n\n- **State:** {state}\n- **Iteration:** T-010\n");
            let other = "# T-012: b\n\n- **State:** todo\n";
            let cards = [
                ("T-010", ITERATION_CARD),
                ("T-011", member.as_str()),
                ("T-012", other),
            ];
            let findings = iteration_findings(&cards);
            assert_eq!(findings.len(), usize::from(open), "{state}");
            if open {
                assert_eq!(findings[0].file, "board/tasks/T-010.md");
                assert_eq!(findings[0].line, 5);
                assert!(findings[0].message.contains(&format!("T-011 is `{state}`")));
            }
        }
        // An iteration still in progress may have open cards.
        let running = iteration_with("- **State:** done", "- **State:** in progress");
        let member = "# T-011: a\n\n- **State:** todo\n- **Iteration:** T-010\n";
        assert!(iteration_findings(&[("T-010", running.as_str()), ("T-011", member)]).is_empty());
    }

    #[test]
    fn a_card_of_an_iteration_names_an_existing_iteration_card() {
        let unknown = "- **State:** done\n- **Iteration:** T-099\n";
        let findings = iteration_findings(&[("T-010", ITERATION_CARD), ("T-011", unknown)]);
        assert_eq!(
            rules(&findings),
            [("board/tasks/T-011.md", "board-iteration")]
        );
        assert_eq!(findings[0].line, 2);
        assert!(findings[0].message.contains("T-099"));
        // T-012 is a card, but not an iteration.
        let plain = "# T-012: b\n\n- **State:** done\n";
        let member = "- **State:** done\n- **Iteration:** T-012\n";
        let findings = iteration_findings(&[("T-011", member), ("T-012", plain)]);
        assert_eq!(
            rules(&findings),
            [("board/tasks/T-011.md", "board-iteration")]
        );
        assert!(findings[0].message.contains("not an iteration card"));
    }
}
