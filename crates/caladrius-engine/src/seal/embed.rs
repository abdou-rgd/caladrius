//! A seal inside a file: the same facts as one line of text, as a comment of a CSV, as a comment
//! of an SVG, or as members of a JSON object.
//!
//! The commands of the engine return clean results (a CSV is a CSV, an SVG is an SVG): a seal
//! cannot sit inside the text it seals. The programs that write a file (the desktop application,
//! the command line) add the stamp of the command that produced it with these functions, and
//! `data.import` skips a stamp line at the top of a CSV, so a table exported from Caladrius can
//! be imported again. A stamp holds no time: the same command on the same data gives the same
//! file, byte for byte; the time of a command is in the history of the project, next to its seal.

use serde_json::Value;

use super::Seal;

/// What a stamp line starts with after its comment marker. `# caladrius engine=... seal=...`.
pub const STAMP_MARK: &str = "caladrius ";

/// The facts of a seal on one line: `caladrius engine=0.3.2 commit=... command=export.table
/// seal=... input=...`, without the members that are not known.
pub fn stamp_line(command: &str, seal: &Seal) -> String {
    let mut line = String::from("caladrius");
    for (key, value) in [
        ("engine", seal.engine.as_deref()),
        ("commit", seal.commit.as_deref()),
        ("command", Some(command)),
        ("seal", Some(seal.sha256.as_str())),
        ("input", seal.input.as_deref()),
    ] {
        if let Some(value) = value {
            line.push(' ');
            line.push_str(key);
            line.push('=');
            line.push_str(&one_word(value));
        }
    }
    line
}

/// A value without spaces and without `--` (which a comment of XML cannot hold).
fn one_word(value: &str) -> String {
    let mut word: String = value
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    while word.contains("--") {
        word = word.replace("--", "-");
    }
    word
}

/// A CSV with the stamp as a comment line at the top: `# caladrius ...`.
pub fn csv_with_stamp(csv: &str, line: &str) -> String {
    format!("# {line}\n{csv}")
}

/// The CSV text without the stamp lines at its top (and without a byte order mark before them);
/// the text itself when it has none.
pub fn strip_stamp(csv: &str) -> &str {
    let body = csv.strip_prefix('\u{feff}').unwrap_or(csv);
    let mut rest = body;
    let mut stamped = false;
    while let Some(after) = rest.strip_prefix('#') {
        if !after.trim_start_matches(' ').starts_with(STAMP_MARK) {
            break;
        }
        stamped = true;
        rest = match after.split_once('\n') {
            Some((_, tail)) => tail,
            None => "",
        };
    }
    if stamped { rest } else { csv }
}

/// An SVG with the stamp as an XML comment right after the opening tag of the root, so the file
/// still starts with `<svg`. The text itself when it has no opening tag.
pub fn svg_with_stamp(svg: &str, line: &str) -> String {
    match svg.find('>') {
        Some(end) => {
            let (head, tail) = svg.split_at(end + 1);
            format!("{head}\n<!-- {} -->{tail}", one_word_comment(line))
        }
        None => svg.to_owned(),
    }
}

/// The line as the inside of an XML comment: no `--`, and no `>` that could be read as the end.
fn one_word_comment(line: &str) -> String {
    let mut text = line.replace('>', "&gt;");
    while text.contains("--") {
        text = text.replace("--", "-");
    }
    text
}

/// A JSON object with the seal as a `seal` member, unless the object already has a member of that
/// name. Anything that is not an object is returned as it is.
pub fn json_with_seal(mut value: Value, seal: &Seal) -> Value {
    if let Value::Object(map) = &mut value {
        if let Ok(seal) = serde_json::to_value(seal) {
            map.entry("seal").or_insert(seal);
        }
    }
    value
}
