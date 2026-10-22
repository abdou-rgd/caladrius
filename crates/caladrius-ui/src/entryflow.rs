//! How a typed table becomes a worksheet: the same two commands as a file, `data.preview` for the
//! checks and `data.import` to store it, with the roles and units of the columns said explicitly.
//! A table with a failed check waits, like an import, until the person says they looked at it.

use serde_json::{Value, json};

use crate::app::{Notice, NoticeKind, Selection, UiApp};
use crate::entry::DataEntry;
use crate::unitpick::Accepted;

impl UiApp {
    /// The units the engine recognises, asked once.
    pub(crate) fn fetch_units(&mut self) {
        if self.units == Accepted::default() {
            if let Ok(answer) = self.call("units.check", Value::Null) {
                self.units = answer
                    .get("accepted")
                    .cloned()
                    .and_then(|a| serde_json::from_value(a).ok())
                    .unwrap_or_default();
            }
        }
    }

    /// The engine's checks of the table's units (`units.check`), made again when a unit changed
    /// and the person left the unit cell.
    pub(crate) fn check_entry_units(&mut self) {
        self.fetch_units();
        let Some(entry) = self.state.entry.as_ref() else {
            return;
        };
        let units = entry.units();
        let current = entry
            .unit_checks
            .as_ref()
            .is_some_and(|(checked, _)| *checked == units);
        if entry.unit_focus || current {
            return;
        }
        let given = |u: &String| if u.is_empty() { Value::Null } else { json!(u) };
        let [time, concentration, dose] = &units;
        let messages = match self.call(
            "units.check",
            json!({ "time": given(time), "concentration": given(concentration), "dose": given(dose) }),
        ) {
            Ok(answer) => answer
                .get("warnings")
                .and_then(Value::as_array)
                .map(|w| {
                    w.iter()
                        .filter_map(|w| w.get("message").and_then(Value::as_str))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            Err(message) => vec![message],
        };
        if let Some(entry) = self.state.entry.as_mut() {
            entry.unit_checks = Some((units, messages));
        }
    }

    /// Shows the table being typed, or a new one.
    pub(crate) fn open_entry(&mut self) {
        if self.state.entry.is_none() {
            self.state.entry = Some(DataEntry::new());
        }
        self.state.selection = Selection::Entry;
        self.notice = None;
    }

    pub(crate) fn cancel_entry(&mut self) {
        self.state.entry = None;
        self.state.selection = self
            .overview
            .worksheets
            .last()
            .map_or(Selection::Welcome, |w| Selection::Worksheet(w.id));
        if let Selection::Worksheet(id) = self.state.selection {
            self.refresh_sheet(id);
        }
    }

    /// "Create worksheet": checks the table through `data.preview`, then imports it when it has
    /// no failed check or the person acknowledged the checks of this very table.
    pub(crate) fn create_from_entry(&mut self) {
        let Some(mut entry) = self.state.entry.take() else {
            return;
        };
        let acknowledged =
            entry.review_is_current() && entry.review.as_ref().is_some_and(|r| r.acknowledged);
        let mut review = entry.pending();
        let answer = self.call("data.preview", review.preview_params());
        review.adopt(answer);
        review.acknowledged = acknowledged;
        let params = review
            .can_import()
            .then(|| review.import_params())
            .flatten();
        let Some(mut params) = params else {
            if review.error.is_none() {
                self.notice = Some(Notice {
                    kind: NoticeKind::Info,
                    text:
                        "The engine found something to check in these data: see above the button."
                            .to_owned(),
                });
            }
            entry.review = Some(review);
            self.state.entry = Some(entry);
            return;
        };
        params["columns"] = entry.import_columns();
        match self.call("data.import", params) {
            Ok(done) => {
                let id = done
                    .pointer("/worksheet/id")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let rows = done
                    .pointer("/worksheet/rows")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                self.refresh_overview();
                self.state.selection = Selection::Worksheet(id);
                self.refresh_sheet(id);
                self.notice = Some(Notice {
                    kind: NoticeKind::Info,
                    text: format!(
                        "Created the worksheet {} with {rows} rows.",
                        entry.worksheet_name()
                    ),
                });
            }
            Err(message) => {
                review.error = Some(message);
                entry.review = Some(review);
                self.state.entry = Some(entry);
            }
        }
    }
}
