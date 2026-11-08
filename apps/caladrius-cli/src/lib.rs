#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! App layer: the command-line application. It runs the engine's commands (`caladrius-engine`)
//! exactly as the UI and the MCP server do; this crate only reads arguments and files, calls
//! [`caladrius_engine::Engine::execute`] and prints the answer.
//!
//! ```text
//! caladrius-cli commands [--format text|json]
//! caladrius-cli <command-id> [--json FILE|-] [--param KEY=VALUE]... [--csv FILE]
//!               [--project FILE] [--format json|csv] [--table NAME] [--no-seal]
//!               [--progress] [--cancel-after N]      (fit.run only)
//! caladrius-cli compare A B [--param parameters=[...]] --project FILE [--format json|csv]
//! ```
//!
//! Every failure is a one-line message on stderr and exit code 1. `project.replay --project FILE`
//! prints its report and exits with code 1 when the replay finds a difference.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use caladrius_engine::seal::{csv_with_stamp, json_with_seal, stamp_line};
use caladrius_engine::{CommandInfo, Control, Engine, Observer, Progress, command_ids, describe};
use serde_json::{Map, Value, json};

/// Why the invocation failed. The message is exactly what the person reads.
#[derive(Debug, Clone, PartialEq)]
pub struct CliError(pub String);

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CliError {}

fn fail<T>(message: impl Into<String>) -> Result<T, CliError> {
    Err(CliError(message.into()))
}

impl From<caladrius_engine::CommandError> for CliError {
    fn from(e: caladrius_engine::CommandError) -> Self {
        CliError(format!("{}: {}", e.code, e.message))
    }
}

/// What a successful invocation prints.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Output {
    /// For standard output, ends with a newline.
    pub stdout: String,
    /// Remarks for standard error, one per line: what an import guessed, unit warnings.
    pub notes: Vec<String>,
    /// The exit code: 0, or 1 when `project.replay` found that the project is not what its
    /// history produces (the report is printed all the same).
    pub exit_code: u8,
}

/// Output format of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Csv,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
struct RunArgs {
    command: String,
    json: Option<String>,
    params: Vec<String>,
    csv: Option<PathBuf>,
    project: Option<PathBuf>,
    format: Option<Format>,
    table: Option<String>,
    /// Print the answer as it is, without the seal of the command.
    no_seal: bool,
    /// `fit.run` only: one line per iteration on standard error.
    progress: bool,
    /// `fit.run` only: cancel after this iteration (0: before the first).
    cancel_after: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Help,
    Commands(Option<Format>),
    Run(Box<RunArgs>),
}

/// The text printed by `help`.
pub const USAGE: &str = "\
usage:
  caladrius-cli commands [--format text|json]
  caladrius-cli <command-id> [options]

options for a command:
  --json FILE|-       parameters as a JSON object, from a file or from standard input
  --param KEY=VALUE   one parameter (repeatable, overrides --json). KEY may be dotted
                      (options.auc_method=linear). VALUE is JSON when it parses (1, true,
                      [1,2]), else text; @FILE reads the text of a file (csv=@data.csv)
  --csv FILE          import FILE as a worksheet first; commands that take a `worksheet`
                      use it when none is given
  --project FILE      load this project if it exists, and write it back after a command
                      that changes it. For project.replay it is the project to replay:
                      nothing is loaded or written
  --format json|csv   json (default): the command's result; csv: its table (an analysis
                      prints its main table, see --table)
  --table NAME        the table to print with --format csv (export.table names)
  --progress          fit.run only: one line per iteration on standard error, such as
                      `iteration 3, wrss 12.5`; the answer on standard output is unchanged.
                      Not with `--param subject=all`
  --cancel-after N    fit.run only: cancel after iteration N (for scripts and tests). Prints
                      `error: fit cancelled after iteration N; nothing was stored` on standard error,
                      stores nothing, prints no answer and exits with code 1. Ctrl-C does the
                      same at the next iteration
  --param subject=all with fit.run: one independent fit per subject of the worksheet (same
                      model and options, initial estimates generated per subject); the answer
                      is a list in `subjects`, and a subject that cannot be fitted is that
                      subject's result with its error while the others run. Without it a
                      worksheet with several subjects needs one named (`subject_required`).
                      nca.run without `subject` already analyses every subject. There is no
                      progress and no cancel for `all`: it is not a job
  --no-seal           print the answer as it is. By default it carries the seal of the command:
                      a `seal` member in JSON, a first line `# caladrius engine=... seal=...` in
                      CSV (data.import skips that line; for other programs read with
                      comment=\"#\" or drop the lines that start with #)

Every command is also listed, with the JSON schema of its parameters and results, by
`caladrius-cli commands --format json`. Failures print `error: ...` and exit with code 1.

compare A B         short for `analysis.compare --param a=A --param b=B`: the difference, the
                      percentage and the ratio of the parameters two analyses of the project
                      share (--format csv prints them as a table)

project.replay      runs the history of a saved project again and checks every step against
                      its seal, then the stored project against the one the steps produce. The
                      report is printed; the exit code is 1 when something differs

example:
  caladrius-cli nca.run --csv oracle/data/theoph.csv --param route=extravascular --format csv
  caladrius-cli compare 2 3 --project study.caladrius.json --format csv
  caladrius-cli project.replay --project study.caladrius.json";

fn parse_format(text: &str) -> Result<Format, CliError> {
    match text {
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        "text" => Ok(Format::Text),
        other => fail(format!(
            "unknown format `{other}`; use json, csv or text (see `caladrius-cli help`)"
        )),
    }
}

/// Splits `--flag=value` and `--flag value`.
fn parse_args(args: &[String]) -> Result<Action, CliError> {
    let mut it = args.iter().peekable();
    let Some(first) = it.next() else {
        return fail(
            "missing command; run `caladrius-cli help`, or `caladrius-cli commands` for the list",
        );
    };
    if matches!(first.as_str(), "help" | "--help" | "-h") {
        return Ok(Action::Help);
    }
    let is_commands = first == "commands";
    // `compare A B` is `analysis.compare` with the two analysis ids as plain arguments.
    let is_compare = first == "compare";
    let mut positional = 0;
    let mut run = RunArgs {
        command: if is_compare {
            "analysis.compare".to_owned()
        } else {
            first.clone()
        },
        json: None,
        params: Vec::new(),
        csv: None,
        project: None,
        format: None,
        table: None,
        no_seal: false,
        progress: false,
        cancel_after: None,
    };
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        if is_compare && !arg.starts_with("--") && positional < 2 {
            if arg.parse::<u64>().is_err() {
                return fail(format!(
                    "`{arg}` is not an analysis id; use `compare A B` with the ids of two analyses"
                ));
            }
            run.params
                .push(format!("{}={arg}", if positional == 0 { "a" } else { "b" }));
            positional += 1;
            continue;
        }
        if !flag.starts_with("--") {
            return fail(format!(
                "unexpected argument `{arg}`; options start with `--` (see `caladrius-cli help`)"
            ));
        }
        let has_value = inline.is_some();
        let value = |it: &mut std::iter::Peekable<std::slice::Iter<String>>| match inline {
            Some(v) => Ok(v),
            None => it
                .next()
                .cloned()
                .ok_or_else(|| CliError(format!("`{flag}` needs a value"))),
        };
        match flag.as_str() {
            "--json" if !is_commands => run.json = Some(value(&mut it)?),
            "--param" if !is_commands => run.params.push(value(&mut it)?),
            "--csv" if !is_commands => run.csv = Some(PathBuf::from(value(&mut it)?)),
            "--project" if !is_commands => run.project = Some(PathBuf::from(value(&mut it)?)),
            "--table" if !is_commands => run.table = Some(value(&mut it)?),
            "--no-seal" if !is_commands => {
                if has_value {
                    return fail("`--no-seal` takes no value");
                }
                run.no_seal = true;
            }
            "--progress" if !is_commands => {
                if has_value {
                    return fail("`--progress` takes no value");
                }
                run.progress = true;
            }
            "--cancel-after" if !is_commands => {
                let text = value(&mut it)?;
                run.cancel_after = Some(text.parse().map_err(|_| {
                    CliError(format!(
                        "`--cancel-after` needs a whole number of iterations (0 or more), not `{text}`"
                    ))
                })?);
            }
            "--format" => run.format = Some(parse_format(&value(&mut it)?)?),
            other => {
                return fail(format!(
                    "unknown option `{other}` (see `caladrius-cli help`)"
                ));
            }
        }
    }
    if is_commands {
        return Ok(Action::Commands(run.format));
    }
    Ok(Action::Run(Box::new(run)))
}

/// Parses `key=value` into a path and a JSON value (text when the value is not JSON).
fn parse_param(
    text: &str,
    read_file: &dyn Fn(&Path) -> Result<String, CliError>,
) -> Result<(Vec<String>, Value), CliError> {
    let Some((key, raw)) = text.split_once('=') else {
        return fail(format!("`--param {text}` must be KEY=VALUE"));
    };
    let path: Vec<String> = key.split('.').map(str::to_owned).collect();
    if key.is_empty() || path.iter().any(String::is_empty) {
        return fail(format!("`--param {text}` has an empty parameter name"));
    }
    let value = if let Some(file) = raw.strip_prefix('@') {
        Value::String(read_file(Path::new(file))?)
    } else {
        serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
    };
    Ok((path, value))
}

fn set_path(root: &mut Map<String, Value>, path: &[String], value: Value) -> Result<(), CliError> {
    match path {
        [] => Ok(()),
        [last] => {
            root.insert(last.clone(), value);
            Ok(())
        }
        [head, rest @ ..] => {
            let entry = root
                .entry(head.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            match entry {
                Value::Object(map) => set_path(map, rest, value),
                _ => fail(format!(
                    "`{head}` is not an object, so `{head}.{}` cannot be set",
                    rest.join(".")
                )),
            }
        }
    }
}

fn read_text(path: &Path) -> Result<String, CliError> {
    let bytes =
        fs::read(path).map_err(|e| CliError(format!("cannot read {}: {e}", path.display())))?;
    String::from_utf8(bytes).map_err(|_| {
        CliError(format!(
            "{} is not UTF-8 text; save it as UTF-8",
            path.display()
        ))
    })
}

/// Writes to a temporary file then renames, so an interrupted run never leaves half a project.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    fs::write(&temp, bytes)
        .map_err(|e| CliError(format!("cannot write {}: {e}", temp.display())))?;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        CliError(format!("cannot write {}: {e}", path.display()))
    })
}

fn to_json_text(value: &Value) -> Result<String, CliError> {
    serde_json::to_string_pretty(value)
        .map(|mut s| {
            s.push('\n');
            s
        })
        .map_err(|e| CliError(format!("cannot print the result: {e}")))
}

fn list_commands(format: Option<Format>) -> Result<Output, CliError> {
    let all = describe();
    let stdout = match format.unwrap_or(Format::Text) {
        Format::Json => to_json_text(&json!(all))?,
        Format::Text => {
            let width = all.iter().map(|c| c.id.len()).max().unwrap_or(0);
            let mut out = String::new();
            for c in &all {
                out.push_str(&format!("{:<width$}  {}\n", c.id, c.title));
            }
            out
        }
        Format::Csv => return fail("`commands` prints text or json, not csv"),
    };
    Ok(Output {
        stdout,
        notes: Vec::new(),
        exit_code: 0,
    })
}

/// The `worksheet` a command should use when none is given: the one just imported, or the only
/// one of the project.
fn default_worksheet(engine: &Engine, imported: Option<&Value>) -> Option<Value> {
    if let Some(id) = imported {
        return Some(id.clone());
    }
    match engine.project().worksheets() {
        [only] => Some(json!(only.id())),
        _ => None,
    }
}

fn takes(info: &CommandInfo, property: &str) -> bool {
    info.params_schema["properties"].get(property).is_some()
}

/// A table of one result as CSV.
fn csv_of(
    engine: &mut Engine,
    params: &Value,
    result: &Value,
    table: Option<&str>,
) -> Result<String, CliError> {
    // analysis.compare: one row per parameter.
    if params.get("a").is_some() && result.get("rows").is_some() && result.get("b").is_some() {
        return Ok(compare_csv(result));
    }
    // export.table prints its own table.
    if let Some(csv) = result.get("csv").and_then(Value::as_str) {
        return Ok(csv.to_owned());
    }
    // A simulation that was not stored: its curves.
    if table.is_none() {
        if let (Some(times), Some(conc), Some(auc)) = (
            result.get("times").and_then(Value::as_array),
            result.get("conc").and_then(Value::as_array),
            result.get("auc").and_then(Value::as_array),
        ) {
            let mut out = String::from("time,conc,auc\n");
            for ((t, c), a) in times.iter().zip(conc).zip(auc) {
                out.push_str(&format!(
                    "{},{},{}
",
                    number(t),
                    number(c),
                    number(a)
                ));
            }
            return Ok(out);
        }
    }
    let default_table = match result.get("kind").and_then(Value::as_str) {
        Some("nca") => Some("nca.parameters"),
        Some("fit") => Some("fit.parameters"),
        Some("simulation") => Some("simulation"),
        _ => None,
    };
    let analysis = result
        .get("id")
        .cloned()
        .or_else(|| params.get("analysis").cloned());
    let Some(table) = table.or(default_table) else {
        return fail(
            "this command has no table; use --format json, or --table NAME to export one (see `export.table`)",
        );
    };
    let mut export = json!({ "table": table });
    if table == "worksheet" {
        if let Some(ws) = params
            .get("worksheet")
            .cloned()
            .or_else(|| result["worksheet"].get("id").cloned())
        {
            export["worksheet"] = ws;
        }
    } else if let Some(id) = analysis {
        export["analysis"] = id;
    }
    let exported = engine.execute("export.table", export)?;
    Ok(exported["csv"].as_str().unwrap_or_default().to_owned())
}

/// Commands whose answer has a CSV form without naming a table.
const CSV_COMMANDS: [&str; 7] = [
    "analysis.compare",
    "nca.run",
    "fit.run",
    "analysis.run",
    "analysis.get",
    "model.simulate",
    "export.table",
];

/// The columns of the table of `analysis.compare`, in the order of the rows' fields.
const COMPARE_COLUMNS: [&str; 10] = [
    "parameter",
    "a",
    "b",
    "unit_a",
    "unit_b",
    "difference",
    "difference_unit",
    "relative_percent",
    "ratio",
    "not_comparable",
];

/// A CSV field: quoted when it holds a comma, a quote or a line end.
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// The comparison of two analyses as a table: a column per field of a row, a line per parameter,
/// an empty cell for a number the engine did not compute.
fn compare_csv(result: &Value) -> String {
    let mut out = COMPARE_COLUMNS.join(",");
    out.push('\n');
    for row in result["rows"].as_array().into_iter().flatten() {
        let cells: Vec<String> = COMPARE_COLUMNS
            .iter()
            .map(|column| match &row[*column] {
                Value::Null => String::new(),
                Value::String(text) => csv_field(text),
                other => number(other),
            })
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

/// The tables `export.table` knows, from its schema.
fn known_tables() -> Vec<String> {
    describe()
        .into_iter()
        .find(|c| c.id == "export.table")
        .and_then(|c| {
            c.params_schema["properties"]["table"]["enum"]
                .as_array()
                .cloned()
        })
        .map(|v| {
            v.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Checks the combination of flags before anything runs, so a refusal never costs a run (and, with
/// `--project`, never loses its result).
fn check_flags(args: &RunArgs) -> Result<(), CliError> {
    let format = args.format.unwrap_or(Format::Json);
    if format == Format::Text {
        return fail("`--format text` is only for `commands`; use json or csv");
    }
    if (args.progress || args.cancel_after.is_some()) && args.command != "fit.run" {
        return fail(format!(
            "`--progress` and `--cancel-after` are for `fit.run`, not `{}`",
            args.command
        ));
    }
    if args.table.is_some() && format != Format::Csv {
        return fail("--table needs --format csv");
    }
    if let Some(table) = &args.table {
        let tables = known_tables();
        if !tables.contains(table) {
            return fail(format!(
                "there is no table `{table}`; the tables are: {}",
                tables.join(", ")
            ));
        }
    }
    if format == Format::Csv
        && args.table.is_none()
        && !CSV_COMMANDS.contains(&args.command.as_str())
    {
        return fail(format!(
            "`{}` has no table to print; use --format json, or --table NAME to export a table of an analysis (the tables are: {})",
            args.command,
            known_tables().join(", ")
        ));
    }
    Ok(())
}

/// Refuses a parameter given twice, or given both whole and in part (`a` and `a.b`).
fn check_duplicate_params(params: &[String]) -> Result<(), CliError> {
    let mut seen: Vec<Vec<String>> = Vec::new();
    for p in params {
        let key = p.split_once('=').map_or(p.as_str(), |(k, _)| k);
        let path: Vec<String> = key.split('.').map(str::to_owned).collect();
        for earlier in &seen {
            let n = earlier.len().min(path.len());
            if earlier.get(..n) == path.get(..n) {
                return fail(format!(
                    "`--param {key}` conflicts with `--param {}`: give each parameter once",
                    earlier.join(".")
                ));
            }
        }
        seen.push(path);
    }
    Ok(())
}

/// The notes an import prints: what was guessed, and the unit warnings.
fn import_notes(imported: &Value) -> Vec<String> {
    let mut notes: Vec<String> = imported["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|n| format!("note: {n}"))
        .collect();
    for w in imported["worksheet"]["unit_warnings"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(m) = w["message"].as_str() {
            notes.push(format!("warning: {m}"));
        }
    }
    notes
}

/// What a fit asked to do between two iterations: print a line, stop after iteration N, stop when
/// the person pressed Ctrl-C.
struct FitWatch<'a> {
    progress: bool,
    cancel_after: Option<u32>,
    sink: &'a mut dyn FnMut(&str),
    interrupted: &'a dyn Fn() -> bool,
}

/// A number as the engine writes numbers in its answers (`12.5`, `1e-7`).
fn engine_number(x: f64) -> String {
    serde_json::Number::from_f64(x).map_or_else(|| format!("{x}"), |n| n.to_string())
}

impl FitWatch<'_> {
    fn stop(&self, iteration: u32) -> Control {
        if self.cancel_after.is_some_and(|n| iteration >= n) || (self.interrupted)() {
            Control::Cancel
        } else {
            Control::Continue
        }
    }
}

impl Observer for FitWatch<'_> {
    fn before_first_iteration(&mut self, progress: &Progress<'_>) -> Control {
        self.stop(progress.iteration)
    }

    fn after_iteration(&mut self, progress: &Progress<'_>) -> Control {
        if self.progress {
            let line = format!(
                "iteration {}, wrss {}",
                progress.iteration,
                engine_number(progress.objective)
            );
            (self.sink)(&line);
        }
        self.stop(progress.iteration)
    }
}

/// How `fit.run` ended on the command line.
enum FitEnd {
    Answer(Value),
    /// Cancelled: the sentence has been sent to the sink.
    Cancelled,
}

/// `fit.run` as a job: one line per iteration when asked, a stop after iteration N, a stop on
/// Ctrl-C. The answer, its seal and its entry are those of `execute` (the engine proves it).
fn run_fit_job(
    engine: &mut Engine,
    params: &Value,
    watch: &mut FitWatch<'_>,
) -> Result<FitEnd, CliError> {
    let job = engine.begin("fit.run", params.clone())?;
    let completed = job.run(watch);
    match engine.finish(completed) {
        Ok(answer) => Ok(FitEnd::Answer(answer)),
        Err(e) if e.code == "fit_cancelled" => {
            (watch.sink)(&format!("error: {}", e.message));
            Ok(FitEnd::Cancelled)
        }
        Err(e) => Err(e.into()),
    }
}

fn run_command(
    args: &RunArgs,
    stdin: &mut dyn Read,
    sink: &mut dyn FnMut(&str),
    interrupted: &dyn Fn() -> bool,
) -> Result<Output, CliError> {
    let Some(info) = describe().into_iter().find(|c| c.id == args.command) else {
        // The engine's own message lists the commands.
        let mut engine = Engine::new();
        return match engine.execute(&args.command, Value::Null) {
            Err(e) => Err(e.into()),
            Ok(_) => fail("unknown command"),
        };
    };
    check_flags(args)?;
    check_duplicate_params(&args.params)?;
    // Parameters: the JSON object first, then each --param on top.
    let mut params = match &args.json {
        None => Map::new(),
        Some(source) => {
            let text = if source == "-" {
                let mut buffer = String::new();
                stdin
                    .read_to_string(&mut buffer)
                    .map_err(|e| CliError(format!("cannot read standard input: {e}")))?;
                buffer
            } else {
                read_text(Path::new(source))?
            };
            match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(map)) => map,
                Ok(_) => return fail(format!("{source} must hold a JSON object of parameters")),
                Err(e) => return fail(format!("{source} is not valid JSON: {e}")),
            }
        }
    };
    for p in &args.params {
        let (path, value) = parse_param(p, &read_text)?;
        set_path(&mut params, &path, value)?;
    }

    // `project.replay` checks a file: it is the text it is given, and nothing is loaded or written.
    let replaying = args.command == "project.replay";
    if replaying {
        if args.csv.is_some() {
            return fail("`project.replay` replays a project: give it with --project FILE");
        }
        if let Some(path) = &args.project {
            if params.contains_key("text") || params.contains_key("project") {
                return fail("give the project once: --project FILE, or --param text=@FILE");
            }
            params.insert("text".to_owned(), Value::String(read_text(path)?));
        }
    }

    let mut notes = Vec::new();
    let mut engine = Engine::new();
    engine.set_clock(system_clock);
    if let Some(path) = args.project.as_ref().filter(|_| !replaying) {
        match fs::read(path) {
            Ok(bytes) => {
                engine.load_bytes(&bytes)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return fail(format!("cannot read {}: {e}", path.display())),
        }
    }
    let mut imported_id = None;
    if let Some(csv) = &args.csv {
        let name = csv
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("data")
            .to_owned();
        if args.command == "data.import" {
            // `data.import --csv FILE` is the import itself: the file is its `csv` parameter.
            if params.contains_key("csv") {
                return fail("give the CSV once: --csv FILE, or --param csv=...");
            }
            let text = read_text(csv)?;
            params.insert("csv".to_owned(), Value::String(text));
            params
                .entry("name".to_owned())
                .or_insert_with(|| Value::String(name));
        } else {
            let bytes = fs::read(csv)
                .map_err(|e| CliError(format!("cannot read {}: {e}", csv.display())))?;
            let imported = engine.import_csv(&name, &bytes)?;
            notes.extend(import_notes(&imported));
            imported_id = Some(imported["worksheet"]["id"].clone());
        }
    }
    if takes(&info, "worksheet") && !params.contains_key("worksheet") {
        if let Some(id) = default_worksheet(&engine, imported_id.as_ref()) {
            params.insert("worksheet".to_owned(), id);
        }
    }
    let params = Value::Object(params);
    // `--param subject=all` is one fit per subject: it runs in one step through `execute`, as a
    // command with no progress to watch and no cancel (DAT-USE-02).
    let all_subjects = args.command == "fit.run" && params["subject"].as_str() == Some("all");
    if all_subjects && (args.progress || args.cancel_after.is_some()) {
        return fail(
            "`--progress` and `--cancel-after` watch one fit; `subject=all` runs one fit per subject in one step, so name one subject to watch it",
        );
    }
    let result = if args.command == "fit.run" && !all_subjects {
        // What the import said comes before the lines of the fit.
        for note in notes.drain(..) {
            sink(&note);
        }
        let mut watch = FitWatch {
            progress: args.progress,
            cancel_after: args.cancel_after,
            sink,
            interrupted,
        };
        match run_fit_job(&mut engine, &params, &mut watch)? {
            FitEnd::Answer(answer) => answer,
            // Nothing is stored and nothing is printed on standard output; with --project the
            // file is left as it was.
            FitEnd::Cancelled => {
                return Ok(Output {
                    stdout: String::new(),
                    notes,
                    exit_code: 1,
                });
            }
        }
    } else {
        engine.execute(&args.command, params.clone())?
    };
    if args.command == "data.import" {
        notes.extend(import_notes(&result));
    }
    let mut exit_code = 0;
    if replaying {
        let verdict = result["verdict"].as_str().unwrap_or("unknown");
        let mut note = format!("{verdict}: {}", result["summary"].as_str().unwrap_or(""));
        // The first seal that is not reproduced, as a screen shows it.
        let first = &result["first_divergence"];
        if let (Some(stored), Some(now)) = (first["stored"].as_str(), first["recomputed"].as_str())
        {
            let short = caladrius_engine::seal::short;
            note.push_str(&format!(
                " Stored {}, recomputed {}.",
                short(stored),
                short(now)
            ));
        }
        notes.push(note);
        if verdict == "diverged" {
            exit_code = 1;
        }
    }

    let stdout = match args.format.unwrap_or(Format::Json) {
        Format::Csv => {
            let csv = csv_of(&mut engine, &params, &result, args.table.as_deref())?;
            // The last command executed is the one that produced what is printed (the command
            // itself, or the export.table that gave its table).
            match stamp_of(&engine).filter(|_| !args.no_seal) {
                Some(line) => csv_with_stamp(&csv, &line),
                None => csv,
            }
        }
        _ => match last_seal(&engine).filter(|_| !args.no_seal) {
            Some(seal) => to_json_text(&json_with_seal(result, &seal))?,
            None => to_json_text(&result)?,
        },
    };
    if let Some(path) = &args.project {
        if info.mutates || imported_id.is_some() {
            write_atomic(path, &engine.save_bytes()?)?;
        }
    }
    Ok(Output {
        stdout,
        notes,
        exit_code,
    })
}

/// The seal of the last command the engine executed, if it carries one.
fn last_seal(engine: &Engine) -> Option<caladrius_engine::Seal> {
    engine.history().entries().last()?.seal.clone()
}

/// The stamp line of the last command the engine executed, if it carries a seal.
fn stamp_of(engine: &Engine) -> Option<String> {
    let entry = engine.history().entries().last()?;
    Some(stamp_line(&entry.command, entry.seal.as_ref()?))
}

/// The time of the system, in seconds since 1970 (the engine reads no clock itself).
fn system_clock() -> Option<i64> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    i64::try_from(elapsed.as_secs()).ok()
}

/// Runs the command line `args` (without the program name). `stdin` is read only for `--json -`.
/// The lines a fit prints on standard error while it runs (`--progress`, the sentence of a
/// cancel) come back with the other notes.
pub fn run(args: &[String], stdin: &mut dyn Read) -> Result<Output, CliError> {
    let mut lines = Vec::new();
    let mut output = run_with(
        args,
        stdin,
        &mut |line| lines.push(line.to_owned()),
        &|| false,
    )?;
    output.notes.extend(lines);
    Ok(output)
}

/// As [`run`], for a program that shows the lines of a fit as they come: `sink` receives each
/// one (they are not in `Output::notes`), and `interrupted` is asked between two iterations of a
/// fit (Ctrl-C): when it says true the fit is cancelled like `--cancel-after`.
pub fn run_with(
    args: &[String],
    stdin: &mut dyn Read,
    sink: &mut dyn FnMut(&str),
    interrupted: &dyn Fn() -> bool,
) -> Result<Output, CliError> {
    match parse_args(args)? {
        Action::Help => Ok(Output {
            stdout: format!("{USAGE}\n"),
            notes: Vec::new(),
            exit_code: 0,
        }),
        Action::Commands(format) => list_commands(format),
        Action::Run(run) => run_command(&run, stdin, sink, interrupted),
    }
}

/// The command line as text. An argument that is not valid Unicode is refused with its position
/// (`std::env::args` would panic on it).
pub fn text_args(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<Vec<String>, CliError> {
    args.into_iter()
        .enumerate()
        .map(|(i, a)| {
            a.into_string()
                .map_err(|_| CliError(format!("argument {} is not valid text", i + 1)))
        })
        .collect()
}

/// The ids of the commands the CLI can run.
pub fn commands() -> Vec<&'static str> {
    command_ids()
}

#[cfg(test)]
mod tests;

/// A JSON number as CSV text: the shortest form that reads back (`2`, not `2.0`).
fn number(v: &Value) -> String {
    v.as_f64().map_or_else(|| v.to_string(), |x| format!("{x}"))
}
