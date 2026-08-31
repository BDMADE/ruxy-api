use crate::api;
use crate::models::{LoginRequest, LoginResponse};
use gloo_storage::{LocalStorage, Storage};
use leptos::*;

const TOKEN_KEY: &str = "ruxy_admin_token";

#[derive(Clone)]
pub struct AuthContext {
    pub token: RwSignal<Option<String>>,
    pub is_authenticated: Memo<bool>,
}

pub fn provide_auth_context() {
    let initial_token = LocalStorage::get::<String>(TOKEN_KEY).ok();
    let token = create_rw_signal(initial_token);

    let is_authenticated = create_memo(move |_| token.get().is_some());

    provide_context(AuthContext {
        token,
        is_authenticated,
    });
}

pub fn use_auth() -> AuthContext {
    use_context::<AuthContext>().expect("AuthContext not provided")
}

pub async fn login(password: String) -> Result<bool, String> {
    let auth = use_auth();
    let req = LoginRequest { password };

    match api::api_post::<_, LoginResponse>("/admin/login", None, &req).await {
        Ok(res) => {
            if res.success {
                if let Some(t) = res.token {
                    let _ = LocalStorage::set(TOKEN_KEY, &t);
                    auth.token.set(Some(t));
                    return Ok(true);
                }
            }
            Err(res.message)
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn logout() {
    let auth = use_auth();
    LocalStorage::delete(TOKEN_KEY);
    auth.token.set(None);
}
