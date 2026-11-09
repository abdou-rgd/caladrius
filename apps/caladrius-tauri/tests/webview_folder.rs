//! The webview folder (`%LOCALAPPDATA%\org.caladrius.shell` on Windows): when it cannot be created the
//! shell says so in one sentence and exits, it does not panic inside the web view.
//!
//! The tests write nothing: a path with a NUL character is refused by the operating system on every
//! platform, whatever the rights of the user (the executables of the tests may not be allowed to write
//! at all, which is the very case this covers).

use caladrius_tauri_lib::{ensure_webview_folder, webview_folder};
use std::ffi::OsString;
use std::path::PathBuf;

#[test]
fn a_folder_that_cannot_be_created_gives_a_sentence() -> Result<(), String> {
    let folder = PathBuf::from("caladrius-test\0org.caladrius.shell");
    let sentence = ensure_webview_folder(&folder)
        .err()
        .ok_or("a path with a NUL character should not be created")?;
    assert!(sentence.starts_with("Caladrius cannot open its window"), "{sentence}");
    assert!(sentence.contains("org.caladrius.shell"), "{sentence}");
    assert!(sentence.ends_with("start Caladrius again."), "{sentence}");
    assert!(!sentence.contains('\n'), "one sentence on one line");
    Ok(())
}

#[test]
fn the_folder_is_local_app_data_plus_the_identifier_on_windows_only() {
    let base = PathBuf::from("local-app-data");
    let folder = webview_folder(Some(OsString::from(&base)), "org.caladrius.shell");
    if cfg!(windows) {
        assert_eq!(folder, Some(base.join("org.caladrius.shell")));
        assert_eq!(webview_folder(None, "org.caladrius.shell"), None);
        assert_eq!(webview_folder(Some(OsString::new()), "org.caladrius.shell"), None);
    } else {
        assert_eq!(folder, None);
    }
}
