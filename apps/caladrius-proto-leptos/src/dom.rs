//! The few things the screen asks of the browser that the framework does not give: the frame that
//! follows a render (for the measure), the clipboard, a CSS variable as a number, the query string.

use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageChannel, Window};

pub fn window() -> Option<Window> {
    web_sys::window()
}

/// Milliseconds since `performance.timeOrigin`.
pub fn now() -> f64 {
    window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// The frame that shows what has just been rendered: after the next paint. A `requestAnimationFrame`
/// callback runs before the paint, and a message posted from it is delivered after it. Gives the
/// time of that message and whether the wait ran out first.
pub async fn next_paint(timeout_ms: i32) -> (f64, bool) {
    let Some(win) = window() else {
        return (now(), true);
    };
    let timeout_window = win.clone();
    let promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        let on_timeout = {
            let resolve = resolve.clone();
            Closure::once_into_js(move || {
                let _ = resolve.call1(&JsValue::NULL, &JsValue::from_f64(-1.0));
            })
        };
        let _ = timeout_window.set_timeout_with_callback_and_timeout_and_arguments_0(
            on_timeout.unchecked_ref(),
            timeout_ms,
        );
        let on_frame = Closure::once_into_js(move || {
            let Ok(channel) = MessageChannel::new() else {
                let _ = resolve.call1(&JsValue::NULL, &JsValue::from_f64(now()));
                return;
            };
            let keep = channel.clone();
            let on_message = Closure::once_into_js(move || {
                let _keep = &keep;
                let _ = resolve.call1(&JsValue::NULL, &JsValue::from_f64(now()));
            });
            channel
                .port1()
                .set_onmessage(Some(on_message.unchecked_ref()));
            let _ = channel.port2().post_message(&JsValue::from_f64(0.0));
        });
        let _ = timeout_window.request_animation_frame(on_frame.unchecked_ref());
    });
    match JsFuture::from(promise).await {
        Ok(v) => match v.as_f64() {
            Some(t) if t >= 0.0 => (t, false),
            _ => (now(), true),
        },
        Err(_) => (now(), true),
    }
}

/// Copies text to the clipboard; false when the browser does not allow it here. (`web-sys` keeps the
/// clipboard behind an unstable flag, so it is reached by name.)
pub async fn copy_text(text: &str) -> bool {
    let Some(win) = window() else { return false };
    let navigator: JsValue = win.navigator().into();
    let Ok(clipboard) = Reflect::get(&navigator, &JsValue::from_str("clipboard")) else {
        return false;
    };
    if clipboard.is_undefined() || clipboard.is_null() {
        return false;
    }
    let Ok(write) = Reflect::get(&clipboard, &JsValue::from_str("writeText")) else {
        return false;
    };
    let Ok(write) = write.dyn_into::<Function>() else {
        return false;
    };
    let Ok(promise) = write.call1(&clipboard, &JsValue::from_str(text)) else {
        return false;
    };
    match promise.dyn_into::<Promise>() {
        Ok(p) => JsFuture::from(p).await.is_ok(),
        Err(_) => false,
    }
}

/// A size token of `theme.json` (a CSS variable such as `--size-hit-radius`) as a number of pixels.
pub fn token(name: &str, default: f64) -> f64 {
    let Some(win) = window() else { return default };
    let Some(root) = win.document().and_then(|d| d.document_element()) else {
        return default;
    };
    let Ok(Some(style)) = win.get_computed_style(&root) else {
        return default;
    };
    style
        .get_property_value(name)
        .ok()
        .and_then(|v| v.trim().trim_end_matches("px").parse::<f64>().ok())
        .unwrap_or(default)
}

/// The value of a key of the query string (`?theme=dark`).
pub fn query(key: &str) -> Option<String> {
    let search = window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == key).then(|| v.to_owned())
    })
}

/// Waits `ms` milliseconds.
pub async fn sleep(ms: i32) {
    let Some(win) = window() else { return };
    let promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = JsFuture::from(promise).await;
}
