use crate::auth::{login, use_auth};
use leptos::*;
use leptos_router::*;

#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();

    let (password, set_password) = create_signal(String::new());
    let (error_msg, set_error_msg) = create_signal(None::<String>);
    let (is_loading, set_is_loading) = create_signal(false);

    // Redirect if already authenticated
    let nav_redirect = navigate.clone();
    create_effect(move |_| {
        if auth.is_authenticated.get() {
            nav_redirect("/admin/dashboard", Default::default());
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_is_loading.set(true);
        set_error_msg.set(None);
        let nav = navigate.clone();

        spawn_local(async move {
            match login(password.get()).await {
                Ok(_) => {
                    nav("/admin/dashboard", Default::default());
                }
                Err(e) => {
                    set_error_msg.set(Some(e));
                }
            }
            set_is_loading.set(false);
        });
    };

    view! {
        <div style="display:flex; justify-content:center; align-items:center; height:100vh;">
            <div class="card" style="width: 100%; max-width: 400px;">
                <h2 style="text-align: center; margin-bottom: 24px;">"🔐 Ruxy Admin"</h2>

                <form on:submit=on_submit>
                    <div>
                        <label style="display: block; margin-bottom: 8px;">"Password"</label>
                        <input
                            type="password"
                            class="input"
                            prop:value=password
                            on:input=move |ev| set_password.set(event_target_value(&ev))
                            disabled=is_loading
                            required
                        />
                    </div>

                    <button type="submit" class="btn" style="width: 100%;" disabled=is_loading>
                        {move || if is_loading.get() { "Signing in..." } else { "🔑 Sign In" }}
                    </button>

                    {move || error_msg.get().map(|msg| view! {
                        <div style="margin-top: 16px; color: var(--danger); text-align: center;">
                            "⚠️ " {msg}
                        </div>
                    })}
                </form>
            </div>
        </div>
    }
}
