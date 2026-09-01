use crate::auth::{logout, use_auth};
use leptos::*;
use leptos_router::*;

#[component]
pub fn AdminLayout() -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();

    // Guard route
    let nav_guard = navigate.clone();
    create_effect(move |_| {
        if !auth.is_authenticated.get() {
            nav_guard("/admin/dashboard/login", Default::default());
        }
    });

    let nav_logout = navigate.clone();
    let handle_logout = move |_| {
        logout();
        nav_logout("/admin/dashboard/login", Default::default());
    };

    view! {
        <div style="min-height: 100vh; padding: 24px; max-width: 1200px; margin: 0 auto;">
            <header style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 32px; border-bottom: 1px solid var(--border); padding-bottom: 16px;">
                <h1>"Ruxy Admin"</h1>
                <div style="display: flex; gap: 16px; align-items: center;">
                    <A href="/admin/dashboard" class="nav-link">"Routes"</A>
                    <button class="btn btn-danger" on:click=handle_logout>"Logout"</button>
                </div>
            </header>
            <main>
                <Outlet/>
            </main>
        </div>
    }
}
