//! Settings page

use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::use_auth;
use crate::Route;

#[function_component(SettingsPage)]
pub fn settings_page() -> Html {
    let auth = use_auth();
    let navigator = use_navigator().unwrap();

    let on_logout = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            // Clear localStorage
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.remove_item("happy_token");
                    let _ = storage.remove_item("happy_user_id");
                    let _ = storage.remove_item("happy_user_email");
                }
            }
            // Redirect to login
            navigator.push(&Route::Login);
        })
    };

    html! {
        <div class="settings-page">
            <header class="settings-header">
                <h1>{ "Settings" }</h1>
                <button class="btn-back" onclick={Callback::from(move |_| navigator.push(&Route::Home))}>
                    { "← Back to Sessions" }
                </button>
            </header>

            <main class="settings-content">
                // Account Section
                <section class="settings-section">
                    <h2>{ "Account" }</h2>
                    <div class="account-info">
                        <div class="info-row">
                            <label>{ "User ID" }</label>
                            <span class="info-value">{ auth.user_id.as_deref().unwrap_or("Unknown") }</span>
                        </div>
                        <div class="info-row">
                            <label>{ "Email" }</label>
                            <span class="info-value">{ auth.user_email.as_deref().unwrap_or("Unknown") }</span>
                        </div>
                    </div>
                    <div class="settings-actions">
                        <button class="btn-danger" onclick={on_logout}>
                            { "Logout" }
                        </button>
                    </div>
                </section>

                // Notifications Section
                <section class="settings-section">
                    <h2>{ "Notifications" }</h2>
                    <label class="setting-item">
                        <input type="checkbox" />
                        <span>{ "Enable push notifications" }</span>
                    </label>
                    <label class="setting-item">
                        <input type="checkbox" checked={true} />
                        <span>{ "Notify when session needs confirmation" }</span>
                    </label>
                    <label class="setting-item">
                        <input type="checkbox" checked={true} />
                        <span>{ "Notify on session errors" }</span>
                    </label>
                </section>

                // Appearance Section
                <section class="settings-section">
                    <h2>{ "Appearance" }</h2>
                    <div class="setting-item">
                        <label>{ "Theme:" }</label>
                        <select class="theme-select">
                            <option value="dark">{ "Dark" }</option>
                            <option value="light">{ "Light" }</option>
                            <option value="system">{ "System" }</option>
                        </select>
                    </div>
                    <div class="setting-item">
                        <label>{ "Terminal Font Size:" }</label>
                        <input type="range" min="10" max="24" value="14" class="font-slider" />
                    </div>
                </section>

                // About Section
                <section class="settings-section">
                    <h2>{ "About" }</h2>
                    <div class="about-info">
                        <p>{ "Happy Remote v0.1.0" }</p>
                        <p>{ "A Rust-native remote Claude Code control system" }</p>
                        <p>
                            <a href="https://github.com/yourusername/happy-remote" target="_blank">
                                { "View on GitHub" }
                            </a>
                        </p>
                    </div>
                </section>
            </main>
        </div>
    }
}
