mod api;
mod app;
mod auth;
mod components;
mod models;
mod utils;

use app::App;
use leptos::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App/> })
}
