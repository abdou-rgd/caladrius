//! How a typed table becomes a worksheet: the same two commands as a file, `data.preview` for the
//! checks and `data.import` to store it, with the roles and units of the columns said explicitly.
//! A table with a failed check waits, like an import, until the person says they looked at it.

use serde_json::Value;

use crate::app::{Notice, NoticeKind, Selection, UiApp};
use crate::entry::DataEntry;

impl UiApp {
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
