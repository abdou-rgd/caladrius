//! The browser side of the page (wasm32 only): the canvas eframe draws on, the file picker, the
//! download, local storage, the tab's title and the question before leaving with unsaved work.
//! Nothing leaves the browser: files are read in the page and saved as downloads.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use caladrius_ui::UiApp;
use eframe::egui;
use js_sys::{Array, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, HtmlCanvasElement, HtmlInputElement, Url};

use crate::host::{Browser, Incoming, Pick, SETTINGS_KEY, Web};

/// The id of the canvas in `index.html`, and of the text shown while the page loads.
const CANVAS: &str = "caladrius";
const LOADING: &str = "loading";

/// A JavaScript error as a sentence.
fn js_text(context: &str, value: &wasm_bindgen::JsValue) -> String {
    match value.as_string() {
        Some(text) => format!("{context}: {text}"),
        None => format!("{context}: {value:?}"),
    }
}

/// The browser as the page uses it.
struct Tab {
    /// Files read by a picker, waiting for the next frame.
    inbox: Rc<RefCell<Vec<Result<Incoming, String>>>>,
    /// Read by the `beforeunload` handler.
    unsaved: Rc<Cell<bool>>,
    /// To draw a frame when a file has been read.
    ctx: egui::Context,
}

impl Tab {
    fn new(ctx: egui::Context) -> Tab {
        let unsaved = Rc::new(Cell::new(false));
        if let Some(window) = web_sys::window() {
            let flag = Rc::clone(&unsaved);
            // Closing or reloading the tab with unsaved work: the browser asks first.
            let handler =
                Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
                    if flag.get() {
                        event.prevent_default();
                    }
                });
            window.set_onbeforeunload(Some(handler.as_ref().unchecked_ref()));
            // The handler lives as long as the page.
            handler.forget();
        }
        Tab {
            inbox: Rc::new(RefCell::new(Vec::new())),
            unsaved,
            ctx,
        }
    }
}

/// Reads the file chosen in `input` and puts it in `inbox`.
fn read_chosen(
    input: &HtmlInputElement,
    pick: Pick,
    inbox: Rc<RefCell<Vec<Result<Incoming, String>>>>,
    ctx: egui::Context,
) {
    let Some(file) = input.files().and_then(|list| list.get(0)) else {
        return;
    };
    let name = file.name();
    wasm_bindgen_futures::spawn_local(async move {
        let read = JsFuture::from(file.array_buffer()).await;
        let item = match read {
            Ok(buffer) => Ok(Incoming {
                pick: Some(pick),
                name,
                bytes: Uint8Array::new(&buffer).to_vec(),
            }),
            Err(e) => Err(js_text(&format!("{name} could not be read"), &e)),
        };
        inbox.borrow_mut().push(item);
        ctx.request_repaint();
    });
}

impl Browser for Tab {
    fn pick(&mut self, pick: Pick) {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(input) = document
            .create_element("input")
            .ok()
            .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
        else {
            return;
        };
        input.set_type("file");
        input.set_accept(pick.accept());
        let inbox = Rc::clone(&self.inbox);
        let ctx = self.ctx.clone();
        let chosen = input.clone();
        let on_change = Closure::once_into_js(move || read_chosen(&chosen, pick, inbox, ctx));
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
    }

    fn download(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let failed = |e: wasm_bindgen::JsValue| js_text(&format!("{name} could not be saved"), &e);
        let window =
            web_sys::window().ok_or_else(|| format!("{name} could not be saved: no window"))?;
        let document = window
            .document()
            .ok_or_else(|| format!("{name} could not be saved: no document"))?;
        let parts = Array::of1(&Uint8Array::from(bytes));
        let options = BlobPropertyBag::new();
        options.set_type("application/json");
        let blob =
            Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(failed)?;
        let url = Url::create_object_url_with_blob(&blob).map_err(failed)?;
        let link = document
            .create_element("a")
            .map_err(failed)?
            .dyn_into::<HtmlAnchorElement>()
            .map_err(|e| failed(e.into()))?;
        link.set_href(&url);
        link.set_download(name);
        link.click();
        // The browser has started the download; the address is released a little later.
        let release = Closure::once_into_js(move || {
            let _ = Url::revoke_object_url(&url);
        });
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(release.unchecked_ref(), 10_000);
        Ok(())
    }

    fn store_settings(&mut self, text: &str) -> Result<(), String> {
        let failed = || {
            "the settings could not be kept: this browser does not allow local storage".to_owned()
        };
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .ok_or_else(failed)?;
        storage.set_item(SETTINGS_KEY, text).map_err(|_| failed())
    }

    fn take_incoming(&mut self) -> Vec<Result<Incoming, String>> {
        std::mem::take(&mut *self.inbox.borrow_mut())
    }

    fn set_title(&mut self, title: &str) {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            document.set_title(title);
        }
    }

    fn set_unsaved(&mut self, unsaved: bool) {
        self.unsaved.set(unsaved);
    }
}

/// The settings kept by an earlier visit, and the language of the browser.
fn stored_settings() -> (Option<String>, Result<Option<String>, String>) {
    let Some(window) = web_sys::window() else {
        return (None, Ok(None));
    };
    let locale = window.navigator().language();
    let stored = match window.local_storage() {
        Ok(Some(storage)) => storage
            .get_item(SETTINGS_KEY)
            .map_err(|e| js_text("the stored settings could not be read", &e)),
        // Local storage disabled: the defaults, as on a first visit.
        _ => Ok(None),
    };
    (locale, stored)
}

struct Page {
    web: Web<Tab>,
}

impl eframe::App for Page {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // A file dropped on the page arrives with its bytes (there is no path in a browser).
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Some(bytes) = file.bytes {
                self.web.receive(Incoming {
                    pick: None,
                    name: file.name,
                    bytes: bytes.to_vec(),
                });
            }
        }
        self.web.before_frame();
        self.web.ui.ui(ctx);
        self.web.after_frame();
    }
}

/// The time of the browser, in seconds since 1970 (the engine dates its history with it; it reads
/// no clock itself).
fn clock() -> Option<i64> {
    let milliseconds = js_sys::Date::now();
    milliseconds
        .is_finite()
        .then(|| (milliseconds / 1000.0).floor() as i64)
}

/// Starts the interface on the page's canvas; if it cannot start (no WebGL), the loading text
/// says so instead.
pub fn start() {
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(canvas) = document
            .get_element_by_id(CANVAS)
            .and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok())
        else {
            return;
        };
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    let mut web = Web::new(UiApp::with_clock(clock), Tab::new(cc.egui_ctx.clone()));
                    let (locale, stored) = stored_settings();
                    web.start_settings(locale, stored);
                    Ok(Box::new(Page { web }))
                }),
            )
            .await;
        if let Some(loading) = document.get_element_by_id(LOADING) {
            match started {
                Ok(()) => loading.remove(),
                Err(e) => loading.set_text_content(Some(&js_text(
                    "Caladrius could not start in this browser (it needs WebGL 2)",
                    &e,
                ))),
            }
        }
    });
}
