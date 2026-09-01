use crate::api::{api_delete, api_get};
use crate::auth::use_auth;
use crate::models::{ApiListResponse, ApiResponse};
use crate::utils::{copy_to_clipboard, get_base_url};
use leptos::*;
use leptos_router::*;

#[component]
pub fn RouteList() -> impl IntoView {
    let auth = use_auth();
    let (search, set_search) = create_signal(String::new());
    let (page_size, set_page_size) = create_signal(25);
    let (current_page, set_current_page) = create_signal(1);
    let (reload, set_reload) = create_signal(0);
    let (copied_key, set_copied_key) = create_signal(Option::<String>::None);

    let base_url = get_base_url();

    let routes = create_local_resource(
        move || (auth.token.get(), reload.get()),
        move |(token, _)| async move {
            if let Some(t) = token {
                api_get::<ApiListResponse>("/admin/api/routes", Some(&t))
                    .await
                    .map(|r| r.data)
            } else {
                Err(crate::api::ApiError("No token".into()))
            }
        },
    );

    let filtered_routes = create_memo(move |_| {
        routes
            .get()
            .and_then(|r| r.ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|route| {
                let s = search.get().to_lowercase();
                if s.is_empty() {
                    return true;
                }
                route.key.to_lowercase().contains(&s) || route.value.to_lowercase().contains(&s)
            })
            .collect::<Vec<_>>()
    });

    let total_pages = create_memo(move |_| {
        let total = filtered_routes.get().len();
        let size = page_size.get();
        (total as f64 / size as f64).ceil() as usize
    });

    let paginated_routes = create_memo(move |_| {
        let all = filtered_routes.get();
        let size = page_size.get();
        let page = current_page.get().max(1);
        let start = (page - 1) * size;
        all.into_iter().skip(start).take(size).collect::<Vec<_>>()
    });

    let delete_route = move |key: String| {
        if window()
            .confirm_with_message(&format!("Are you sure you want to delete '{}'?", key))
            .unwrap_or(false)
        {
            let auth_token = auth.token.get_untracked();
            spawn_local(async move {
                if let Some(t) = auth_token {
                    match api_delete::<ApiResponse>(&format!("/admin/api/routes/{}", key), Some(&t))
                        .await
                    {
                        Ok(_) => set_reload.update(|n| *n += 1),
                        Err(e) => window()
                            .alert_with_message(&format!("Failed to delete: {}", e))
                            .unwrap_or(()),
                    }
                }
            });
        }
    };

    let copy_url = move |key: String, full_url: String| {
        copy_to_clipboard(&full_url);
        set_copied_key.set(Some(key.clone()));
        set_timeout(
            move || {
                set_copied_key.update(|current| {
                    if let Some(k) = current {
                        if k == &key {
                            *current = None;
                        }
                    }
                });
            },
            std::time::Duration::from_millis(2000),
        );
    };

    view! {
        <div>
            <div style="display:flex; justify-content:space-between; margin-bottom: 24px;">
                <h2>"Routes"</h2>
                <A href="/admin/dashboard/routes/new" class="btn">"+ Add Route"</A>
            </div>

            <div class="card">
                <input
                    type="text"
                    class="input"
                    placeholder="Search by key or target..."
                    prop:value=search
                    on:input=move |ev| {
                        set_search.set(event_target_value(&ev));
                        set_current_page.set(1);
                    }
                />

                <Suspense fallback=move || view! { <div>"Loading routes..."</div> }>
                    <table style="margin-bottom: 24px;">
                        <thead>
                            <tr>
                                <th>"Key"</th>
                                <th>"Pseudo URL"</th>
                                <th>"Target URL"</th>
                                <th>"Actions"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {
                                let base = base_url.clone();
                                move || paginated_routes.get().into_iter().map(|route| {
                                    let key_clone = route.key.clone();
                                    let key_for_delete = route.key.clone();
                                    let key_for_copy = route.key.clone();
                                    let pseudo_url = format!("{}/{}", base, route.key);
                                    let pseudo_url_for_copy = pseudo_url.clone();
                                    let current_key_for_class = route.key.clone();
                                    let current_key_for_text = route.key.clone();

                                    view! {
                                        <tr>
                                            <td>{route.key}</td>
                                            <td>
                                                <div class="pseudo-url-container">
                                                    <span class="pseudo-url-code">{pseudo_url}</span>
                                                    <button
                                                        type="button"
                                                        class=move || {
                                                            if copied_key.get().as_deref() == Some(&current_key_for_class) {
                                                                "btn-icon copied"
                                                            } else {
                                                                "btn-icon"
                                                            }
                                                        }
                                                        title="Copy Pseudo URL"
                                                        on:click=move |_| copy_url(key_for_copy.clone(), pseudo_url_for_copy.clone())
                                                    >
                                                        {move || {
                                                            if copied_key.get().as_deref() == Some(&current_key_for_text) {
                                                                "✓ Copied"
                                                            } else {
                                                                "📋 Copy"
                                                            }
                                                        }}
                                                    </button>
                                                </div>
                                            </td>
                                            <td>{route.value}</td>
                                            <td>
                                                <span style="margin-right: 8px;">
                                                    <A href=format!("/admin/dashboard/routes/{}/edit", key_clone)>"✏️"</A>
                                                </span>
                                                <button style="background:none; border:none; cursor:pointer;" on:click=move |_| delete_route(key_for_delete.clone())>"🗑️"</button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()
                            }
                        </tbody>
                    </table>

                    <div style="display:flex; justify-content:space-between; align-items:center;">
                        <div>
                            "Showing " {move || filtered_routes.get().len()} " items"
                        </div>
                        <div style="display:flex; gap:16px; align-items:center;">
                            <label>"Page Size:"</label>
                            <select
                                style="background:var(--bg-card); color:var(--text-primary); border:1px solid var(--border); padding:4px;"
                                on:change=move |ev| {
                                    if let Ok(size) = event_target_value(&ev).parse::<usize>() {
                                        set_page_size.set(size);
                                        set_current_page.set(1);
                                    }
                                }
                            >
                                <option value="25" selected=move || page_size.get() == 25>"25"</option>
                                <option value="50" selected=move || page_size.get() == 50>"50"</option>
                                <option value="100" selected=move || page_size.get() == 100>"100"</option>
                            </select>

                            <div style="display:flex; gap:8px;">
                                <button
                                    class="btn"
                                    disabled=move || { current_page.get() <= 1 }
                                    on:click=move |_| set_current_page.update(|p| *p -= 1)
                                >"Previous"</button>
                                <span style="padding: 10px;">"Page " {current_page} " of " {total_pages}</span>
                                <button
                                    class="btn"
                                    disabled=move || { current_page.get() >= total_pages.get() }
                                    on:click=move |_| set_current_page.update(|p| *p += 1)
                                >"Next"</button>
                            </div>
                        </div>
                    </div>
                </Suspense>
            </div>
        </div>
    }
}
