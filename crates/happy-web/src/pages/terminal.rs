//! Terminal Page - Main page with session list and terminal display
//!
//! Responsive layout:
//! - Large screen: Session list (left) + Terminal (right) side by side
//! - Small screen: Toggle between session list and terminal
//!
//! Features:
//! - Sessions grouped by machine, then by folder
//! - Right-click context menu for delete with confirmation
//! - "+" button to create new remote session

use gloo_timers::callback::Interval;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{
    Event, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement, InputEvent, MessageEvent, MouseEvent, SubmitEvent,
    WebSocket,
};
use yew::prelude::*;

use crate::components::{XTerm, XTermProps};

#[derive(Clone, PartialEq)]
pub struct SessionSummary {
    pub id: String,
    pub tag: String,
    pub status: String,
    pub cwd: String,
    pub machine_id: String,
    pub machine_name: String,
    pub is_online: bool,
}

impl SessionSummary {
    /// Get the last folder name from cwd for display
    pub fn folder_name(&self) -> &str {
        if self.cwd.is_empty() || self.cwd == "/" {
            return "root";
        }
        self.cwd
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&self.cwd)
    }
}

#[derive(Clone, PartialEq)]
pub struct MachineInfo {
    pub id: String,
    pub name: String,
}

/// Mobile view state for responsive UI
#[derive(Clone, PartialEq)]
pub enum MobileView {
    SessionList,
    Terminal,
}

#[derive(Properties, PartialEq)]
pub struct TerminalPageProps {}

#[function_component(TerminalPage)]
pub fn terminal_page(_props: &TerminalPageProps) -> Html {
    // Read target_tag from URL fragment (#tag)
    let window = web_sys::window().unwrap();
    let location = window.location();
    let hash = location.hash().unwrap_or_default();
    let target_tag = if hash.len() > 1 {
        Some(hash[1..].to_string()) // Remove the leading #
    } else {
        None
    };

    let ws_status = use_state(|| "Connecting...".to_string());
    // Use use_mut_ref for sessions and machines to allow updates from WebSocket callbacks
    let sessions = use_mut_ref(|| Vec::<SessionSummary>::new());
    let machines = use_mut_ref(|| Vec::<MachineInfo>::new());
    let sessions_version = use_state(|| 0u32); // Used to trigger re-render
    let selected_session_id = use_state(|| None::<String>);
    // selected_session_id_ref removed - using selected_session_id directly
    let input_value = use_state(|| String::new());
    let ws_ref = use_mut_ref(|| None::<WebSocket>);
    let joined_tags_ref = use_mut_ref(HashSet::<String>::new);
    let mobile_view = use_state(|| {
        if target_tag.is_some() {
            MobileView::Terminal
        } else {
            MobileView::SessionList
        }
    });

    // Delete confirmation state
    let delete_confirm = use_state(|| None::<(String, String)>); // (session_id, tag)

    // Create session modal state
    let show_create_modal = use_state(|| false);
    let create_cwd = use_state(|| String::new());
    let create_machine = use_state(|| String::new());
    let create_args = use_state(|| String::new());

    // Context menu state
    let context_menu = use_state(|| None::<(i32, i32, String, String)>); // (x, y, session_id, tag)

    // Use Rc<RefCell<>> for terminal buffers
    let terminal_buffers = use_mut_ref(|| HashMap::<String, String>::new());
    let buffer_version = use_state(|| 0u32);

    // Track loading state - true until we receive first sessions_list
    let sessions_loaded = use_state(|| false);

    // Virtual keyboard key sender callback
    let key_sender = use_state(|| None::<Callback<String>>);

    // Terminal direct writer callback - for incremental updates without re-rendering
    // Use use_mut_ref to allow updates from WebSocket callbacks
    let terminal_writer = use_mut_ref(|| None::<Callback<Vec<u8>>>);

    // Terminal scroll-to-bottom callback
    let scroll_to_bottom = use_state(|| None::<Callback<()>>);
    let show_scroll_to_bottom = use_state(|| false);

    // Git status state
    let git_status = use_state(|| None::<GitStatusInfo>);
    let selected_file = use_state(|| None::<String>);
    let file_diff = use_state(|| None::<String>);
    let show_git_panel = use_state(|| false);
    let git_refresh_interval = use_mut_ref(|| None::<Interval>);
    let commit_message = use_state(|| String::new());
    let show_commit_modal = use_state(|| false);
    let is_amend = use_state(|| false);

    // Git status info structure
    #[derive(Clone, PartialEq)]
    struct GitStatusInfo {
        branch: String,
        ahead: u32,
        behind: u32,
        modified: Vec<ModifiedFile>,
        staged: Vec<ModifiedFile>,
        untracked: Vec<String>,
        conflicts: Vec<String>,
    }

    #[derive(Clone, PartialEq)]
    struct ModifiedFile {
        path: String,
        change_type: String,
        additions: u32,
        deletions: u32,
    }

    // Diff line types for syntax highlighting
    #[derive(Clone, PartialEq)]
    enum DiffLine {
        Header(String),       // diff --git, index, ---, +++ lines
        ChunkHeader(String),  // @@ -x,x +x,x @@
        Context(String),      // Context lines (space prefix)
        Addition(String),     // Added lines (+ prefix)
        Deletion(String),     // Deleted lines (- prefix)
        Empty,                // Empty lines
    }

    // Parse diff text into structured lines
    fn parse_diff(diff_text: &str) -> Vec<DiffLine> {
        let mut lines = Vec::new();
        for line in diff_text.lines() {
            let diff_line = if line.starts_with("diff --git") {
                DiffLine::Header(line.to_string())
            } else if line.starts_with("index ") || line.starts_with("--- ") || line.starts_with("+++ ") {
                DiffLine::Header(line.to_string())
            } else if line.starts_with("@@") && line.contains("@@") {
                DiffLine::ChunkHeader(line.to_string())
            } else if line.starts_with('+') {
                DiffLine::Addition(line[1..].to_string())
            } else if line.starts_with('-') {
                DiffLine::Deletion(line[1..].to_string())
            } else if line.starts_with(' ') {
                DiffLine::Context(line[1..].to_string())
            } else if line.is_empty() {
                DiffLine::Empty
            } else {
                // Treat unknown lines as context
                DiffLine::Context(line.to_string())
            };
            lines.push(diff_line);
        }
        lines
    }

    // Render diff content as HTML
    fn render_diff_content(diff_text: &str) -> Html {
        let lines = parse_diff(diff_text);
        html! {
            <>
                { for lines.into_iter().map(|line| {
                    match line {
                        DiffLine::Header(text) => html! {
                            <div class="diff-line header">{ text }</div>
                        },
                        DiffLine::ChunkHeader(text) => html! {
                            <div class="diff-line chunk-header">{ text }</div>
                        },
                        DiffLine::Context(text) => html! {
                            <div class="diff-line context">
                                <span class="diff-marker">{" "}</span>
                                <span class="diff-text">{ text }</span>
                            </div>
                        },
                        DiffLine::Addition(text) => html! {
                            <div class="diff-line addition">
                                <span class="diff-marker">{"+"}</span>
                                <span class="diff-text">{ text }</span>
                            </div>
                        },
                        DiffLine::Deletion(text) => html! {
                            <div class="diff-line deletion">
                                <span class="diff-marker">{"-"}</span>
                                <span class="diff-text">{ text }</span>
                            </div>
                        },
                        DiffLine::Empty => html! {
                            <div class="diff-line empty">
                                <span class="diff-marker"></span>
                                <span class="diff-text"></span>
                            </div>
                        },
                    }
                })}
            </>
        }
    }

    // Close context menu on any click
    {
        let context_menu = context_menu.clone();
        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            let closure = Closure::wrap(Box::new(move |_e: MouseEvent| {
                context_menu.set(None);
            }) as Box<dyn FnMut(MouseEvent)>);
            let _ =
                window.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
            closure.forget();
            Box::new(|| {}) as Box<dyn FnOnce()>
        });
    }

    // Listen to URL hash changes (browser back/forward or manual hash change)
    {
        let selected_session_id = selected_session_id.clone();
        let sessions = sessions.clone();
        // selected_session_id_ref clone removed
        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            let closure = Closure::wrap(Box::new(move |_e: Event| {
                let location = web_sys::window().unwrap().location();
                let hash = location.hash().unwrap_or_default();
                let tag = if hash.len() > 1 {
                    Some(hash[1..].to_string())
                } else {
                    None
                };
                // Update selected_session_id based on hash
                let sessions_ref = sessions.borrow();
                if let Some(ref t) = tag {
                    if let Some(session) = sessions_ref.iter().find(|s| &s.tag == t) {
                        log::info!(
                            "Hash change matched session tag '{}' -> id '{}'",
                            t,
                            session.id
                        );
                        selected_session_id.set(Some(session.id.clone()));
                    }
                } else {
                    selected_session_id.set(None);
                }
            }) as Box<dyn FnMut(Event)>);
            let _ = window
                .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
            closure.forget();
            Box::new(|| {}) as Box<dyn FnOnce()>
        });
    }

    // Clone sessions_version for use outside the effect closure
    let sessions_version_clone = sessions_version.clone();

    // WebSocket setup
    {
        let ws_status = ws_status.clone();
        let sessions = sessions.clone();
        let machines = machines.clone();
        let selected_session_id = selected_session_id.clone();
        // selected_session_id_ref is unused in the effect, so we don't clone it
        let terminal_buffers = terminal_buffers.clone();
        let buffer_version = buffer_version.clone();
        let ws_ref = ws_ref.clone();
        let joined_tags_ref = joined_tags_ref.clone();
        let target_tag = target_tag.clone();
        let sessions_loaded_for_effect = sessions_loaded.clone();
        let sessions_version_for_effect = sessions_version.clone(); // Clone for use inside the effect
        let terminal_writer_for_effect = terminal_writer.clone();
        let git_status_for_effect = git_status.clone();
        let file_diff_for_effect = file_diff.clone();
        let show_commit_modal_for_effect = show_commit_modal.clone();
        let commit_message_for_effect = commit_message.clone();
        let ws_ref_for_effect = ws_ref.clone();

        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            let storage = window.local_storage().unwrap().unwrap();
            let auth_token = storage.get_item("happy_token").unwrap_or(None);

            if auth_token.is_none() {
                let location = window.location();
                let current_url = location.href().unwrap_or_default();
                let login_url = format!(
                    "/login?redirect={}",
                    js_sys::encode_uri_component(&current_url)
                );
                let _ = location.set_href(&login_url);
                return Box::new(|| {}) as Box<dyn FnOnce()>;
            }
            let auth_token = auth_token.unwrap_or_default();

            let location = window.location();
            let protocol = if location.protocol().unwrap() == "https:" {
                "wss"
            } else {
                "ws"
            };
            let host = location.host().unwrap();
            let ws_url = format!("{}://{}/ws", protocol, host);

            let ws = WebSocket::new(&ws_url).unwrap();
            ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

            let ws_clone = ws.clone();
            let ws_status_clone = ws_status.clone();

            let onopen = Closure::wrap(Box::new(move || {
                ws_status_clone.set("Authenticating...".to_string());
                let auth_msg = json!({ "type": "authenticate", "token": auth_token }).to_string();
                let _ = ws_clone.send_with_str(&auth_msg);
                let list_msg = json!({ "type": "list_sessions" }).to_string();
                let _ = ws_clone.send_with_str(&list_msg);
                // Also request machine list
                let machines_msg = json!({ "type": "list_machines" }).to_string();
                let _ = ws_clone.send_with_str(&machines_msg);
            }) as Box<dyn FnMut()>);
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();

            let ws_status_for_close = ws_status.clone();
            let onclose = Closure::wrap(Box::new(move || {
                ws_status_for_close.set("Disconnected".to_string());
            }) as Box<dyn FnMut()>);
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();

            let ws_status_for_msg = ws_status.clone();
            let sessions_for_msg = sessions.clone();
            let sessions_loaded_for_msg = sessions_loaded_for_effect.clone();
            let machines_for_msg = machines.clone();
            let selected_session_id_for_msg = selected_session_id.clone();
            let terminal_buffers_for_msg = terminal_buffers.clone();
            let buffer_version_for_msg = buffer_version.clone();
            let ws_for_msg = ws.clone();
            let joined_tags_ref_for_msg = joined_tags_ref.clone();
            let target_tag_for_msg = target_tag.clone();
            let sessions_version_for_msg = sessions_version_for_effect.clone();
            let git_status_for_msg = git_status_for_effect.clone();
            let file_diff_for_msg = file_diff_for_effect.clone();
            let show_commit_modal_for_msg = show_commit_modal_for_effect.clone();
            let commit_message_for_msg = commit_message_for_effect.clone();
            let ws_ref_for_msg = ws_ref_for_effect.clone();
            let terminal_writer_for_msg = terminal_writer_for_effect.clone();

            let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    let text = String::from(text);
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        // Log all message types except frequent ones
                        if msg_type != "terminal_output" && msg_type != "ping" && msg_type != "pong" {
                            log::info!("WS message type: {}", msg_type);
                        } else if msg_type == "terminal_output" {
                            // Debug: log when we receive terminal_output
                            if let Some(session_id) = json.get("session_id").and_then(|v| v.as_str()) {
                                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                                    log::info!("terminal_output received: session={}, bytes={}", session_id, data.len());
                                } else {
                                    log::warn!("terminal_output received but data is not an array: {:?}", json.get("data"));
                                }
                            } else {
                                log::warn!("terminal_output received but no session_id");
                            }
                        }
                        match msg_type {
                            "authenticated" => {
                                ws_status_for_msg.set("Connected".to_string());
                            }
                            "sessions_list" => {
                                log::info!("Received sessions_list message: {}", text);
                                let mut next_sessions = Vec::new();

                                if let Some(list) = json.get("sessions").and_then(|s| s.as_array())
                                {
                                    log::info!("sessions_list contains {} sessions", list.len());
                                    for session in list {
                                        if let (Some(id), Some(tag), Some(status)) = (
                                            session.get("id").and_then(|v| v.as_str()),
                                            session.get("tag").and_then(|v| v.as_str()),
                                            session.get("status").and_then(|v| v.as_str()),
                                        ) {
                                            let machine_id = session
                                                .get("machine_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();

                                            // Get machine name directly from session data
                                            let machine_name = session
                                                .get("machine_name")
                                                .and_then(|v| v.as_str())
                                                .map(|s| {
                                                    // Remove .local suffix if present
                                                    if s.ends_with(".local") {
                                                        s.trim_end_matches(".local").to_string()
                                                    } else {
                                                        s.to_string()
                                                    }
                                                })
                                                .unwrap_or_else(|| {
                                                    machine_id.chars().take(8).collect()
                                                });

                                            let cwd = session
                                                .get("metadata")
                                                .and_then(|m| m.get("cwd"))
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("/")
                                                .to_string();

                                            next_sessions.push(SessionSummary {
                                                id: id.to_string(),
                                                tag: tag.to_string(),
                                                status: status.to_string(),
                                                cwd,
                                                machine_id: machine_id.clone(),
                                                machine_name,
                                                is_online: false, // Will be updated when machines list arrives
                                            });

                                            // Auto-join sessions
                                            {
                                                match joined_tags_ref_for_msg.try_borrow_mut() {
                                                    Ok(mut joined) => {
                                                        if !joined.contains(tag) {
                                                            log::info!("Auto-joining session: tag='{}'", tag);
                                                            joined.insert(tag.to_string());
                                                            let join_msg = json!({
                                                                "type": "join_session",
                                                                "tag": tag
                                                            })
                                                            .to_string();
                                                            log::info!("Sending auto-join_session: {}", join_msg);
                                                            let result = ws_for_msg.send_with_str(&join_msg);
                                                            if let Err(e) = result {
                                                                log::error!("Failed to send auto-join_session: {:?}", e);
                                                            } else {
                                                                log::info!("Auto-join_session sent successfully for tag='{}'", tag);
                                                            }
                                                        } else {
                                                            log::debug!("Session '{}' already in joined list, skipping auto-join", tag);
                                                        }
                                                    }
                                                    Err(_) => {
                                                        log::warn!("joined_tags contention detected, deferring join: {}", tag);
                                                        let joined_clone =
                                                            joined_tags_ref_for_msg.clone();
                                                        let tag = tag.to_string();
                                                        let ws_clone = ws_for_msg.clone();
                                                        wasm_bindgen_futures::spawn_local(
                                                            async move {
                                                                if let Ok(mut joined) =
                                                                    joined_clone.try_borrow_mut()
                                                                {
                                                                    if !joined.contains(&tag) {
                                                                        log::info!("Auto-joining session (deferred): tag='{}'", tag);
                                                                        joined.insert(tag.clone());
                                                                        let join_msg = json!({
                                                                            "type": "join_session",
                                                                            "tag": tag
                                                                        })
                                                                        .to_string();
                                                                        let _ = ws_clone
                                                                            .send_with_str(
                                                                                &join_msg,
                                                                            );
                                                                    }
                                                                }
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if (*selected_session_id_for_msg).is_none() {
                                    if let Some(ref target) = target_tag_for_msg {
                                        if let Some(target_session) =
                                            next_sessions.iter().find(|s| &s.tag == target)
                                        {
                                            selected_session_id_for_msg
                                                .set(Some(target_session.id.clone()));
                                        }
                                    }
                                    if (*selected_session_id_for_msg).is_none()
                                        && !next_sessions.is_empty()
                                    {
                                        selected_session_id_for_msg
                                            .set(Some(next_sessions[0].id.clone()));
                                    }
                                }

                                log::info!(
                                    "next_sessions has {} items, current sessions has {} items",
                                    next_sessions.len(),
                                    sessions_for_msg.borrow().len()
                                );
                                {
                                    match sessions_for_msg.try_borrow_mut() {
                                        Ok(mut sessions_ref) => {
                                            *sessions_ref = next_sessions;
                                            sessions_version_for_msg
                                                .set(*sessions_version_for_msg + 1);
                                        }
                                        Err(_) => {
                                            log::warn!(
                                                "sessions contention detected (list), deferring..."
                                            );
                                            let sessions_clone = sessions_for_msg.clone();
                                            let version_clone = sessions_version_for_msg.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Ok(mut sessions_ref) =
                                                    sessions_clone.try_borrow_mut()
                                                {
                                                    *sessions_ref = next_sessions;
                                                    version_clone.set(*version_clone + 1);
                                                }
                                            });
                                        }
                                    }
                                }
                                // Mark sessions as loaded after first response
                                sessions_loaded_for_msg.set(true);
                            }
                            "session_updated" | "session_started" => {
                                // Only process if we already have sessions loaded
                                // (sessions_list is the source of truth for initial load)
                                if !sessions_for_msg.borrow().is_empty() {
                                    if let Some(session) = json.get("session") {
                                        if let (Some(id), Some(tag), Some(status)) = (
                                            session.get("id").and_then(|v| v.as_str()),
                                            session.get("tag").and_then(|v| v.as_str()),
                                            session.get("status").and_then(|v| v.as_str()),
                                        ) {
                                            let cwd = session
                                                .get("metadata")
                                                .and_then(|m| m.get("cwd"))
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("/")
                                                .to_string();
                                            let machine_id = session
                                                .get("machine_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            // Get machine name directly from session data
                                            let machine_name = session
                                                .get("machine_name")
                                                .and_then(|v| v.as_str())
                                                .map(|s| {
                                                    // Remove .local suffix if present
                                                    if s.ends_with(".local") {
                                                        s.trim_end_matches(".local").to_string()
                                                    } else {
                                                        s.to_string()
                                                    }
                                                })
                                                .unwrap_or_else(|| {
                                                    machine_id.chars().take(8).collect()
                                                });

                                            let mut next_sessions =
                                                sessions_for_msg.borrow().clone();
                                            if let Some(existing) =
                                                next_sessions.iter_mut().find(|s| s.id == id)
                                            {
                                                existing.tag = tag.to_string();
                                                existing.status = status.to_string();
                                                existing.cwd = cwd;
                                                existing.machine_name = machine_name;
                                            } else {
                                                next_sessions.push(SessionSummary {
                                                    id: id.to_string(),
                                                    tag: tag.to_string(),
                                                    status: status.to_string(),
                                                    cwd,
                                                    machine_id: machine_id.clone(),
                                                    machine_name,
                                                    is_online: false,
                                                });
                                            }
                                            match sessions_for_msg.try_borrow_mut() {
                                                Ok(mut sessions_ref) => {
                                                    *sessions_ref = next_sessions;
                                                    sessions_version_for_msg
                                                        .set(*sessions_version_for_msg + 1);
                                                }
                                                Err(_) => {
                                                    log::warn!("sessions contention detected (update), deferring...");
                                                    let sessions_clone = sessions_for_msg.clone();
                                                    let version_clone =
                                                        sessions_version_for_msg.clone();
                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        if let Ok(mut sessions_ref) =
                                                            sessions_clone.try_borrow_mut()
                                                        {
                                                            *sessions_ref = next_sessions;
                                                            version_clone.set(*version_clone + 1);
                                                        }
                                                    });
                                                }
                                            }
                                            // Trigger re-render
                                        }
                                    }
                                }
                            }
                            "session_stopped" => {
                                if let Some(session_id) =
                                    json.get("session_id").and_then(|v| v.as_str())
                                {
                                    log::info!("session_stopped received for: {}", session_id);
                                    let next_sessions: Vec<_> = sessions_for_msg
                                        .borrow()
                                        .iter()
                                        .cloned()
                                        .filter(|s| s.id != session_id)
                                        .collect();
                                    match sessions_for_msg.try_borrow_mut() {
                                        Ok(mut sessions_ref) => {
                                            *sessions_ref = next_sessions;
                                            sessions_version_for_msg
                                                .set(*sessions_version_for_msg + 1);
                                        }
                                        Err(_) => {
                                            log::warn!("sessions contention detected (stopped), deferring...");
                                            let sessions_clone = sessions_for_msg.clone();
                                            let version_clone = sessions_version_for_msg.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Ok(mut sessions_ref) =
                                                    sessions_clone.try_borrow_mut()
                                                {
                                                    *sessions_ref = next_sessions;
                                                    version_clone.set(*version_clone + 1);
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            "session_deleted" => {
                                if let Some(session_id) =
                                    json.get("session_id").and_then(|v| v.as_str())
                                {
                                    log::info!("session_deleted received for: {}", session_id);
                                    let current_count = sessions_for_msg.borrow().len();
                                    log::info!(
                                        "Current sessions count before delete: {}",
                                        current_count
                                    );

                                    let next_sessions: Vec<_> = sessions_for_msg
                                        .borrow()
                                        .iter()
                                        .cloned()
                                        .filter(|s| s.id != session_id)
                                        .collect();

                                    log::info!(
                                        "Next sessions count after delete: {}",
                                        next_sessions.len()
                                    );
                                    match sessions_for_msg.try_borrow_mut() {
                                        Ok(mut sessions_ref) => {
                                            *sessions_ref = next_sessions;
                                            sessions_version_for_msg
                                                .set(*sessions_version_for_msg + 1);
                                        }
                                        Err(_) => {
                                            log::warn!("sessions contention detected (delete), deferring...");
                                            let sessions_clone = sessions_for_msg.clone();
                                            let version_clone = sessions_version_for_msg.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Ok(mut sessions_ref) =
                                                    sessions_clone.try_borrow_mut()
                                                {
                                                    *sessions_ref = next_sessions;
                                                    version_clone.set(*version_clone + 1);
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            "terminal_history" => {
                                if let (Some(session_id), Some(data)) = (
                                    json.get("session_id").and_then(|v| v.as_str()),
                                    json.get("data").and_then(|d| d.as_array()),
                                ) {
                                    let bytes: Vec<u8> = data
                                        .iter()
                                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                                        .collect();
                                    let text = String::from_utf8_lossy(&bytes).to_string();
                                    log::info!("terminal_history received: session={}, bytes={}, text_len={}, is_current={}",
                                        session_id, bytes.len(), text.len(),
                                        selected_session_id_for_msg.as_ref().map(|s| s == session_id).unwrap_or(false));

                                    match terminal_buffers_for_msg.try_borrow_mut() {
                                        Ok(mut buffers) => {
                                            buffers.insert(session_id.to_string(), text);
                                            buffer_version_for_msg.set(*buffer_version_for_msg + 1);
                                            log::info!("terminal_history stored in buffer for session {}", session_id);
                                        }
                                        Err(_) => {
                                            log::warn!("terminal_buffers contention detected for history, deferring...");
                                            let buffers_clone = terminal_buffers_for_msg.clone();
                                            let session_id = session_id.to_string();
                                            let version_clone = buffer_version_for_msg.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Ok(mut buffers) =
                                                    buffers_clone.try_borrow_mut()
                                                {
                                                    buffers.insert(session_id, text);
                                                    version_clone.set(*version_clone + 1);
                                                }
                                            });
                                        }
                                    }
                                } else {
                                    log::warn!("terminal_history message missing session_id or data: {:?}", json);
                                }
                            }
                            "terminal_output" => {
                                if let (Some(session_id), Some(data)) = (
                                    json.get("session_id").and_then(|v| v.as_str()),
                                    json.get("data").and_then(|d| d.as_array()),
                                ) {
                                    let bytes: Vec<u8> = data
                                        .iter()
                                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                                        .collect();

                                    // Check if this is the currently selected session
                                    let is_current_session = selected_session_id_for_msg.as_ref().map(|s| s == session_id).unwrap_or(false);

                                    // ALWAYS store to buffer first
                                    let text = String::from_utf8_lossy(&bytes).to_string();

                                    // Check if this is the currently selected session
                                    let is_current_session = selected_session_id_for_msg.as_ref().map(|s| s == session_id).unwrap_or(false);

                                    log::info!("terminal_output: session={}, bytes={}, is_current={}", session_id, bytes.len(), is_current_session);

                                    // For current session: also write directly to terminal for real-time display
                                    if is_current_session {
                                        let writer_opt = terminal_writer_for_msg.borrow().clone();
                                        if let Some(ref writer) = writer_opt {
                                            log::info!("terminal_output: emitting {} bytes via writer", bytes.len());
                                            writer.emit(bytes.clone());
                                        } else {
                                            log::warn!("terminal_output: no writer available, content will show on next render");
                                        }
                                    }

                                    // ALWAYS update buffer for persistence and display
                                    match terminal_buffers_for_msg.try_borrow_mut() {
                                        Ok(mut buffers) => {
                                            let buffer = buffers.entry(session_id.to_string()).or_default();
                                            buffer.push_str(&text);
                                            // Limit buffer size
                                            if buffer.len() > 100000 {
                                                *buffer = buffer[buffer.len() - 80000..].to_string();
                                            }
                                            // Trigger re-render to show content
                                            buffer_version_for_msg.set(*buffer_version_for_msg + 1);
                                            log::info!("terminal_output: buffer updated for session {}, total len={}", session_id, buffer.len());
                                        }
                                        Err(_) => {
                                            let buffers_clone = terminal_buffers_for_msg.clone();
                                            let session_id = session_id.to_string();
                                            let version_clone = buffer_version_for_msg.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Ok(mut buffers) = buffers_clone.try_borrow_mut() {
                                                    let buffer = buffers.entry(session_id).or_default();
                                                    buffer.push_str(&text);
                                                    if buffer.len() > 100000 {
                                                        *buffer = buffer[buffer.len() - 80000..].to_string();
                                                    }
                                                    version_clone.set(*version_clone + 1);
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            "error" => {
                                let code = json.get("code").and_then(|c| c.as_str()).unwrap_or("");
                                let message =
                                    json.get("message").and_then(|m| m.as_str()).unwrap_or("");
                                log::warn!("Error: {} - {}", code, message);
                                if code == "auth_failed" || code == "not_authenticated" {
                                    let window = web_sys::window().unwrap();
                                    let storage = window.local_storage().unwrap().unwrap();
                                    let _ = storage.remove_item("happy_token");
                                    let _ = window.location().set_href("/login");
                                }
                            }
                            "git_status" => {
                                if let (Some(session_id), Some(branch)) = (
                                    json.get("session_id").and_then(|v| v.as_str()),
                                    json.get("branch").and_then(|v| v.as_str()),
                                ) {
                                    let ahead = json.get("ahead").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                    let behind = json.get("behind").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                                    let parse_files = |arr: Option<&Vec<serde_json::Value>>| -> Vec<ModifiedFile> {
                                        arr.unwrap_or(&Vec::new()).iter().filter_map(|f| {
                                            Some(ModifiedFile {
                                                path: f.get("path")?.as_str()?.to_string(),
                                                change_type: f.get("change_type")?.as_str()?.to_string(),
                                                additions: f.get("additions")?.as_u64()? as u32,
                                                deletions: f.get("deletions")?.as_u64()? as u32,
                                            })
                                        }).collect()
                                    };

                                    let modified = parse_files(json.get("modified").and_then(|v| v.as_array()));
                                    let staged = parse_files(json.get("staged").and_then(|v| v.as_array()));
                                    let untracked: Vec<String> = json.get("untracked")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default();
                                    let conflicts: Vec<String> = json.get("conflicts")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default();

                                    let git_status = git_status_for_msg.clone();
                                    git_status.set(Some(GitStatusInfo {
                                        branch: branch.to_string(),
                                        ahead,
                                        behind,
                                        modified,
                                        staged,
                                        untracked,
                                        conflicts,
                                    }));
                                }
                            }
                            "git_diff" => {
                                if let (Some(_session_id), Some(_path), Some(diff)) = (
                                    json.get("session_id").and_then(|v| v.as_str()),
                                    json.get("path").and_then(|v| v.as_str()),
                                    json.get("diff").and_then(|v| v.as_str()),
                                ) {
                                    file_diff_for_msg.set(Some(diff.to_string()));
                                }
                            }
                            "git_commit_result" => {
                                if let (Some(session_id), Some(success)) = (
                                    json.get("session_id").and_then(|v| v.as_str()),
                                    json.get("success").and_then(|v| v.as_bool()),
                                ) {
                                    let msg_text = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
                                    if success {
                                        // Clear commit message and refresh git status
                                        commit_message_for_msg.set(String::new());
                                        show_commit_modal_for_msg.set(false);
                                        // Request fresh git status
                                        if let Some(ws) = ws_ref_for_msg.borrow().as_ref() {
                                            let msg = json!({
                                                "type": "get_git_status",
                                                "session_id": session_id
                                            });
                                            let _ = ws.send_with_str(&msg.to_string());
                                        }
                                    }
                                    // Could show toast notification here
                                    log::info!("Commit result: {} - {}", success, msg_text);
                                }
                            }
                            "remote_session_response" => {
                                let success = json
                                    .get("success")
                                    .and_then(|s| s.as_bool())
                                    .unwrap_or(false);
                                if success {
                                    if let Some(session) = json.get("session") {
                                        if let (Some(id), Some(tag)) = (
                                            session.get("id").and_then(|v| v.as_str()),
                                            session.get("tag").and_then(|v| v.as_str()),
                                        ) {
                                            log::info!("Remote session created: {} ({})", tag, id);
                                            // Auto-select the new session
                                            selected_session_id_for_msg.set(Some(id.to_string()));
                                            let list_msg =
                                                json!({ "type": "list_sessions" }).to_string();
                                            let _ = ws_for_msg.send_with_str(&list_msg);
                                        }
                                    }
                                } else {
                                    let error = json
                                        .get("error")
                                        .and_then(|e| e.as_str())
                                        .unwrap_or("Unknown error");
                                    log::error!("Failed to create remote session: {}", error);
                                }
                            }
                            "machine_list" => {
                                if let Some(machine_list) =
                                    json.get("machines").and_then(|m| m.as_array())
                                {
                                    let mut next_machines = Vec::new();
                                    let mut online_machine_ids = std::collections::HashSet::new();
                                    for m in machine_list {
                                        if let (Some(id), Some(name), Some(online)) = (
                                            m.get("id").and_then(|v| v.as_str()),
                                            m.get("name").and_then(|v| v.as_str()),
                                            m.get("is_online").and_then(|v| v.as_bool()),
                                        ) {
                                            next_machines.push(MachineInfo {
                                                id: id.to_string(),
                                                name: name.to_string(),
                                            });
                                            if online {
                                                online_machine_ids.insert(id.to_string());
                                            }
                                            log::info!(
                                                "Machine: {} ({}) - online: {}",
                                                name,
                                                id,
                                                online
                                            );
                                        }
                                    }
                                    match machines_for_msg.try_borrow_mut() {
                                        Ok(mut machines_ref) => {
                                            *machines_ref = next_machines;
                                        }
                                        Err(_) => {
                                            log::warn!(
                                                "machines contention detected, deferring..."
                                            );
                                            let machines_clone = machines_for_msg.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Ok(mut machines_ref) =
                                                    machines_clone.try_borrow_mut()
                                                {
                                                    *machines_ref = next_machines;
                                                }
                                            });
                                        }
                                    }

                                    // Update sessions' is_online status based on machine status
                                    let mut updated_sessions = sessions_for_msg.borrow().clone();
                                    let mut has_changes = false;
                                    for session in updated_sessions.iter_mut() {
                                        let new_online =
                                            online_machine_ids.contains(&session.machine_id);
                                        if session.is_online != new_online {
                                            session.is_online = new_online;
                                            has_changes = true;
                                        }
                                    }
                                    if has_changes {
                                        {
                                            let mut sessions_ref = sessions_for_msg.borrow_mut();
                                            *sessions_ref = updated_sessions;
                                        }
                                        sessions_version_for_msg.set(*sessions_version_for_msg + 1);
                                        // Trigger re-render
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }) as Box<dyn FnMut(MessageEvent)>);
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();

            *ws_ref.borrow_mut() = Some(ws.clone());

            Box::new(move || {
                if let Some(ws) = ws_ref.borrow().as_ref() {
                    let _ = ws.close();
                }
            }) as Box<dyn FnOnce()>
        });
    }

    // Git status auto-refresh every 20 seconds when panel is open
    {
        let show_git_panel = show_git_panel.clone();
        let selected_session_id = selected_session_id.clone();
        let ws_ref = ws_ref.clone();
        let git_refresh_interval = git_refresh_interval.clone();

        use_effect_with(
            (*show_git_panel.clone(), (*selected_session_id).clone()),
            move |(is_open, session_id): &(bool, Option<String>)| {
                // Clear existing interval
                if let Some(interval) = git_refresh_interval.borrow_mut().take() {
                    drop(interval);
                }

                // Start new interval if panel is open and session is selected
                if *is_open {
                    if let Some(ref sid) = session_id {
                        let ws_ref = ws_ref.clone();
                        let session_id = sid.clone();

                        let interval = Interval::new(20_000, move || {
                            if let Some(ws) = ws_ref.borrow().as_ref() {
                                let msg = json!({
                                    "type": "get_git_status",
                                    "session_id": session_id
                                });
                                let _ = ws.send_with_str(&msg.to_string());
                            }
                        });

                        *git_refresh_interval.borrow_mut() = Some(interval);
                    }
                }

                // Cleanup function
                Box::new(move || {
                    if let Some(interval) = git_refresh_interval.borrow_mut().take() {
                        drop(interval);
                    }
                }) as Box<dyn FnOnce()>
            },
        );
    }

    let on_input = {
        let input_value = input_value.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            input_value.set(input.value());
        })
    };

    let on_terminal_input = {
        let selected_session_id = selected_session_id.clone();
        let ws_ref = ws_ref.clone();
        Callback::from(move |data: Vec<u8>| {
            // Use the selected_session_id state directly
            // Note: This works because when selected_session_id changes, the component re-renders,
            // creating a new on_terminal_input callback with the new captured value.
            // And now XTerm::changed updates the callback even if the ID prop doesn't change (though it should).
            if let Some(session_id) = (*selected_session_id).clone() {
                let msg = json!({
                    "type": "terminal_input",
                    "session_id": session_id,
                    "data": data
                });
                log::info!(
                    "Sending terminal_input for session_id: {} ({} bytes)",
                    session_id,
                    data.len()
                );
                if let Some(ws) = ws_ref.borrow().as_ref() {
                    let _ = ws.send_with_str(&msg.to_string());
                }
            } else {
                log::warn!("Attempted to send terminal input but selected_session_id is None");
            }
        })
    };

    // Key sequences for virtual keyboard
    // Arrow keys use ANSI escape sequences
    // Enter uses \r (carriage return)
    let on_key_up = {
        let key_sender = key_sender.clone();
        Callback::from(move |_| {
            if let Some(ref sender) = *key_sender {
                sender.emit("\x1b[A".to_string()); // ANSI up arrow
            }
        })
    };

    let on_key_down = {
        let key_sender = key_sender.clone();
        Callback::from(move |_| {
            if let Some(ref sender) = *key_sender {
                sender.emit("\x1b[B".to_string()); // ANSI down arrow
            }
        })
    };

    let on_key_left = {
        let key_sender = key_sender.clone();
        Callback::from(move |_| {
            if let Some(ref sender) = *key_sender {
                sender.emit("\x1b[D".to_string()); // ANSI left arrow
            }
        })
    };

    let on_key_right = {
        let key_sender = key_sender.clone();
        Callback::from(move |_| {
            if let Some(ref sender) = *key_sender {
                sender.emit("\x1b[C".to_string()); // ANSI right arrow
            }
        })
    };

    let on_key_enter = {
        let key_sender = key_sender.clone();
        Callback::from(move |_| {
            if let Some(ref sender) = *key_sender {
                sender.emit("\r".to_string()); // Enter key (carriage return)
            }
        })
    };

    let on_send = {
        let input_value = input_value.clone();
        let selected_session_id = selected_session_id.clone();
        let ws_ref = ws_ref.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let text = input_value.trim().to_string();
            if text.is_empty() {
                return;
            }
            if let Some(session_id) = (*selected_session_id).clone() {
                let payload = format!("{}\n", text);
                let msg = json!({
                    "type": "terminal_input",
                    "session_id": session_id,
                    "data": payload.into_bytes()
                });
                if let Some(ws) = ws_ref.borrow().as_ref() {
                    let _ = ws.send_with_str(&msg.to_string());
                }
                input_value.set(String::new());
            }
        })
    };

    let selected_session = (*selected_session_id)
        .as_ref()
        .and_then(|id| sessions.borrow().iter().find(|s| &s.id == id).cloned());

    let target_session_missing = if let Some(ref target) = target_tag {
        !sessions.borrow().iter().any(|s| &s.tag == target)
    } else {
        false
    };

    let _version = *buffer_version;
    // Use state for terminal_content so it updates when buffer changes
    let terminal_content_state = use_state(|| String::new());
    {
        let terminal_content_state = terminal_content_state.clone();
        let terminal_buffers = terminal_buffers.clone();
        let selected_session_id = selected_session_id.clone();
        use_effect_with(
            (*buffer_version, (*selected_session_id).clone()),
            move |(version, session_id): &(u32, Option<String>)| {
                let buffers = terminal_buffers.borrow();
                let content = session_id
                    .as_ref()
                    .and_then(|id| buffers.get(id).cloned())
                    .unwrap_or_default();
                log::info!("terminal_content updated: version={}, session={:?}, content_len={}", version, session_id, content.len());
                terminal_content_state.set(content);
                || ()
            },
        );
    }
    let terminal_content = (*terminal_content_state).clone();

    // Clone terminal_writer for use in XTerm component
    let terminal_writer_for_xterm = terminal_writer.clone();

    let sidebar_class = if *mobile_view == MobileView::SessionList {
        "chat-sidebar"
    } else {
        "chat-sidebar mobile-hidden"
    };
    let panel_class = if *mobile_view == MobileView::Terminal {
        "chat-panel"
    } else {
        "chat-panel mobile-hidden"
    };

    let on_back = {
        let mobile_view = mobile_view.clone();
        Callback::from(move |_| {
            mobile_view.set(MobileView::SessionList);
        })
    };

    // Git status toggle
    let on_toggle_git_panel = {
        let show_git_panel = show_git_panel.clone();
        let selected_session_id = selected_session_id.clone();
        let ws_ref = ws_ref.clone();
        Callback::from(move |_| {
            let new_state = !*show_git_panel;
            show_git_panel.set(new_state);

            // If opening and we have a selected session, request git status
            if new_state {
                if let Some(ref session_id) = *selected_session_id {
                    if let Some(ws) = ws_ref.borrow().as_ref() {
                        let msg = json!({
                            "type": "get_git_status",
                            "session_id": session_id
                        });
                        let _ = ws.send_with_str(&msg.to_string());
                    }
                }
            }
        })
    };

    // Git file selection
    let on_select_file = {
        let selected_file = selected_file.clone();
        let file_diff = file_diff.clone();
        let selected_session_id = selected_session_id.clone();
        let ws_ref = ws_ref.clone();
        Callback::from(move |path: String| {
            selected_file.set(Some(path.clone()));

            // Request diff for this file
            if let Some(ref session_id) = *selected_session_id {
                if let Some(ws) = ws_ref.borrow().as_ref() {
                    let msg = json!({
                        "type": "get_git_diff",
                        "session_id": session_id,
                        "path": path
                    });
                    let _ = ws.send_with_str(&msg.to_string());
                }
            }
        })
    };

    // Git commit
    let on_commit = {
        let show_commit_modal = show_commit_modal.clone();
        let is_amend = is_amend.clone();
        Callback::from(move |amend: bool| {
            is_amend.set(amend);
            show_commit_modal.set(true);
        })
    };

    // Submit commit
    let on_submit_commit = {
        let selected_session_id = selected_session_id.clone();
        let commit_message = commit_message.clone();
        let is_amend = is_amend.clone();
        let ws_ref = ws_ref.clone();
        Callback::from(move |_| {
            if let Some(ref session_id) = *selected_session_id {
                let msg = json!({
                    "type": "git_commit",
                    "session_id": session_id,
                    "message": (*commit_message).clone(),
                    "amend": *is_amend
                });
                if let Some(ws) = ws_ref.borrow().as_ref() {
                    let _ = ws.send_with_str(&msg.to_string());
                }
            }
        })
    };

    // Delete session callback
    let on_delete_session = {
        let ws_ref = ws_ref.clone();
        let selected_session_id = selected_session_id.clone();
        let delete_confirm = delete_confirm.clone();
        Callback::from(move |session_id: String| {
            let msg = json!({
                "type": "delete_session",
                "session_id": session_id
            });
            if let Some(ws) = ws_ref.borrow().as_ref() {
                let _ = ws.send_with_str(&msg.to_string());
            }
            if *selected_session_id == Some(session_id.clone()) {
                selected_session_id.set(None);
            }
            delete_confirm.set(None);
        })
    };

    // Right-click handler for context menu
    let on_context_menu = {
        let context_menu = context_menu.clone();
        Callback::from(move |(e, session_id, tag): (MouseEvent, String, String)| {
            e.prevent_default();
            e.stop_propagation();
            context_menu.set(Some((e.client_x(), e.client_y(), session_id, tag)));
        })
    };

    // Group sessions by machine, then by folder
    // Use sessions_version to ensure re-computation when sessions change
    let _sessions_ver = *sessions_version_clone;
    let grouped_sessions: HashMap<String, HashMap<String, Vec<SessionSummary>>> = {
        let sessions_ref = sessions.borrow();
        let mut machine_groups: HashMap<String, HashMap<String, Vec<SessionSummary>>> =
            HashMap::new();

        for session in sessions_ref.iter() {
            let machine_key = if session.machine_name.is_empty() {
                "unknown".to_string()
            } else {
                session.machine_name.clone()
            };
            let folder = session.folder_name().to_string();

            machine_groups
                .entry(machine_key)
                .or_default()
                .entry(folder)
                .or_default()
                .push(session.clone());
        }

        // Sort sessions within each folder
        for machine_folders in machine_groups.values_mut() {
            for folder_sessions in machine_folders.values_mut() {
                folder_sessions.sort_by(|a, b| match (a.status.as_str(), b.status.as_str()) {
                    ("running", "running") => a.tag.cmp(&b.tag),
                    ("running", _) => std::cmp::Ordering::Less,
                    (_, "running") => std::cmp::Ordering::Greater,
                    _ => a.tag.cmp(&b.tag),
                });
            }
        }
        machine_groups
    };

    log::info!(
        "grouped_sessions: {} machines, total sessions: {}",
        grouped_sessions.len(),
        grouped_sessions
            .values()
            .flat_map(|f| f.values())
            .map(|v| v.len())
            .sum::<usize>()
    );

    let mut sorted_machines: Vec<_> = grouped_sessions.keys().cloned().collect();
    sorted_machines.sort();

    // Create session handler
    let on_create_session = {
        let ws_ref = ws_ref.clone();
        let machines = machines.clone(); // Capture machines list
        let create_cwd = create_cwd.clone();
        let create_machine = create_machine.clone();
        let create_args = create_args.clone();
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let cwd = (*create_cwd).clone();
            let mut machine_id = (*create_machine).clone();
            let args = (*create_args).clone();

            // If no machine selected, try to default to the first available machine
            if machine_id.is_empty() {
                if let Some(first_machine) = machines.borrow().first() {
                    machine_id = first_machine.id.clone();
                    log::info!("No machine selected, defaulting to: {}", machine_id);
                } else {
                    log::warn!("No machines available for session creation");
                }
            }

            let msg = json!({
                "type": "request_remote_session",
                "machine_id": if machine_id.is_empty() { "local" } else { &machine_id },
                "cwd": if cwd.is_empty() { None } else { Some(cwd) },
                "args": if args.is_empty() { None } else { Some(args) }
            });
            if let Some(ws) = ws_ref.borrow().as_ref() {
                let _ = ws.send_with_str(&msg.to_string());
            }

            show_create_modal.set(false);
            create_cwd.set(String::new());
            create_machine.set(String::new());
            create_args.set(String::new());
        })
    };

    // Clone state for use in html! closures
    let show_create_modal_clone = show_create_modal.clone();
    let delete_confirm_clone = delete_confirm.clone();
    let delete_confirm_for_overlay = delete_confirm.clone();
    let delete_confirm_for_cancel = delete_confirm.clone();
    let show_create_modal_for_overlay = show_create_modal.clone();
    let show_create_modal_for_cancel = show_create_modal.clone();

    // Commit modal clones
    let show_commit_modal_for_modal = show_commit_modal.clone();
    let show_commit_modal_for_cancel = show_commit_modal.clone();
    let commit_message_for_modal = commit_message.clone();
    let commit_message_for_input = commit_message.clone();
    let is_amend_for_modal = *is_amend;

    html! {
        <div class="chat-page">
            <header class="chat-header">
                if *mobile_view == MobileView::Terminal {
                    <button class="chat-back-btn" onclick={on_back}>{ "←" }</button>
                }
                <div class="chat-title">
                    <div class="logo">
                        <span class="logo-icon">{ "🚀" }</span>
                        <h1>{ "Happy Coding" }</h1>
                    </div>
                </div>
                <div class="header-actions">
                    <button class="btn-create-session" onclick={Callback::from(move |_| show_create_modal_clone.set(true))}>
                        { "+" }
                    </button>
                </div>
            </header>

            <main class="chat-main">
                <aside class={sidebar_class}>
                    <div class="chat-sidebar-header">{ "会话列表" }</div>
                    <div class="chat-session-list">
                        {
                            if !*sessions_loaded {
                                // Loading state
                                html! {
                                    <div class="session-list-empty">
                                        <div class="loading-spinner"></div>
                                        <p>{ "加载中..." }</p>
                                    </div>
                                }
                            } else if sessions.borrow().is_empty() {
                                // Empty state with setup guide
                                html! {
                                    <div class="session-list-empty">
                                        <div class="empty-icon">{ "🚀" }</div>
                                        <h3>{ "欢迎使用 Happy Coding" }</h3>
                                        <p>{ "在本地电脑运行以下命令启动会话：" }</p>
                                        <div class="setup-code">
                                            <code>{ "happy run claude --remote" }</code>
                                        </div>
                                        <small>{ "运行后即可在此处查看和管理会话" }</small>
                                    </div>
                                }
                            } else {
                                // Normal session list
                                html! {
                                    { for sorted_machines.iter().map(|machine_name| {
                            let folders = grouped_sessions.get(machine_name).unwrap();
                            let mut sorted_folders: Vec<_> = folders.keys().cloned().collect();
                            sorted_folders.sort();

                            html! {
                                <div class="machine-group">
                                    <div class="machine-group-header">
                                        <span class="machine-icon">{ "💻" }</span>
                                        { machine_name }
                                    </div>
                                    { for sorted_folders.iter().map(|folder| {
                                        let sessions_in_folder = folders.get(folder).unwrap();
                                        html! {
                                            <div class="session-folder-group">
                                                <div class="session-folder-header">{ folder }</div>
                                                { for sessions_in_folder.iter().map(|session| {
                                                    let is_selected = (*selected_session_id)
                                                        .as_ref()
                                                        .map(|id| id == &session.id)
                                                        .unwrap_or(false);

                                                    let session_id = session.id.clone();
                                                    let session_tag = session.tag.clone();
                                                    let session_status = session.status.clone();
                                                    let session_id_for_delete = session.id.clone();
                                                    let session_tag_for_delete = session.tag.clone();
                                                    let mobile_view_clone = mobile_view.clone();

                                                    let on_select = {
                                                        let selected_session_id = selected_session_id.clone();
                                                        // selected_session_id_ref removed
                                                        let mobile_view = mobile_view_clone.clone();
                                                        let ws_ref = ws_ref.clone();
                                                        let joined_tags_ref = joined_tags_ref.clone();
                                                        let session_tag = session_tag.clone();
                                                        Callback::from(move |_| {
                                                            log::info!("Session selected: tag='{}' id='{}'", session_tag, session_id);
                                                            selected_session_id.set(Some(session_id.clone()));
                                                            mobile_view.set(MobileView::Terminal);

                                                            // Update URL hash without reloading page
                                                            if let Some(window) = web_sys::window() {
                                                                let location = window.location();
                                                                let _ = location.set_hash(&session_tag);
                                                            }

                                                            // Join the session if not already joined
                                                            {
                                                                let mut joined = joined_tags_ref.borrow_mut();
                                                                log::info!("Checking if need to join: tag='{}', already_joined={}, status='{}'", session_tag, joined.contains(&session_tag), session_status);
                                                                if !joined.contains(&session_tag) {
                                                                    // Only join if session is not terminated
                                                                    if session_status != "terminated" {
                                                                        joined.insert(session_tag.clone());
                                                                        let join_msg = json!({
                                                                            "type": "join_session",
                                                                            "tag": &session_tag
                                                                        }).to_string();
                                                                        log::info!("Sending join_session message: {}", join_msg);
                                                                        if let Some(ws) = ws_ref.borrow().as_ref() {
                                                                            let result = ws.send_with_str(&join_msg);
                                                                            if let Err(e) = result {
                                                                                log::error!("Failed to send join_session: {:?}", e);
                                                                            } else {
                                                                                log::info!("join_session message sent successfully");
                                                                            }
                                                                        } else {
                                                                            log::warn!("WebSocket not available, cannot send join_session");
                                                                        }
                                                                    } else {
                                                                        log::info!("Session is terminated, not sending join_session");
                                                                    }
                                                                } else {
                                                                    log::info!("Already joined session '{}', skipping join", session_tag);
                                                                }
                                                            }
                                                        })
                                                    };

                                                    let on_right_click = {
                                                        let context_menu = context_menu.clone();
                                                        let session_id = session_id_for_delete.clone();
                                                        let session_tag = session_tag_for_delete.clone();
                                                        Callback::from(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            e.stop_propagation();
                                                            context_menu.set(Some((e.client_x(), e.client_y(), session_id.clone(), session_tag.clone())));
                                                        })
                                                    };

                                                    let status_cls = match session.status.as_str() {
                                                        "running" => "running",
                                                        "initializing" => "initializing",
                                                        "terminated" => "exited",
                                                        _ => "",
                                                    };
                                                    let conn_cls = if session.is_online { "conn-online" } else { "conn-offline" };

                                                    html! {
                                                        <div
                                                            class={classes!("chat-session-item", if is_selected { "selected" } else { "" })}
                                                            oncontextmenu={on_right_click}
                                                        >
                                                            <button class="session-select-btn" onclick={on_select}>
                                                                <div class="session-title-row">
                                                                    <span class="chat-session-tag">{ session.tag.clone() }</span>
                                                                    <span class={classes!("conn-dot", conn_cls)} title={if session.is_online { "在线" } else { "离线" }}/>
                                                                </div>
                                                                <div class="session-status-row">
                                                                    <span class={classes!("status-badge", status_cls)}>{ session.status.clone() }</span>
                                                                </div>
                                                            </button>
                                                        </div>
                                                    }
                                                }) }
                                            </div>
                                        }
                                    }) }
                                </div>
                            }
                        }) }
                                }
                            }
                        }
                    </div>
                </aside>

                <section class={panel_class}>
                    <div class="terminal-container" style="background: #0d1117;">
                        {
                            if !*sessions_loaded {
                                // Loading state
                                html! {
                                    <div class="terminal-empty">
                                        <div class="loading-spinner"></div>
                                        <p>{ "加载中..." }</p>
                                    </div>
                                }
                            } else if sessions.borrow().is_empty() {
                                // Empty state - same as sidebar
                                html! {
                                    <div class="terminal-empty">
                                        <div class="empty-icon">{ "🚀" }</div>
                                        <h3>{ "欢迎使用 Happy Coding" }</h3>
                                        <p>{ "在本地电脑运行以下命令启动会话：" }</p>
                                        <div class="setup-code">
                                            <code>{ "happy run claude --remote" }</code>
                                        </div>
                                        <small>{ "运行后即可在此处查看和管理会话" }</small>
                                    </div>
                                }
                            } else if target_session_missing {
                                // Session not found
                                html! {
                                    <div class="terminal-empty">
                                        <div class="empty-icon">{ "❓" }</div>
                                        <h3>{ format!("Session '{}' 不存在", target_tag.as_ref().unwrap_or(&String::new())) }</h3>
                                        <p>{ "该会话可能已结束或从未创建" }</p>
                                        <div class="setup-code">
                                            <code>{ "happy run claude --remote" }</code>
                                        </div>
                                    </div>
                                }
                            } else if (*selected_session_id).is_none() {
                                // No session selected
                                html! {
                                    <div class="terminal-empty">
                                        <div class="empty-icon">{ "👈" }</div>
                                        <h3>{ "选择一个会话" }</h3>
                                        <p>{ "从左侧列表选择一个会话开始交互" }</p>
                                    </div>
                                }
                            } else {
                                // Show terminal with header
                                let terminal_writer_clone = terminal_writer_for_xterm.clone();
                                let scroll_to_bottom_clone = scroll_to_bottom.clone();
                                let show_scroll_to_bottom_clone = show_scroll_to_bottom.clone();
                                let session_id_for_header = (*selected_session_id).as_ref().unwrap_or(&String::new()).clone();
                                let session_tag_for_header = sessions.borrow().iter()
                                    .find(|s| s.id == session_id_for_header)
                                    .map(|s| s.tag.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());
                                html! {
                                    <>
                                        <div class="terminal-header">
                                            <div class="terminal-header-info">
                                                <span class="terminal-session-tag">{ session_tag_for_header }</span>
                                                <span class="terminal-session-id">{ format!("({})", &session_id_for_header[..8.min(session_id_for_header.len())]) }</span>
                                            </div>
                                            <button
                                                class={classes!("btn-terminal-git", if *show_git_panel { "active" } else { "" })}
                                                onclick={on_toggle_git_panel.clone()}
                                            >
                                                { "📝 变更" }
                                                if let Some(ref status) = *git_status {
                                                    if !status.modified.is_empty() || !status.staged.is_empty() {
                                                        <span class="git-badge">{ status.modified.len() + status.staged.len() }</span>
                                                    }
                                                }
                                            </button>
                                        </div>
                                        <div class="terminal-content">
                                            <XTerm
                                                id={format!("terminal-{}", session_id_for_header)}
                                                initial_content={terminal_content}
                                                on_input={on_terminal_input.clone()}
                                                on_key_sender={Callback::from(move |sender| {
                                                    key_sender.set(Some(sender));
                                                })}
                                                on_writer={Callback::from(move |writer| {
                                                    *terminal_writer_clone.borrow_mut() = Some(writer);
                                                })}
                                                on_scroll_to_bottom={Callback::from(move |cb| {
                                                    scroll_to_bottom_clone.set(Some(cb));
                                                })}
                                                on_scroll_state_change={Callback::from(move |scrolled_up| {
                                                    show_scroll_to_bottom_clone.set(scrolled_up);
                                                })}
                                                read_only=false
                                            />
                                            {if *show_scroll_to_bottom {
                                                let scroll_cb = scroll_to_bottom.clone();
                                                let show_cb = show_scroll_to_bottom.clone();
                                                html! {
                                                    <button
                                                        class="btn-scroll-to-bottom"
                                                        onclick={Callback::from(move |_| {
                                                            if let Some(ref cb) = *scroll_cb {
                                                                cb.emit(());
                                                            }
                                                            show_cb.set(false);
                                                        })}
                                                    >
                                                        { "⬇ 回到底部" }
                                                    </button>
                                                }
                                            } else {
                                                html! {}
                                            }}
                                        </div>
                                    </>
                                }
                            }
                        }
                    </div>
                    // Git status panel
                    if *show_git_panel && (*selected_session_id).is_some() {
                        <div class="git-panel">
                            <div class="git-panel-header">
                                <div class="git-branch">
                                    if let Some(ref status) = *git_status {
                                        <>
                                            <span class="branch-name">{ format!("🌿 {}", status.branch) }</span>
                                            if status.ahead > 0 || status.behind > 0 {
                                                <span class="sync-status">
                                                    if status.ahead > 0 {
                                                        <span class="ahead">{ format!("↑{}", status.ahead) }</span>
                                                    }
                                                    if status.behind > 0 {
                                                        <span class="behind">{ format!("↓{}", status.behind) }</span>
                                                    }
                                                </span>
                                            }
                                        </>
                                    } else {
                                        <span class="loading">{ "加载中..." }</span>
                                    }
                                </div>
                                <div class="git-actions">
                                    <button class="btn-git-action" onclick={on_commit.reform(|_| false)}>
                                        { "提交" }
                                    </button>
                                    <button class="btn-git-action amend" onclick={on_commit.reform(|_| true)}>
                                        { "修订" }
                                    </button>
                                    <button class="btn-git-close" onclick={Callback::from(move |_| show_git_panel.set(false))}>
                                        { "✕" }
                                    </button>
                                </div>
                            </div>
                            <div class="git-content">
                                <div class="git-file-list">
                                    if let Some(ref status) = *git_status {
                                        if !status.staged.is_empty() {
                                            <div class="file-section">
                                                <div class="section-title">{ "已暂存" }</div>
                                                { for status.staged.iter().map(|f| {
                                                    let path = f.path.clone();
                                                    let is_selected = (*selected_file).as_ref() == Some(&f.path);
                                                    html! {
                                                        <div
                                                            class={classes!("file-item", "staged", is_selected.then_some("selected"))}
                                                            onclick={on_select_file.reform(move |_| path.clone())}
                                                        >
                                                            <span class="file-status staged">{ "●" }</span>
                                                            <span class="file-name">{ &f.path }</span>
                                                            <span class="file-stats">
                                                                if f.additions > 0 {
                                                                    <span class="additions">{ format!("+{}", f.additions) }</span>
                                                                }
                                                                if f.deletions > 0 {
                                                                    <span class="deletions">{ format!("-{}", f.deletions) }</span>
                                                                }
                                                            </span>
                                                        </div>
                                                    }
                                                })}
                                            </div>
                                        }
                                        if !status.modified.is_empty() {
                                            <div class="file-section">
                                                <div class="section-title">{ "已修改" }</div>
                                                { for status.modified.iter().map(|f| {
                                                    let path = f.path.clone();
                                                    let is_selected = (*selected_file).as_ref() == Some(&f.path);
                                                    html! {
                                                        <div
                                                            class={classes!("file-item", "modified", is_selected.then_some("selected"))}
                                                            onclick={on_select_file.reform(move |_| path.clone())}
                                                        >
                                                            <span class="file-status modified">{ "M" }</span>
                                                            <span class="file-name">{ &f.path }</span>
                                                            <span class="file-stats">
                                                                if f.additions > 0 {
                                                                    <span class="additions">{ format!("+{}", f.additions) }</span>
                                                                }
                                                                if f.deletions > 0 {
                                                                    <span class="deletions">{ format!("-{}", f.deletions) }</span>
                                                                }
                                                            </span>
                                                        </div>
                                                    }
                                                })}
                                            </div>
                                        }
                                        if !status.untracked.is_empty() {
                                            <div class="file-section">
                                                <div class="section-title">{ "未跟踪" }</div>
                                                { for status.untracked.iter().map(|path_str| {
                                                    let path_for_closure = path_str.clone();
                                                    let path_for_display = path_str.clone();
                                                    let is_selected = (*selected_file).as_ref() == Some(path_str);
                                                    html! {
                                                        <div
                                                            class={classes!("file-item", "untracked", is_selected.then_some("selected"))}
                                                            onclick={on_select_file.reform(move |_| path_for_closure.clone())}
                                                        >
                                                            <span class="file-status untracked">{ "?" }</span>
                                                            <span class="file-name">{ path_for_display }</span>
                                                        </div>
                                                    }
                                                })}
                                            </div>
                                        }
                                        if status.staged.is_empty() && status.modified.is_empty() && status.untracked.is_empty() {
                                            <div class="git-empty">
                                                <div class="empty-icon">{ "✨" }</div>
                                                <p>{ "工作目录干净" }</p>
                                            </div>
                                        }
                                    } else {
                                        <div class="git-loading">
                                            <div class="loading-spinner"></div>
                                            <p>{ "加载 Git 状态..." }</p>
                                        </div>
                                    }
                                </div>
                                <div class="git-diff-viewer">
                                    if let Some(ref diff_text) = *file_diff {
                                        <div class="diff-content">
                                            { render_diff_content(diff_text) }
                                        </div>
                                    } else {
                                        <div class="diff-empty">
                                            <p>{ "选择文件查看变更" }</p>
                                        </div>
                                    }
                                </div>
                            </div>
                        </div>
                    }
                    // Virtual keyboard (mobile only)
                    if (*selected_session_id).is_some() {
                        <div class="virtual-keyboard">
                            <button class="vk-btn" onclick={on_key_up}>{ "↑" }</button>
                            <button class="vk-btn" onclick={on_key_down}>{ "↓" }</button>
                            <button class="vk-btn" onclick={on_key_left}>{ "←" }</button>
                            <button class="vk-btn" onclick={on_key_right}>{ "→" }</button>
                            <button class="vk-btn vk-enter" onclick={on_key_enter}>{ "Enter" }</button>
                        </div>
                    }
                </section>
            </main>

            // Context Menu
            if let Some((x, y, session_id, tag)) = (*context_menu).clone() {
                <div
                    class="context-menu"
                    style={format!("left: {}px; top: {}px;", x, y)}
                    onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                >
                    <div class="context-menu-item context-menu-delete"
                        onclick={Callback::from(move |_| {
                            delete_confirm_clone.set(Some((session_id.clone(), tag.clone())));
                            context_menu.set(None);
                        })}
                    >
                        { "🗑 删除会话" }
                    </div>
                </div>
            }

            // Delete Confirmation Modal
            if let Some((ref session_id, ref tag)) = *delete_confirm {
                <div class="modal-overlay" onclick={Callback::from(move |_| delete_confirm_for_overlay.set(None))}>
                    <div class="modal" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <h3>{ "确认删除" }</h3>
                        <p>{ format!("确定要删除会话 '{}' 吗？", tag) }</p>
                        <div class="modal-actions">
                            <button class="btn-cancel" onclick={Callback::from(move |_| delete_confirm_for_cancel.set(None))}>
                                { "取消" }
                            </button>
                            <button class="btn-danger"
                                onclick={{
                                    let on_delete = on_delete_session.clone();
                                    let sid = session_id.clone();
                                    Callback::from(move |_| on_delete.emit(sid.clone()))
                                }}
                            >
                                { "删除" }
                            </button>
                        </div>
                    </div>
                </div>
            }

            // Create Session Modal
            if *show_create_modal {
                <div class="modal-overlay" onclick={Callback::from(move |_| show_create_modal_for_overlay.set(false))}>
                    <div class="modal create-session-modal" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <h3>{ "创建新会话" }</h3>
                        <form onsubmit={on_create_session}>
                            <div class="form-group">
                                <label>{ "选择机器" }</label>
                                <select
                                    class="machine-select"
                                    value={(*create_machine).clone()}
                                    onchange={Callback::from(move |e: Event| {
                                        let input: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                        create_machine.set(input.value());
                                    })}
                                >
                                    <option value="">{ "-- 选择机器 --" }</option>
                                    { for machines.borrow().iter().map(|m| {
                                        html! {
                                            <option value={m.id.clone()}>{ &m.name }</option>
                                        }
                                    })}
                                </select>
                                if machines.borrow().is_empty() {
                                    <small class="form-hint-inline">{ "没有检测到在线机器。请确保在目标机器上运行了 happy daemon。" }</small>
                                }
                            </div>
                            <div class="form-group">
                                <label>{ "工作目录" }</label>
                                <input
                                    type="text"
                                    placeholder="例如: /Users/you/workspace/myproject"
                                    value={(*create_cwd).clone()}
                                    oninput={Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        create_cwd.set(input.value());
                                    })}
                                />
                            </div>
                            <div class="form-group">
                                <label>{ "启动参数 (可选)" }</label>
                                <input
                                    type="text"
                                    placeholder="例如: --model opus"
                                    value={(*create_args).clone()}
                                    oninput={Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        create_args.set(input.value());
                                    })}
                                />
                            </div>
                            <p class="form-hint">
                                { "会话将在选定的机器上启动。确保该机器上运行了 happy daemon。" }
                            </p>
                            <div class="modal-actions">
                                <button type="button" class="btn-cancel"
                                    onclick={Callback::from(move |_| show_create_modal_for_cancel.set(false))}
                                >
                                    { "取消" }
                                </button>
                                <button type="submit" class="btn-primary">
                                    { "创建" }
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            }

            // Commit Modal
            if *show_commit_modal {
                <div class="modal-overlay" onclick={Callback::from(move |_| show_commit_modal_for_modal.set(false))}>
                    <div class="modal" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <h3>{ if is_amend_for_modal { "修订提交 (Amend)" } else { "提交更改" } }</h3>
                        <form onsubmit={on_submit_commit}>
                            <div class="form-group">
                                <label>{ "提交信息" }</label>
                                <textarea
                                    rows="4"
                                    placeholder={ if is_amend_for_modal { "修改提交信息 (留空保持原信息)" } else { "输入提交信息..." } }
                                    value={(*commit_message_for_modal).clone()}
                                    oninput={Callback::from(move |e: InputEvent| {
                                        let input: HtmlTextAreaElement = e.target_unchecked_into();
                                        commit_message_for_input.set(input.value());
                                    })}
                                />
                            </div>
                            if is_amend_for_modal {
                                <p class="form-hint">
                                    { "⚠️ 这将修改最后一次提交。确保还没有推送到远程仓库。" }
                                </p>
                            }
                            <div class="modal-actions">
                                <button type="button" class="btn-cancel"
                                    onclick={Callback::from(move |_| show_commit_modal_for_cancel.set(false))}
                                >
                                    { "取消" }
                                </button>
                                <button type="submit" class="btn-primary"
                                    disabled={!is_amend_for_modal && commit_message.is_empty()}
                                >
                                    { if is_amend_for_modal { "修订" } else { "提交" } }
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            }
        </div>
    }
}
