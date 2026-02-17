//! Protected Route Component
//!
//! Ensures only authenticated users can access certain routes

use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

/// Checks if user is authenticated by looking for token in localStorage
fn is_authenticated() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|storage| storage.get_item("happy_token").ok())
        .flatten()
        .is_some()
}

/// Get auth state from localStorage
fn get_auth_state() -> AuthState {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .map(|storage| {
            let token = storage.get_item("happy_token").ok().flatten();
            let user_id = storage.get_item("happy_user_id").ok().flatten();
            let user_email = storage.get_item("happy_user_email").ok().flatten();
            AuthState {
                is_authenticated: token.is_some(),
                token,
                user_id,
                user_email,
            }
        })
        .unwrap_or_else(|| AuthState {
            is_authenticated: false,
            token: None,
            user_id: None,
            user_email: None,
        })
}

/// Redirects to login page if not authenticated
#[function_component(ProtectedRoute)]
pub fn protected_route(props: &ProtectedRouteProps) -> Html {
    let authenticated = use_state(|| is_authenticated());

    {
        let authenticated = authenticated.clone();
        use_effect_with((), move |_| {
            authenticated.set(is_authenticated());
            || ()
        });
    }

    if *authenticated {
        props.children.clone()
    } else {
        html! {
            <Redirect<Route> to={Route::Login} />
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct ProtectedRouteProps {
    pub children: Html,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthState {
    pub is_authenticated: bool,
    pub token: Option<String>,
    pub user_id: Option<String>,
    pub user_email: Option<String>,
}

impl AuthState {
    /// Logout the user by clearing localStorage
    pub fn logout(&self) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.remove_item("happy_token");
                let _ = storage.remove_item("happy_user_id");
                let _ = storage.remove_item("happy_user_email");
            }
            // Redirect to login
            let _ = window.location().set_href("/login");
        }
    }
}

/// Hook to get current user info from localStorage
#[hook]
pub fn use_auth() -> AuthState {
    let auth_state = use_state(get_auth_state);

    {
        let auth_state = auth_state.clone();
        use_effect_with((), move |_| {
            auth_state.set(get_auth_state());
            || ()
        });
    }

    (*auth_state).clone()
}
