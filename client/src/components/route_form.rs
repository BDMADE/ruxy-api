use crate::api::{api_delete, api_get, api_post};
use crate::auth::use_auth;
use crate::models::{ApiItemResponse, ApiResponse, RouteEntry};
use leptos::*;
use leptos_router::*;

#[component]
pub fn RouteForm() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let navigate = use_navigate();

    let is_edit = create_memo(move |_| params.with(|p| p.get("key").is_some()));

    let (key, set_key) = create_signal(String::new());
    let (value, set_value) = create_signal(String::new());
    let (error_msg, set_error_msg) = create_signal(None::<String>);
    let (is_loading, set_is_loading) = create_signal(false);

    // Load existing data if edit mode
    create_effect(move |_| {
        if let Some(edit_key) = params.with(|p| p.get("key").cloned()) {
            set_key.set(edit_key.clone());
            let auth_token = auth.token.get_untracked();
            spawn_local(async move {
                if let Some(t) = auth_token {
                    if let Ok(res) =
                        api_get::<ApiItemResponse>(&format!("/admin/routes/{}", edit_key), Some(&t))
                            .await
                    {
                        if let Some(data) = res.data {
                            set_value.set(data.value);
                        }
                    }
                }
            });
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_is_loading.set(true);
        set_error_msg.set(None);
        let nav = navigate.clone();

        let req = RouteEntry {
            key: key.get(),
            value: value.get(),
        };
        let auth_token = auth.token.get_untracked();
        let original_key = params.with_untracked(|p| p.get("key").cloned());

        spawn_local(async move {
            if let Some(t) = auth_token {
                if let Some(old_key) = &original_key {
                    if old_key != &req.key {
                        let _ = api_delete::<ApiResponse>(
                            &format!("/admin/routes/{}", old_key),
                            Some(&t),
                        )
                        .await;
                    }
                }
                match api_post::<_, ApiResponse>("/admin/routes", Some(&t), &req).await {
                    Ok(_) => nav("/admin/ui", Default::default()),
                    Err(e) => set_error_msg.set(Some(e.to_string())),
                }
            }
            set_is_loading.set(false);
        });
    };

    view! {
        <div>
            <div style="margin-bottom: 24px;">
                <A href="/admin/ui" class="back-link">"← Back to Routes"</A>
            </div>
            <div class="card" style="max-width: 600px; margin: 0 auto;">
                <h2 style="margin-bottom: 24px;">{move || if is_edit.get() { "Edit Route" } else { "Create New Route" }}</h2>

                <form on:submit=on_submit>
                    <div>
                        <label style="display: block; margin-bottom: 8px;">"Route Key (e.g. webhook-test)"</label>
                        <input
                            type="text"
                            class="input"
                            prop:value=key
                            on:input=move |ev| set_key.set(event_target_value(&ev))
                            disabled=is_loading
                            required
                        />
                    </div>

                    <div>
                        <label style="display: block; margin-bottom: 8px;">"Target URL (e.g. https://backend.example.com)"</label>
                        <input
                            type="url"
                            class="input"
                            prop:value=value
                            on:input=move |ev| set_value.set(event_target_value(&ev))
                            disabled=is_loading
                            required
                        />
                    </div>

                    <div style="display:flex; gap:16px; margin-top: 24px;">
                        <button type="submit" class="btn" disabled=is_loading>
                            {move || if is_loading.get() { "Saving..." } else { "💾 Save Route" }}
                        </button>
                        <A href="/admin/ui" class="btn btn-secondary">"Cancel"</A>
                    </div>

                    {move || error_msg.get().map(|msg| view! {
                        <div style="margin-top: 16px; color: var(--danger);">
                            "⚠️ " {msg}
                        </div>
                    })}
                </form>
            </div>
        </div>
    }
}
