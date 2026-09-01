/// Returns the base URL of the current window origin (e.g. "https://example.com" or "http://localhost:7654")
pub fn get_base_url() -> String {
    if let Some(w) = web_sys::window() {
        if let Ok(loc) = w.location().origin() {
            return loc;
        }
    }
    String::new()
}

/// Copies text to clipboard using navigator.clipboard
pub fn copy_to_clipboard(text: &str) {
    let text_owned = text.to_string();
    leptos::spawn_local(async move {
        if let Some(w) = web_sys::window() {
            let nav = w.navigator();
            let clipboard = nav.clipboard();
            let promise = clipboard.write_text(&text_owned);
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
        }
    });
}
