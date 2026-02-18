//! Log Viewer Component
//!
//! A floating console log viewer with filtering and copy functionality.

use yew::prelude::*;
use crate::utils::logger::{LogLevel, LogEntry, get_logs, clear_logs, copy_logs_to_clipboard, get_logs_filtered};
use web_sys::HtmlElement;
use wasm_bindgen::JsValue;

#[derive(Properties, PartialEq, Clone)]
pub struct LogViewerProps {
    /// Whether the viewer is visible
    #[prop_or(false)]
    pub visible: bool,
    /// Callback when close button is clicked
    #[prop_or_default]
    pub on_close: Callback<()>,
}

pub enum LogViewerMsg {
    Toggle,
    Close,
    Refresh,
    Clear,
    Copy,
    SetFilter(LogLevel),
    ToggleLevel(LogLevel),
    ScrollToBottom,
}

pub struct LogViewer {
    logs: Vec<LogEntry>,
    filter: LogLevel,
    auto_scroll: bool,
    log_container_ref: NodeRef,
    visible: bool,
}

impl Component for LogViewer {
    type Message = LogViewerMsg;
    type Properties = LogViewerProps;

    fn create(ctx: &Context<Self>) -> Self {
        let logs = get_logs();
        Self {
            logs,
            filter: LogLevel::Debug,
            auto_scroll: true,
            log_container_ref: NodeRef::default(),
            visible: ctx.props().visible,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().visible != old_props.visible {
            self.visible = ctx.props().visible;
            return true;
        }
        false
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            LogViewerMsg::Toggle => {
                self.visible = !self.visible;
                if self.visible {
                    self.logs = get_logs();
                }
                true
            }
            LogViewerMsg::Close => {
                self.visible = false;
                ctx.props().on_close.emit(());
                true
            }
            LogViewerMsg::Refresh => {
                self.logs = if self.filter == LogLevel::Debug {
                    get_logs()
                } else {
                    get_logs_filtered(self.filter)
                };
                self.auto_scroll = true;
                true
            }
            LogViewerMsg::Clear => {
                clear_logs();
                self.logs.clear();
                true
            }
            LogViewerMsg::Copy => {
                if let Err(e) = copy_logs_to_clipboard() {
                    log::error!("Failed to copy logs: {}", e);
                } else {
                    log::info!("Logs copied to clipboard");
                }
                false
            }
            LogViewerMsg::SetFilter(level) => {
                self.filter = level;
                self.logs = if self.filter == LogLevel::Debug {
                    get_logs()
                } else {
                    get_logs_filtered(self.filter)
                };
                true
            }
            LogViewerMsg::ToggleLevel(level) => {
                // Toggle between showing this level and above vs just this level and above
                if self.filter == level {
                    self.filter = LogLevel::Debug;
                } else {
                    self.filter = level;
                }
                self.logs = if self.filter == LogLevel::Debug {
                    get_logs()
                } else {
                    get_logs_filtered(self.filter)
                };
                true
            }
            LogViewerMsg::ScrollToBottom => {
                if let Some(container) = self.log_container_ref.cast::<HtmlElement>() {
                    container.set_scroll_top(container.scroll_height());
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if !self.visible {
            return html! {};
        }

        let filtered_logs = &self.logs;
        let log_count = filtered_logs.len();

        html! {
            <div class="log-viewer-container">
                <div class="log-viewer-header">
                    <div class="log-viewer-title">
                        {"📋 Console Logs"}
                        <span class="log-count">{format!("({})", log_count)}</span>
                    </div>
                    <div class="log-viewer-controls">
                        // Level filters
                        <div class="log-level-filters">
                            {self.render_level_filter(ctx, LogLevel::Debug, "DEBUG")}
                            {self.render_level_filter(ctx, LogLevel::Info, "INFO")}
                            {self.render_level_filter(ctx, LogLevel::Warn, "WARN")}
                            {self.render_level_filter(ctx, LogLevel::Error, "ERROR")}
                        </div>
                        <button
                            class="log-btn log-btn-refresh"
                            onclick={ctx.link().callback(|_| LogViewerMsg::Refresh)}
                            title="Refresh"
                        >
                            {"🔄"}
                        </button>
                        <button
                            class="log-btn log-btn-clear"
                            onclick={ctx.link().callback(|_| LogViewerMsg::Clear)}
                            title="Clear"
                        >
                            {"🗑️"}
                        </button>
                        <button
                            class="log-btn log-btn-copy"
                            onclick={ctx.link().callback(|_| LogViewerMsg::Copy)}
                            title="Copy All"
                        >
                            {"📋 Copy"}
                        </button>
                        <button
                            class="log-btn log-btn-close"
                            onclick={ctx.link().callback(|_| LogViewerMsg::Close)}
                            title="Close"
                        >
                            {"✕"}
                        </button>
                    </div>
                </div>
                <div
                    class="log-viewer-content"
                    ref={self.log_container_ref.clone()}
                >
                    {if filtered_logs.is_empty() {
                        html! {
                            <div class="log-empty">{"No logs to display"}</div>
                        }
                    } else {
                        html! {
                            <>
                                {filtered_logs.iter().map(|entry| {
                                    self.render_log_entry(entry)
                                }).collect::<Html>()}
                            </>
                        }
                    }}
                </div>
                <div class="log-viewer-footer">
                    <span class="log-hint">{"Auto-scroll enabled"}</span>
                    <button
                        class="log-btn log-btn-scroll-bottom"
                        onclick={ctx.link().callback(|_| LogViewerMsg::ScrollToBottom)}
                    >
                        {"⬇ Scroll to Bottom"}
                    </button>
                </div>
            </div>
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        if self.auto_scroll {
            if let Some(container) = self.log_container_ref.cast::<HtmlElement>() {
                container.set_scroll_top(container.scroll_height());
            }
            self.auto_scroll = false;
        }
    }
}

impl LogViewer {
    fn render_level_filter(&self, ctx: &Context<Self>, level: LogLevel, label: &str) -> Html {
        let is_active = self.filter == level;
        let class = if is_active {
            format!("log-level-btn active {}", level.as_str().to_lowercase())
        } else {
            format!("log-level-btn {}", level.as_str().to_lowercase())
        };

        html! {
            <button
                class={class}
                onclick={ctx.link().callback(move |_| LogViewerMsg::ToggleLevel(level))}
            >
                {label}
            </button>
        }
    }

    fn render_log_entry(&self, entry: &LogEntry) -> Html {
        let time = js_sys::Date::new(&JsValue::from_f64(entry.timestamp));
        let time_str = format!(
            "{:02}:{:02}:{:02}",
            time.get_hours(),
            time.get_minutes(),
            time.get_seconds()
        );

        let level_class = format!("log-level-{}" , entry.level.as_str().to_lowercase());
        let level_color = entry.level.color();

        html! {
            <div class={classes!("log-entry", level_class)}>
                <span class="log-timestamp">{time_str}</span>
                <span
                    class="log-level-badge"
                    style={format!("background-color: {}", level_color)}
                >
                    {entry.level.as_str()}
                </span>
                <span class="log-message">{&entry.message}</span>
            </div>
        }
    }
}

/// Hook to auto-refresh log viewer - use this in your component
#[hook]
pub fn use_log_refresh() -> Vec<LogEntry> {
    let logs = use_state(get_logs);
    {
        let logs = logs.clone();
        use_effect_with((), move |_| {
            let interval = gloo_timers::callback::Interval::new(1000, move || {
                logs.set(get_logs());
            });
            || drop(interval)
        });
    }
    (*logs).clone()
}
