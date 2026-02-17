//! Global Dashboard - Monitor all sessions at a glance
//!
//! Provides 1-to-many session management with real-time status aggregation

use gloo_timers::callback::{Interval, Timeout};
use serde_json::json;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlSelectElement, WebSocket, Window};
use yew::prelude::*;
use yew_router::prelude::*;

/// Session status for dashboard
#[derive(Clone, Debug, PartialEq)]
pub struct SessionCard {
    pub id: String,
    pub tag: String,
    pub status: SessionState,
    pub progress: Option<u8>,
    pub operation: Option<String>,
    pub needs_confirmation: bool,
    pub confirmation_prompt: Option<String>,
    pub client_count: usize,
    pub last_activity: String,
    pub error_count: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionState {
    Initializing,
    Running,
    Idle,
    WaitingForConfirm,
    Error,
    Exited,
}

/// Global dashboard component
pub struct Dashboard {
    sessions: Vec<SessionCard>,
    ws: Option<WebSocket>,
    ws_status: ConnectionStatus,
    selected_sessions: Vec<String>,
    show_bulk_actions: bool,
    filter_text: String,
    sort_by: SortBy,
    heartbeat_interval: Option<Interval>,
    auth_token: Option<String>,
    user_email: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortBy {
    LastActivity,
    Status,
    Tag,
    Progress,
}

pub enum DashboardMsg {
    WsConnected,
    WsDisconnected,
    WsError(String),
    WsReconnect,
    WsMessage(String),
    SendPing,
    SessionsUpdate(Vec<SessionCard>),
    SessionUpdate(SessionCard),
    ToggleSelection(String),
    SelectAll,
    DeselectAll,
    BulkKill,
    BulkConfirm(bool),
    FilterChanged(String),
    SortChanged(SortBy),
    Refresh,
}

impl Dashboard {
    fn connect_websocket(&mut self, ctx: &Context<Self>) {
        let window = web_sys::window().unwrap();
        let location = window.location();
        let protocol = if location.protocol().unwrap() == "https:" {
            "wss"
        } else {
            "ws"
        };
        let host = location.host().unwrap();
        let ws_url = format!("{}://{}/ws", protocol, host);

        log::info!("Connecting to WebSocket: {}", ws_url);

        match WebSocket::new(&ws_url) {
            Ok(ws) => {
                ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

                // On open - send auth token
                let ws_clone = ws.clone();
                let link = ctx.link().clone();
                let onopen = Closure::wrap(Box::new(move || {
                    log::info!("WebSocket connected");

                    // Get token from localStorage
                    let window = web_sys::window().unwrap();
                    let storage = window.local_storage().unwrap().unwrap();
                    if let Ok(Some(token)) = storage.get_item("happy_token") {
                        let auth_msg = format!(r#"{{"type":"authenticate","token":"{}"}}"#, token);
                        let _ = ws_clone.send_with_str(&auth_msg);
                    }

                    link.send_message(DashboardMsg::WsConnected);
                }) as Box<dyn FnMut()>);
                ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
                onopen.forget();

                // On message
                let link = ctx.link().clone();
                let onmessage = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                    if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                        let text = String::from(text);
                        link.send_message(DashboardMsg::WsMessage(text));
                    }
                }) as Box<dyn FnMut(_)>);
                ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                onmessage.forget();

                // On error
                let link = ctx.link().clone();
                let onerror = Closure::wrap(Box::new(move |_e: web_sys::ErrorEvent| {
                    log::error!("WebSocket error");
                    link.send_message(DashboardMsg::WsError("Connection error".to_string()));
                }) as Box<dyn FnMut(_)>);
                ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                onerror.forget();

                // On close
                let link = ctx.link().clone();
                let onclose = Closure::wrap(Box::new(move || {
                    log::info!("WebSocket closed");
                    link.send_message(DashboardMsg::WsDisconnected);
                }) as Box<dyn FnMut()>);
                ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
                onclose.forget();

                self.ws = Some(ws);
            }
            Err(e) => {
                log::error!("Failed to create WebSocket: {:?}", e);
                self.ws_status = ConnectionStatus::Error;
            }
        }
    }

    fn send_message(&self, msg: &str) {
        if let Some(ws) = &self.ws {
            if ws.ready_state() == WebSocket::OPEN {
                if let Err(e) = ws.send_with_str(msg) {
                    log::error!("Failed to send message: {:?}", e);
                }
            }
        }
    }

    fn start_heartbeat(&mut self, ctx: &Context<Self>) {
        let link = ctx.link().clone();
        let interval = Interval::new(30_000, move || {
            link.send_message(DashboardMsg::SendPing);
        });
        self.heartbeat_interval = Some(interval);
    }

    fn stop_heartbeat(&mut self) {
        self.heartbeat_interval = None;
    }

    fn handle_ws_message(&mut self, text: String) {
        // Parse server message
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(msg) => {
                if let Some(msg_type) = msg.get("type").and_then(|v| v.as_str()) {
                    match msg_type {
                        "authenticated" => {
                            log::info!("Authenticated with server");
                            // Request session list
                            self.send_message(r#"{"type": "list_sessions"}"#);
                        }
                        "sessions_list" => {
                            // Parse sessions from response
                            if let Some(sessions) = msg.get("sessions").and_then(|v| v.as_array()) {
                                let session_cards: Vec<SessionCard> = sessions
                                    .iter()
                                    .filter_map(|s| self.parse_session_card(s))
                                    .collect();
                                self.sessions = session_cards;
                                self.sort_sessions();
                            }
                        }
                        "session_update" => {
                            // Update single session
                            if let Some(session) = self.parse_session_card(&msg) {
                                if let Some(idx) =
                                    self.sessions.iter().position(|s| s.id == session.id)
                                {
                                    self.sessions[idx] = session;
                                } else {
                                    self.sessions.push(session);
                                }
                                self.sort_sessions();
                            }
                        }
                        "session_created" | "session_started" => {
                            // Add new session
                            if let Some(session) = self.parse_session_card(&msg) {
                                if !self.sessions.iter().any(|s| s.id == session.id) {
                                    self.sessions.push(session);
                                    self.sort_sessions();
                                }
                            }
                        }
                        "session_terminated" | "session_stopped" => {
                            // Remove terminated session
                            if let Some(id) = msg.get("session_id").and_then(|v| v.as_str()) {
                                self.sessions.retain(|s| s.id != id);
                            }
                        }
                        "pong" => {
                            // Heartbeat response received
                        }
                        "error" => {
                            if let Some(code) = msg.get("code").and_then(|v| v.as_str()) {
                                if code == "auth_failed" || code == "not_authenticated" {
                                    log::warn!(
                                        "Authentication failed/expired, redirecting to login"
                                    );
                                    let window = web_sys::window().unwrap();
                                    let storage = window.local_storage().unwrap().unwrap();
                                    let _ = storage.remove_item("happy_token");
                                    let _ = storage.remove_item("happy_user_id");
                                    let _ = storage.remove_item("happy_user_email");
                                    let _ = window.location().set_href("/login");
                                    return;
                                }
                            }
                            if let Some(error_msg) = msg.get("message").and_then(|v| v.as_str()) {
                                log::error!("Server error: {}", error_msg);
                            }
                        }
                        _ => {
                            log::warn!("Unknown message type: {}", msg_type);
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to parse message: {} - error: {}", text, e);
            }
        }
    }

    fn parse_session_card(&self, value: &serde_json::Value) -> Option<SessionCard> {
        Some(SessionCard {
            id: value.get("id")?.as_str()?.to_string(),
            tag: value.get("tag")?.as_str()?.to_string(),
            status: self.parse_status(value.get("status")?.as_str()?),
            progress: value
                .get("progress")
                .and_then(|v| v.as_u64())
                .map(|v| v as u8),
            operation: value
                .get("operation")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            needs_confirmation: value
                .get("needs_confirmation")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            confirmation_prompt: value
                .get("confirmation_prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            client_count: value
                .get("client_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize,
            last_activity: value
                .get("last_activity")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            error_count: value
                .get("error_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            bytes_in: value.get("bytes_in").and_then(|v| v.as_u64()).unwrap_or(0),
            bytes_out: value.get("bytes_out").and_then(|v| v.as_u64()).unwrap_or(0),
        })
    }

    fn parse_status(&self, status: &str) -> SessionState {
        match status {
            "running" => SessionState::Running,
            "idle" => SessionState::Idle,
            "waiting_for_confirm" => SessionState::WaitingForConfirm,
            "error" => SessionState::Error,
            "exited" | "terminated" => SessionState::Exited,
            "initializing" => SessionState::Initializing,
            _ => SessionState::Initializing,
        }
    }
}

impl Component for Dashboard {
    type Message = DashboardMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        // Read auth info from localStorage
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let auth_token = storage.get_item("happy_token").unwrap_or(None);
        let user_email = storage.get_item("happy_user_email").unwrap_or(None);

        let mut dashboard = Self {
            sessions: Vec::new(),
            ws: None,
            ws_status: ConnectionStatus::Connecting,
            selected_sessions: Vec::new(),
            show_bulk_actions: false,
            filter_text: String::new(),
            sort_by: SortBy::LastActivity,
            heartbeat_interval: None,
            auth_token: auth_token.clone(),
            user_email,
        };

        // Only connect WebSocket if authenticated
        if auth_token.is_some() {
            dashboard.connect_websocket(ctx);
        } else {
            dashboard.ws_status = ConnectionStatus::Disconnected;
        }

        dashboard
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            DashboardMsg::WsConnected => {
                self.ws_status = ConnectionStatus::Connected;
                // Send authentication with actual token
                if let Some(token) = &self.auth_token {
                    let auth_msg = json!({
                        "type": "authenticate",
                        "token": token
                    })
                    .to_string();
                    self.send_message(&auth_msg);
                    // Request sessions list
                    self.send_message(r#"{"type": "list_sessions"}"#);
                }
                // Start heartbeat
                self.start_heartbeat(ctx);
                true
            }
            DashboardMsg::WsDisconnected => {
                self.ws_status = ConnectionStatus::Disconnected;
                self.stop_heartbeat();
                // Attempt to reconnect after 3 seconds
                let link = ctx.link().clone();
                Timeout::new(3_000, move || {
                    link.send_message(DashboardMsg::WsReconnect);
                })
                .forget();
                true
            }
            DashboardMsg::WsError(err) => {
                log::error!("WebSocket error: {}", err);
                self.ws_status = ConnectionStatus::Error;
                self.stop_heartbeat();
                // Attempt reconnect
                self.ws = None;
                let link = ctx.link().clone();
                Timeout::new(3_000, move || {
                    link.send_message(DashboardMsg::WsReconnect);
                })
                .forget();
                true
            }
            DashboardMsg::WsReconnect => {
                log::info!("Attempting to reconnect WebSocket...");
                self.ws = None;
                self.connect_websocket(ctx);
                true
            }
            DashboardMsg::WsMessage(text) => {
                self.handle_ws_message(text);
                true
            }
            DashboardMsg::SendPing => {
                self.send_message(r#"{"type": "ping"}"#);
                false
            }
            DashboardMsg::SessionsUpdate(sessions) => {
                self.sessions = sessions;
                self.sort_sessions();
                true
            }
            DashboardMsg::SessionUpdate(session) => {
                if let Some(idx) = self.sessions.iter().position(|s| s.id == session.id) {
                    self.sessions[idx] = session;
                } else {
                    self.sessions.push(session);
                }
                self.sort_sessions();
                true
            }
            DashboardMsg::ToggleSelection(id) => {
                if let Some(pos) = self.selected_sessions.iter().position(|x| x == &id) {
                    self.selected_sessions.remove(pos);
                } else {
                    self.selected_sessions.push(id);
                }
                self.show_bulk_actions = !self.selected_sessions.is_empty();
                true
            }
            DashboardMsg::SelectAll => {
                self.selected_sessions = self
                    .filtered_sessions()
                    .iter()
                    .map(|s| s.id.clone())
                    .collect();
                self.show_bulk_actions = !self.selected_sessions.is_empty();
                true
            }
            DashboardMsg::DeselectAll => {
                self.selected_sessions.clear();
                self.show_bulk_actions = false;
                true
            }
            DashboardMsg::BulkKill => {
                for id in &self.selected_sessions {
                    let msg = format!(r#"{{"type": "stop_session", "session_id": "{}"}}"#, id);
                    self.send_message(&msg);
                }
                self.selected_sessions.clear();
                self.show_bulk_actions = false;
                true
            }
            DashboardMsg::BulkConfirm(confirm) => {
                for id in &self.selected_sessions {
                    let action = if confirm { "confirm" } else { "cancel" };
                    let msg = format!(
                        r#"{{"type": "session_action", "session_id": "{}", "action": "{}"}}"#,
                        id, action
                    );
                    self.send_message(&msg);
                }
                self.selected_sessions.clear();
                self.show_bulk_actions = false;
                true
            }
            DashboardMsg::FilterChanged(text) => {
                self.filter_text = text;
                true
            }
            DashboardMsg::SortChanged(sort) => {
                self.sort_by = sort;
                self.sort_sessions();
                true
            }
            DashboardMsg::Refresh => {
                self.send_message(r#"{"type": "list_sessions"}"#);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let filtered = self.filtered_sessions();
        let needs_attention: Vec<_> = filtered
            .iter()
            .filter(|s| s.needs_confirmation || matches!(s.status, SessionState::Error))
            .collect();

        html! {
            <div class="dashboard">
                <header class="dashboard-header">
                    <h1>{ "Happy Remote Dashboard" }</h1>
                    <div class="header-actions">
                        {
                            if let Some(email) = &self.user_email {
                                html! {
                                    <>
                                        <span class="user-email">{ email }</span>
                                        <button
                                            class="btn-logout"
                                            onclick={Callback::from(|_| {
                                                let window = web_sys::window().unwrap();
                                                let storage = window.local_storage().unwrap().unwrap();
                                                let _ = storage.remove_item("happy_token");
                                                let _ = storage.remove_item("happy_user_id");
                                                let _ = storage.remove_item("happy_user_email");
                                                let _ = window.location().set_href("/login");
                                            })}
                                        >
                                            { "Logout" }
                                        </button>
                                    </>
                                }
                            } else {
                                html! {
                                    <button
                                        class="btn-login"
                                        onclick={Callback::from(|_| {
                                            let window: Window = web_sys::window().unwrap();
                                            let _ = window.location().set_href("/login");
                                        })}
                                    >
                                        { "Login" }
                                    </button>
                                }
                            }
                        }
                        <div class="connection-status">
                            { self.view_connection_status() }
                        </div>
                    </div>
                </header>

                // Summary Cards
                <div class="summary-cards">
                    <div class="summary-card total">
                        <div class="number">{ filtered.len() }</div>
                        <div class="label">{ "Active Sessions" }</div>
                    </div>
                    <div class="summary-card running">
                        <div class="number">
                            { filtered.iter().filter(|s| matches!(s.status, SessionState::Running)).count() }
                        </div>
                        <div class="label">{ "Running" }</div>
                    </div>
                    <div class="summary-card waiting">
                        <div class="number">
                            { filtered.iter().filter(|s| s.needs_confirmation).count() }
                        </div>
                        <div class="label">{ "Needs Confirm" }</div>
                    </div>
                    <div class="summary-card error">
                        <div class="number">
                            { filtered.iter().filter(|s| matches!(s.status, SessionState::Error)).count() }
                        </div>
                        <div class="label">{ "Errors" }</div>
                    </div>
                </div>

                // Attention Alert Banner
                {
                    if !needs_attention.is_empty() {
                        html! {
                            <div class="attention-banner">
                                <span class="icon">{ "⚠️" }</span>
                                <span class="message">
                                    { format!("{} session(s) need your attention", needs_attention.len()) }
                                </span>
                                <button class="btn-primary" onclick={link.callback(|_| DashboardMsg::Refresh)}>
                                    { "View All" }
                                </button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Toolbar
                <div class="toolbar">
                    <div class="filter">
                        <input
                            type="text"
                            placeholder="Filter sessions..."
                            value={self.filter_text.clone()}
                            oninput={link.callback(|e: InputEvent| {
                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                DashboardMsg::FilterChanged(input.value())
                            })}
                        />
                    </div>
                    <div class="sort">
                        <select onchange={link.callback(|e: Event| {
                            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                            match select.value().as_str() {
                                "status" => DashboardMsg::SortChanged(SortBy::Status),
                                "tag" => DashboardMsg::SortChanged(SortBy::Tag),
                                "progress" => DashboardMsg::SortChanged(SortBy::Progress),
                                _ => DashboardMsg::SortChanged(SortBy::LastActivity),
                            }
                        })}>
                            <option value="activity" selected={self.sort_by == SortBy::LastActivity}>
                                { "Sort: Last Activity" }
                            </option>
                            <option value="status" selected={self.sort_by == SortBy::Status}>
                                { "Sort: Status" }
                            </option>
                            <option value="tag" selected={self.sort_by == SortBy::Tag}>
                                { "Sort: Tag" }
                            </option>
                            <option value="progress" selected={self.sort_by == SortBy::Progress}>
                                { "Sort: Progress" }
                            </option>
                        </select>
                    </div>
                    <div class="actions">
                        <button onclick={link.callback(|_| DashboardMsg::SelectAll)}>
                            { "Select All" }
                        </button>
                        <button onclick={link.callback(|_| DashboardMsg::DeselectAll)}>
                            { "Deselect All" }
                        </button>
                        <button class="btn-primary" onclick={link.callback(|_| DashboardMsg::Refresh)}>
                            { "Refresh" }
                        </button>
                    </div>
                </div>

                // Bulk Actions Bar
                {
                    if self.show_bulk_actions {
                        html! {
                            <div class="bulk-actions-bar">
                                <span>{ format!("{} selected", self.selected_sessions.len()) }</span>
                                <button onclick={link.callback(|_| DashboardMsg::BulkConfirm(true))}>
                                    { "Confirm All" }
                                </button>
                                <button onclick={link.callback(|_| DashboardMsg::BulkConfirm(false))}>
                                    { "Cancel All" }
                                </button>
                                <button class="btn-danger" onclick={link.callback(|_| DashboardMsg::BulkKill)}>
                                    { "Kill Selected" }
                                </button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Sessions Grid
                <div class="sessions-grid">
                    { for filtered.iter().map(|session| self.render_session_card(session, link)) }
                </div>

                // Empty State
                {
                    if filtered.is_empty() {
                        let (icon, title, desc) = if self.user_email.is_none() {
                            ("🔒", "Not logged in", "Please login to view your sessions")
                        } else if self.filter_text.is_empty() {
                            ("📭", "No sessions found", "Start a new session from the CLI: happy run claude")
                        } else {
                            ("🔍", "No matches", "No sessions match your filter")
                        };
                        html! {
                            <div class="empty-state">
                                <div class="icon">{ icon }</div>
                                <h3>{ title }</h3>
                                <p>{ desc }</p>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        }
    }
}

impl Dashboard {
    fn view_connection_status(&self) -> Html {
        match self.ws_status {
            ConnectionStatus::Connected => {
                html! { <span class="status connected">{ "● Connected" }</span> }
            }
            ConnectionStatus::Connecting => {
                html! { <span class="status connecting">{ "○ Connecting..." }</span> }
            }
            ConnectionStatus::Disconnected => {
                html! { <span class="status disconnected">{ "○ Disconnected" }</span> }
            }
            ConnectionStatus::Error => {
                html! { <span class="status error">{ "● Error" }</span> }
            }
        }
    }

    fn filtered_sessions(&self) -> Vec<&SessionCard> {
        self.sessions
            .iter()
            .filter(|s| {
                // Filter out exited sessions unless searching for them specifically?
                // For now, just hide them to declutter.
                !matches!(s.status, SessionState::Exited)
            })
            .filter(|s| {
                self.filter_text.is_empty()
                    || s.tag
                        .to_lowercase()
                        .contains(&self.filter_text.to_lowercase())
                    || s.id
                        .to_lowercase()
                        .contains(&self.filter_text.to_lowercase())
            })
            .collect()
    }

    fn sort_sessions(&mut self) {
        match self.sort_by {
            SortBy::LastActivity => {
                // Already sorted by last activity by default
            }
            SortBy::Status => {
                self.sessions.sort_by_key(|s| match s.status {
                    SessionState::Error => 0,
                    SessionState::WaitingForConfirm => 1,
                    SessionState::Running => 2,
                    SessionState::Idle => 3,
                    SessionState::Initializing => 4,
                    SessionState::Exited => 5,
                });
            }
            SortBy::Tag => {
                self.sessions.sort_by(|a, b| a.tag.cmp(&b.tag));
            }
            SortBy::Progress => {
                self.sessions.sort_by_key(|s| s.progress.unwrap_or(0));
                self.sessions.reverse();
            }
        }
    }

    fn render_session_card(&self, session: &SessionCard, link: &yew::html::Scope<Self>) -> Html {
        let is_selected = self.selected_sessions.contains(&session.id);
        let status_class = match session.status {
            SessionState::Running => "running",
            SessionState::Idle => "idle",
            SessionState::WaitingForConfirm => "waiting",
            SessionState::Error => "error",
            SessionState::Exited => "exited",
            SessionState::Initializing => "initializing",
        };

        // Format timestamp to local time
        let last_activity_display = {
            let date = js_sys::Date::new(&JsValue::from_str(&session.last_activity));
            if date.get_time().is_nan() {
                session.last_activity.clone()
            } else {
                String::from(date.to_string())
            }
        };

        html! {
            <div class={classes!("session-card", status_class, is_selected.then_some("selected"))}>
                <div class="card-header">
                    <input
                        type="checkbox"
                        checked={is_selected}
                        onclick={link.callback({
                            let id = session.id.clone();
                            move |_| DashboardMsg::ToggleSelection(id.clone())
                        })}
                    />
                    <span class="tag">{ &session.tag }</span>
                    <span class={classes!("status-badge", status_class)}>
                        { self.format_status(&session.status) }
                    </span>
                </div>

                <div class="card-body">
                    // Progress bar
                    {
                        if let Some(progress) = session.progress {
                            html! {
                                <div class="progress-section">
                                    <div class="progress-bar">
                                        <div class="progress-fill" style={format!("width: {}%", progress)}></div>
                                    </div>
                                    <span class="progress-text">{ format!("{}%", progress) }</span>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }

                    // Current operation
                    {
                        if let Some(op) = &session.operation {
                            html! {
                                <div class="operation">{ op }</div>
                            }
                        } else {
                            html! {}
                        }
                    }

                    // Confirmation prompt
                    {
                        if session.needs_confirmation {
                            html! {
                                <div class="confirmation-box">
                                    <div class="prompt">{ session.confirmation_prompt.as_ref().unwrap_or(&"Action required".to_string()) }</div>
                                    <div class="actions">
                                        <button class="btn-confirm">{ "Confirm" }</button>
                                        <button class="btn-cancel">{ "Cancel" }</button>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                <div class="card-footer">
                    <span class="clients">{ format!("👥 {}", session.client_count) }</span>
                    <span class="activity">{ last_activity_display }</span>
                    <a href={format!("/#{}", session.tag)} class="btn-connect">
                        { "Connect" }
                    </a>
                </div>
            </div>
        }
    }

    fn format_status(&self, status: &SessionState) -> String {
        match status {
            SessionState::Running => "Running".to_string(),
            SessionState::Idle => "Idle".to_string(),
            SessionState::WaitingForConfirm => "Needs Confirm".to_string(),
            SessionState::Error => "Error".to_string(),
            SessionState::Exited => "Exited".to_string(),
            SessionState::Initializing => "Initializing".to_string(),
        }
    }
}
