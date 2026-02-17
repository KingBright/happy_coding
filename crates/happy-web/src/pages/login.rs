//! Login/Register Page
//!
//! User authentication interface

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, Window, XmlHttpRequest, ProgressEvent};
use yew::prelude::*;

pub enum LoginMsg {
    EmailChanged(String),
    PasswordChanged(String),
    NameChanged(String),
    Submit,
    ToggleMode,
    LoginSuccess { token: String, user: UserInfo },
    Error(String),
}

#[derive(Clone)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

pub struct LoginPage {
    email: String,
    password: String,
    name: String,
    is_register: bool,
    loading: bool,
    error: Option<String>,
    redirect_url: Option<String>,
}

impl Component for LoginPage {
    type Message = LoginMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        // Parse redirect URL from query parameters
        let redirect_url = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .and_then(|search| {
                // Parse ?redirect=xxx
                search.strip_prefix('?')
                    .and_then(|params| {
                        for param in params.split('&') {
                            if let Some(eq_pos) = param.find('=') {
                                let key = &param[..eq_pos];
                                let value = &param[eq_pos + 1..];
                                if key == "redirect" {
                                    return js_sys::decode_uri_component(value).ok()
                                        .map(|s| String::from(s));
                                }
                            }
                        }
                        None
                    })
            });

        Self {
            email: String::new(),
            password: String::new(),
            name: String::new(),
            is_register: false,
            loading: false,
            error: None,
            redirect_url,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LoginMsg::EmailChanged(email) => {
                self.email = email;
                true
            }
            LoginMsg::PasswordChanged(password) => {
                self.password = password;
                true
            }
            LoginMsg::NameChanged(name) => {
                self.name = name;
                true
            }
            LoginMsg::ToggleMode => {
                self.is_register = !self.is_register;
                self.error = None;
                true
            }
            LoginMsg::Submit => {
                self.loading = true;
                self.error = None;

                let email = self.email.clone();
                let password = self.password.clone();
                let name = self.name.clone();
                let is_register = self.is_register;

                ctx.link().send_future(async move {
                    match do_auth(&email, &password, &name, is_register).await {
                        Ok((token, user)) => LoginMsg::LoginSuccess { token, user },
                        Err(e) => LoginMsg::Error(e),
                    }
                });

                true
            }
            LoginMsg::LoginSuccess { token, user } => {
                self.loading = false;

                // Store token in localStorage
                let window = web_sys::window().unwrap();
                let storage = window.local_storage().unwrap().unwrap();
                let _ = storage.set_item("happy_token", &token);
                let _ = storage.set_item("happy_user_id", &user.id);
                let _ = storage.set_item("happy_user_email", &user.email);

                log::info!("Login successful for user: {}", user.email);

                // Redirect to original URL or dashboard
                let window: Window = web_sys::window().unwrap();
                let redirect = self.redirect_url.as_deref().unwrap_or("/");
                let _ = window.location().set_href(redirect);

                true
            }
            LoginMsg::Error(e) => {
                self.loading = false;
                self.error = Some(e);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_email_change = ctx.link().callback(|e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            LoginMsg::EmailChanged(input.value())
        });

        let on_password_change = ctx.link().callback(|e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            LoginMsg::PasswordChanged(input.value())
        });

        let on_name_change = ctx.link().callback(|e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            LoginMsg::NameChanged(input.value())
        });

        let on_submit = ctx.link().callback(|e: SubmitEvent| {
            e.prevent_default();
            LoginMsg::Submit
        });

        let on_toggle = ctx.link().callback(|_| LoginMsg::ToggleMode);

        let title = if self.is_register {
            "Create Account"
        } else {
            "Sign In"
        };

        let toggle_text = if self.is_register {
            "Already have an account? Sign In"
        } else {
            "Don't have an account? Create one"
        };

        let button_text = if self.loading {
            "Please wait..."
        } else if self.is_register {
            "Create Account"
        } else {
            "Sign In"
        };

        html! {
            <div class="login-container">
                <div class="login-box">
                    <h1>{ "Happy Remote" }</h1>
                    <h2>{ title }</h2>

                    if let Some(ref error) = self.error {
                        <div class="error-message">{ error }</div>
                    }

                    <form onsubmit={on_submit}>
                        if self.is_register {
                            <div class="form-group">
                                <label>{ "Name" }</label>
                                <input
                                    type="text"
                                    placeholder="Your name"
                                    value={self.name.clone()}
                                    onchange={on_name_change}
                                    disabled={self.loading}
                                />
                            </div>
                        }

                        <div class="form-group">
                            <label>{ "Email" }</label>
                            <input
                                type="email"
                                placeholder="email@example.com"
                                value={self.email.clone()}
                                onchange={on_email_change}
                                disabled={self.loading}
                                required={true}
                            />
                        </div>

                        <div class="form-group">
                            <label>{ "Password" }</label>
                            <input
                                type="password"
                                placeholder="Enter password (min 6 chars)"
                                value={self.password.clone()}
                                onchange={on_password_change}
                                disabled={self.loading}
                                required={true}
                                minlength={"6"}
                            />
                        </div>

                        <button
                            type="submit"
                            class="btn-primary"
                            disabled={self.loading}
                        >
                            { button_text }
                        </button>
                    </form>

                    <div class="login-footer">
                        <button class="btn-link" onclick={on_toggle}>
                            { toggle_text }
                        </button>
                    </div>
                </div>
            </div>
        }
    }
}

async fn do_auth(
    email: &str,
    password: &str,
    name: &str,
    is_register: bool,
) -> Result<(String, UserInfo), String> {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let protocol = location.protocol().unwrap();
    let host = location.host().unwrap();
    let base_url = format!("{}//{}", protocol, host);

    let endpoint = if is_register {
        "/api/v1/auth/register"
    } else {
        "/api/v1/auth/login"
    };

    let url = format!("{}{}", base_url, endpoint);

    let body = if is_register {
        format!(
            r#"{{"email":"{}","password":"{}","name":"{}"}}"#,
            email, password, name
        )
    } else {
        format!(r#"{{"email":"{}","password":"{}"}}"#, email, password)
    };

    let request = XmlHttpRequest::new().map_err(|e| format!("XHR error: {:?}", e))?;

    request
        .open("POST", &url)
        .map_err(|e| format!("Open error: {:?}", e))?;
    request
        .set_request_header("Content-Type", "application/json")
        .map_err(|e| format!("Header error: {:?}", e))?;

    let (sender, receiver) = futures::channel::oneshot::channel();
    let mut sender = Some(sender);

    let onload = Closure::once_into_js(move |e: ProgressEvent| {
        let xhr: XmlHttpRequest = e.target().unwrap().dyn_into().unwrap();
        let sender = sender.take().unwrap();
        let _ = sender.send(xhr);
    });

    request.set_onload(Some(onload.as_ref().unchecked_ref()));

    request
        .send_with_opt_str(Some(&body))
        .map_err(|e| format!("Send error: {:?}", e))?;

    let xhr: XmlHttpRequest = receiver
        .await
        .map_err(|e| format!("Response error: {:?}", e))?;

    let status = xhr.status().map_err(|e| format!("Status error: {:?}", e))?;

    if status != 200 {
        let text = xhr
            .response_text()
            .map_err(|e| format!("Text error: {:?}", e))?
            .unwrap_or_default();
        return Err(format!("Authentication failed: {}", text));
    }

    let response_text = xhr
        .response_text()
        .map_err(|e| format!("Text error: {:?}", e))?
        .unwrap_or_default();

    // Parse JSON response
    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| format!("JSON parse error: {}", e))?;

    let token = json["access_token"]
        .as_str()
        .ok_or("No access token in response")?
        .to_string();

    let user = UserInfo {
        id: json["user"]["id"]
            .as_str()
            .ok_or("No user id")?
            .to_string(),
        email: json["user"]["email"]
            .as_str()
            .ok_or("No user email")?
            .to_string(),
        name: json["user"]["name"].as_str().map(|s| s.to_string()),
    };

    Ok((token, user))
}
