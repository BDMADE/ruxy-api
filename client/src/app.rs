use crate::auth::provide_auth_context;
use crate::components::layout::AdminLayout;
use crate::components::login::LoginPage;
use crate::components::route_form::RouteForm;
use crate::components::route_list::RouteList;
use leptos::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    provide_auth_context();

    view! {
        <Router>
            <Routes>
                <Route path="/admin/dashboard/login" view=LoginPage/>
                <Route path="/admin/dashboard" view=AdminLayout>
                    <Route path="" view=RouteList/>
                    <Route path="routes/new" view=RouteForm/>
                    <Route path="routes/:key/edit" view=RouteForm/>
                </Route>
            </Routes>
        </Router>
    }
}
