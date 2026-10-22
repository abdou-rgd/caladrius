//! What the page does for the interface, apart from the browser itself: the interface's requests
//! become browser effects (a file picker, a download, a value in local storage), and the files the
//! browser reads come back to the interface. The browser sits behind [`Browser`], so this part is
//! tested on the desktop with a fake one.

use caladrius_ui::projectfile::is_project_file;
use caladrius_ui::{Action, Request, UiApp};

/// The key the settings are stored under in the browser's local storage.
pub const SETTINGS_KEY: &str = "caladrius.settings";

/// What a file picker was opened for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Csv,
    Project,
}

impl Pick {
    /// The `accept` attribute of the file input: the extensions the picker offers first.
    pub fn accept(self) -> &'static str {
        match self {
            Pick::Csv => ".csv,.txt,.tsv,text/csv,text/plain",
            Pick::Project => ".json,application/json",
        }
    }
}

/// A file the browser read for the page: chosen in a picker (`pick` says which one) or dropped on
/// the page (`pick` is `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    pub pick: Option<Pick>,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The browser, as the page uses it.
pub trait Browser {
    /// Opens a file picker; the file comes back later through [`Browser::take_incoming`].
    fn pick(&mut self, pick: Pick);
    /// Hands `bytes` to the browser as a download named `name`.
    fn download(&mut self, name: &str, bytes: &[u8]) -> Result<(), String>;
    /// Keeps the settings (JSON text) in local storage.
    fn store_settings(&mut self, text: &str) -> Result<(), String>;
    /// The files read since the last call, or why one could not be read.
    fn take_incoming(&mut self) -> Vec<Result<Incoming, String>>;
    /// The title of the tab.
    fn set_title(&mut self, title: &str);
    /// Whether leaving the page would lose work: the browser then asks before closing the tab.
    fn set_unsaved(&mut self, unsaved: bool);
}

/// The file name a saved project is downloaded under: the suggested name, with the project
/// extension when it has none (the same rule as the desktop's save dialog).
pub fn project_file_name(suggested: &str) -> String {
    if suggested.to_ascii_lowercase().ends_with(".json") {
        suggested.to_owned()
    } else {
        format!("{suggested}.caladrius.json")
    }
}

/// The interface and the browser it runs in.
pub struct Web<B: Browser> {
    pub ui: UiApp,
    pub browser: B,
    /// The title last given to the tab.
    title: String,
    /// What the browser was last told about unsaved work.
    unsaved: bool,
}

impl<B: Browser> Web<B> {
    /// A page around `ui`. A tab is closed by the browser, so the interface offers no "Quit".
    pub fn new(mut ui: UiApp, browser: B) -> Web<B> {
        ui.set_can_quit(false);
        Web {
            ui,
            browser,
            title: String::new(),
            unsaved: false,
        }
    }

    /// Reads the stored settings and the language of the browser into the interface, as the
    /// desktop does with its configuration file and the system locale: with nothing stored (a
    /// first visit) the language suggests the decimal mark.
    pub fn start_settings(
        &mut self,
        locale: Option<String>,
        stored: Result<Option<String>, String>,
    ) {
        self.ui.set_system_locale(locale.as_deref());
        match stored {
            Ok(Some(text)) => {
                self.ui.set_settings_json(&text);
            }
            Ok(None) => self.ui.apply_locale_suggestion(),
            Err(message) => self.ui.perform(vec![Action::Notice(message)]),
        }
    }

    /// A file the browser read. A CSV goes to the import preview; a project chosen in "Open
    /// project" opens (the question about unsaved changes came before the picker); a dropped
    /// project asks that question first.
    pub fn receive(&mut self, file: Incoming) {
        match file.pick {
            Some(Pick::Csv) => self.ui.load_csv(&file.name, &file.bytes),
            Some(Pick::Project) => {
                self.ui.open_project(&file.name, &file.bytes);
            }
            None if is_project_file(&file.name) => {
                self.ui.open_project_guarded(&file.name, file.bytes, None);
            }
            None => self.ui.load_csv(&file.name, &file.bytes),
        }
    }

    /// Everything the page does around one frame of the interface, except drawing it: the
    /// files that arrived, then the interface's requests, then the tab's title and the
    /// question before leaving.
    pub fn before_frame(&mut self) {
        for file in self.browser.take_incoming() {
            match file {
                Ok(file) => self.receive(file),
                Err(message) => self.ui.perform(vec![Action::Notice(message)]),
            }
        }
    }

    /// See [`Web::before_frame`]: what follows the frame.
    pub fn after_frame(&mut self) {
        self.serve();
        let title = self.ui.window_title();
        if title != self.title {
            self.browser.set_title(&title);
            self.title = title;
        }
        let unsaved = self.ui.is_dirty();
        if unsaved != self.unsaved {
            self.browser.set_unsaved(unsaved);
            self.unsaved = unsaved;
        }
    }

    /// Does what the interface asked, until it has nothing more to ask.
    pub fn serve(&mut self) {
        loop {
            let requests = self.ui.take_requests();
            if requests.is_empty() {
                break;
            }
            for request in requests {
                match request {
                    Request::PickCsv => self.browser.pick(Pick::Csv),
                    Request::OpenProject => self.browser.pick(Pick::Project),
                    // A page cannot write over a file on the disk: every save is a download,
                    // and "Save" downloads again under the same name.
                    Request::SaveProject {
                        suggested, bytes, ..
                    } => {
                        let name = project_file_name(&suggested);
                        match self.browser.download(&name, &bytes) {
                            Ok(()) => self.ui.project_saved(&name),
                            Err(message) => self.ui.project_save_failed(&message),
                        }
                    }
                    // The interface offers no "Quit" in a tab ([`Web::new`]); nothing to close.
                    Request::Close => {}
                    Request::SaveSettings(text) => {
                        if let Err(message) = self.browser.store_settings(&text) {
                            self.ui.perform(vec![Action::Notice(message)]);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use caladrius_ui::settings::{DecimalMark, Settings};

    /// A browser that records what the page asked of it.
    #[derive(Default)]
    struct Fake {
        picks: Vec<Pick>,
        downloads: Vec<(String, Vec<u8>)>,
        stored: Option<String>,
        inbox: Vec<Result<Incoming, String>>,
        title: String,
        unsaved: bool,
        fail_download: bool,
    }

    impl Browser for Fake {
        fn pick(&mut self, pick: Pick) {
            self.picks.push(pick);
        }
        fn download(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
            if self.fail_download {
                return Err("the browser refused the download".to_owned());
            }
            self.downloads.push((name.to_owned(), bytes.to_vec()));
            Ok(())
        }
        fn store_settings(&mut self, text: &str) -> Result<(), String> {
            self.stored = Some(text.to_owned());
            Ok(())
        }
        fn take_incoming(&mut self) -> Vec<Result<Incoming, String>> {
            std::mem::take(&mut self.inbox)
        }
        fn set_title(&mut self, title: &str) {
            self.title = title.to_owned();
        }
        fn set_unsaved(&mut self, unsaved: bool) {
            self.unsaved = unsaved;
        }
    }

    const CSV: &str = "time,conc,dose\n0,0,10\n1,5,10\n2,3,10\n4,1,10\n";

    fn page() -> Web<Fake> {
        Web::new(UiApp::new(), Fake::default())
    }

    fn csv(pick: Option<Pick>, name: &str) -> Incoming {
        Incoming {
            pick,
            name: name.to_owned(),
            bytes: CSV.as_bytes().to_vec(),
        }
    }

    /// Imports the CSV, then saves: the bytes of the project the page downloaded.
    fn saved_project() -> Vec<u8> {
        let mut web = page();
        web.receive(csv(Some(Pick::Csv), "study.csv"));
        web.ui
            .perform(vec![Action::ImportConfirm, Action::SaveProject]);
        web.serve();
        web.browser.downloads.pop().map(|(_, b)| b).unwrap()
    }

    #[test]
    fn open_csv_and_open_project_open_the_picker_for_their_kind() {
        let mut web = page();
        web.ui.perform(vec![Action::OpenCsv, Action::OpenProject]);
        web.serve();
        assert_eq!(web.browser.picks, [Pick::Csv, Pick::Project]);
        assert!(Pick::Csv.accept().contains(".csv"));
        assert!(Pick::Project.accept().contains(".json"));
    }

    #[test]
    fn a_chosen_or_dropped_csv_shows_the_import_preview() {
        for pick in [Some(Pick::Csv), None] {
            let mut web = page();
            web.browser.inbox.push(Ok(csv(pick, "study.csv")));
            web.before_frame();
            assert!(web.ui.pending_import().is_some());
        }
    }

    #[test]
    fn a_file_the_browser_could_not_read_says_why() {
        let mut web = page();
        web.browser
            .inbox
            .push(Err("study.csv could not be read".to_owned()));
        web.before_frame();
        assert_eq!(web.ui.notice_text(), Some("study.csv could not be read"));
    }

    #[test]
    fn saving_downloads_the_project_and_marks_it_saved() {
        let mut web = page();
        web.receive(csv(Some(Pick::Csv), "study.csv"));
        web.ui.perform(vec![Action::ImportConfirm]);
        web.after_frame();
        assert!(web.ui.is_dirty() && web.browser.unsaved);
        web.ui.perform(vec![Action::SaveProject]);
        web.after_frame();
        let (name, bytes) = web.browser.downloads.last().unwrap();
        assert!(name.ends_with(".caladrius.json"), "{name}");
        assert!(!bytes.is_empty());
        assert!(!web.ui.is_dirty() && !web.browser.unsaved);
        assert_eq!(web.browser.title, web.ui.window_title());
        // "Save" again downloads again, under the same name.
        web.ui.perform(vec![Action::SaveProject]);
        web.serve();
        assert_eq!(web.browser.downloads.len(), 2);
        assert_eq!(&web.browser.downloads[1].0, &web.browser.downloads[0].0);
    }

    #[test]
    fn a_refused_download_leaves_the_project_unsaved_and_says_why() {
        let mut web = page();
        web.receive(csv(Some(Pick::Csv), "study.csv"));
        web.ui.perform(vec![Action::ImportConfirm]);
        web.browser.fail_download = true;
        web.ui.perform(vec![Action::SaveProject]);
        web.serve();
        assert!(web.ui.is_dirty());
        assert!(
            web.ui.notice_text().unwrap().contains("refused"),
            "{:?}",
            web.ui.notice_text()
        );
    }

    #[test]
    fn a_downloaded_project_opens_again_chosen_or_dropped() {
        let bytes = saved_project();
        let mut web = page();
        web.receive(Incoming {
            pick: Some(Pick::Project),
            name: "study.caladrius.json".to_owned(),
            bytes: bytes.clone(),
        });
        assert_eq!(web.ui.file_name(), Some("study.caladrius.json"));
        assert!(!web.ui.is_dirty());
        // Dropped on a page with unsaved work: the question first, then the file.
        let mut web = page();
        web.receive(csv(Some(Pick::Csv), "other.csv"));
        web.ui.perform(vec![Action::ImportConfirm]);
        web.receive(Incoming {
            pick: None,
            name: "study.caladrius.json".to_owned(),
            bytes,
        });
        assert!(web.ui.guard().is_some());
        web.ui.perform(vec![Action::GuardDiscard]);
        assert_eq!(web.ui.file_name(), Some("study.caladrius.json"));
    }

    #[test]
    fn a_tab_offers_no_quit() {
        let mut web = page();
        assert!(web.ui.palette_entries().iter().all(|e| e.key != "ui:quit"));
        web.ui.perform(vec![Action::Quit]);
        assert!(web.ui.guard().is_none());
        web.serve();
        assert!(web.browser.downloads.is_empty() && web.browser.picks.is_empty());
    }

    #[test]
    fn the_settings_come_from_local_storage_or_from_the_language_and_are_stored_there() {
        let mut web = page();
        web.start_settings(Some("fr-FR".to_owned()), Ok(None));
        assert_eq!(web.ui.settings.decimal_mark, DecimalMark::Comma);
        let stored = Settings {
            significant_digits: 6,
            ..Settings::default()
        };
        let mut web = page();
        web.start_settings(Some("fr-FR".to_owned()), Ok(Some(stored.to_json())));
        assert_eq!(web.ui.settings.significant_digits, 6);
        assert_eq!(web.ui.settings.decimal_mark, DecimalMark::Point);
        let mut web = page();
        web.start_settings(None, Err("local storage is disabled".to_owned()));
        assert_eq!(web.ui.notice_text(), Some("local storage is disabled"));
        // A change of setting is stored.
        let mut web = page();
        web.ui.perform(vec![Action::SetSetting {
            key: "significant_digits".to_owned(),
            value: serde_json::json!(5),
        }]);
        web.serve();
        assert!(
            web.browser
                .stored
                .unwrap()
                .contains("\"significant_digits\": 5")
        );
    }

    #[test]
    fn a_bare_name_gets_the_project_extension() {
        assert_eq!(project_file_name("study"), "study.caladrius.json");
        assert_eq!(
            project_file_name("study.caladrius.json"),
            "study.caladrius.json"
        );
        assert_eq!(project_file_name("old.JSON"), "old.JSON");
    }
}
