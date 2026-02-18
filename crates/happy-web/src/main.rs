//! Happy Remote Web Frontend
//!
//! A Yew-based web application for controlling Claude Code remotely.

use yew::prelude::*;
use yew_router::prelude::*;

mod components;
mod pages;
mod utils;

use components::ProtectedRoute;
use pages::{Dashboard, TerminalPage, LoginPage, SettingsPage};
use utils::logger::init_console_capture;

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/dashboard")]
    Dashboard,
    #[at("/settings")]
    Settings,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! {
            <ProtectedRoute>
                <TerminalPage />
            </ProtectedRoute>
        },
        Route::Login => html! { <LoginPage /> },
        Route::Dashboard => html! {
            <ProtectedRoute>
                <Dashboard />
            </ProtectedRoute>
        },
        Route::Settings => html! {
            <ProtectedRoute>
                <SettingsPage />
            </ProtectedRoute>
        },
        Route::NotFound => html! { <h1>{ "404 - Not Found" }</h1> },
    }
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <>
            <BrowserRouter>
                <Switch<Route> render={switch} />
            </BrowserRouter>
        </>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    // Initialize console log capture
    init_console_capture();
    yew::Renderer::<App>::new().render();
}
