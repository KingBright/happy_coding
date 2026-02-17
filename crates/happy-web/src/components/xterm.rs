use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, ResizeObserver};
use yew::prelude::*;

/// XTerm terminal component for rendering terminal output
pub struct XTerm {
    terminal: Option<XTermInstance>,
    container_ref: NodeRef,
    /// Track how much content we've already written
    written_len: usize,
    /// Content waiting to be written after terminal initializes
    pending_content: String,
    /// Unique ID for debugging component lifecycle
    debug_id: usize,
    /// Current on_input callback - wrapped in Rc to allow updating
    current_on_input: Rc<RefCell<Callback<Vec<u8>>>>,
    /// Resize observer to handle layout changes
    resize_observer: Option<ResizeObserver>,
    /// Keep closure alive
    _resize_closure: Option<Closure<dyn FnMut(js_sys::Array, ResizeObserver)>>,
    /// Track if user has manually scrolled up (disable auto-scroll)
    user_scrolled_up: bool,
    /// Scroll event closure (kept alive)
    _scroll_closure: Option<Closure<dyn FnMut()>>,
    /// Writer callback stored for debugging
    writer: Option<Callback<Vec<u8>>>,
}

static XTERM_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Properties, PartialEq, Clone)]
pub struct XTermProps {
    /// Unique ID for this terminal instance
    pub id: String,
    /// Content to display (will append new content)
    #[prop_or_default]
    pub initial_content: String,
    /// Callback when user types in terminal
    #[prop_or_default]
    pub on_input: Callback<Vec<u8>>,
    /// Whether terminal is read-only
    #[prop_or_default]
    pub read_only: bool,
    /// Callback to get a function that can send keys to terminal
    #[prop_or_default]
    pub on_key_sender: Callback<Callback<String>>,
    /// Callback to get a function that can write bytes to terminal directly
    #[prop_or_default]
    pub on_writer: Callback<Callback<Vec<u8>>>,
    /// Callback to get a function that can scroll to bottom
    #[prop_or_default]
    pub on_scroll_to_bottom: Callback<Callback<()>>,
    /// Callback when user scrolls up (to show scroll-to-bottom button)
    #[prop_or_default]
    pub on_scroll_state_change: Callback<bool>, // true = scrolled up, false = at bottom
}

pub enum XTermMsg {
    Initialize,
    Write(String),
    WriteBytes(Vec<u8>),
    Clear,
    ScrollToBottom,
    UserScrolled(bool), // Track if user manually scrolled up
}

impl Component for XTerm {
    type Message = XTermMsg;
    type Properties = XTermProps;

    fn create(ctx: &Context<Self>) -> Self {
        let debug_id = XTERM_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        log::info!(
            "[XTerm#{}] create() called, initial_content len={}",
            debug_id,
            ctx.props().initial_content.len()
        );
        // Schedule initialization after mount
        ctx.link().send_message(XTermMsg::Initialize);
        // Store initial on_input callback in RefCell
        let current_on_input = Rc::new(RefCell::new(ctx.props().on_input.clone()));
        Self {
            terminal: None,
            container_ref: NodeRef::default(),
            written_len: 0,
            pending_content: String::new(),
            debug_id,
            current_on_input,
            resize_observer: None,
            _resize_closure: None,
            user_scrolled_up: false,
            _scroll_closure: None,
            writer: None,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        // Check if id changed (session switch)
        if ctx.props().id != old_props.id {
            log::info!(
                "[XTerm#{}] Session ID changed from '{}' to '{}'",
                self.debug_id,
                old_props.id,
                ctx.props().id
            );
            // Session switched - need to clear and write new content
            let content = &ctx.props().initial_content;
            log::info!("[XTerm#{}] Session change - initial_content len={}, terminal ready={}",
                self.debug_id, content.len(), self.terminal.is_some());

            if let Some(term) = &self.terminal {
                term.clear();
                if !content.is_empty() {
                    log::info!("[XTerm#{}] Writing {} bytes from initial_content", self.debug_id, content.len());
                    term.write(content);
                    self.written_len = content.len();
                } else {
                    log::warn!("[XTerm#{}] initial_content is empty!", self.debug_id);
                }
                // Reset scroll tracking for new session
                self.user_scrolled_up = false;
                term.scroll_to_bottom();

                // Re-register the writer callback for the new session
                // This is crucial because the parent component's terminal_writer reference
                // needs to point to this new XTerm instance
                let link = ctx.link().clone();
                let writer = Callback::from(move |bytes: Vec<u8>| {
                    link.send_message(XTermMsg::WriteBytes(bytes));
                });
                ctx.props().on_writer.emit(writer.clone());
                self.writer = Some(writer);
                log::info!("[XTerm#{}] Writer callback re-registered after session change", self.debug_id);
            } else {
                log::warn!("[XTerm#{}] Terminal not ready during session change, storing in pending_content", self.debug_id);
                self.pending_content = content.clone();
            }
            // Update callback
            *self.current_on_input.borrow_mut() = ctx.props().on_input.clone();
            return false;
        }

        // ALWAYS update the callback in RefCell to ensure we have the latest closure
        *self.current_on_input.borrow_mut() = ctx.props().on_input.clone();

        // Check if initial_content changed (e.g., terminal_history arrived after init)
        if ctx.props().initial_content != old_props.initial_content {
            let new_content = &ctx.props().initial_content;
            let old_content = &old_props.initial_content;
            log::info!("[XTerm#{}] initial_content changed: old_len={}, new_len={}",
                self.debug_id, old_content.len(), new_content.len());

            if let Some(term) = &self.terminal {
                // If new content is longer, append the difference
                if new_content.len() > old_content.len() {
                    let to_write = if old_content.is_empty() {
                        new_content.as_str()
                    } else {
                        &new_content[old_content.len()..]
                    };
                    if !to_write.is_empty() {
                        log::info!("[XTerm#{}] Appending {} bytes from initial_content change", self.debug_id, to_write.len());
                        term.write(to_write);
                        self.written_len = new_content.len();
                    }
                    // Scroll to bottom after adding content
                    if !self.user_scrolled_up {
                        term.scroll_to_bottom();
                    }
                } else if new_content.len() < old_content.len() {
                    // Content was reset (e.g., new session)
                    log::info!("[XTerm#{}] Content shrunk, clearing and rewriting", self.debug_id);
                    term.clear();
                    if !new_content.is_empty() {
                        term.write(new_content);
                        self.written_len = new_content.len();
                    }
                    // Scroll to bottom after rewriting
                    self.user_scrolled_up = false;
                    term.scroll_to_bottom();
                }
            } else {
                // Terminal not ready, store in pending
                self.pending_content = new_content.clone();
            }
        }

        false // Don't re-render, we've updated the terminal directly
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            XTermMsg::Initialize => {
                log::info!("[XTerm#{}] Initialize message received", self.debug_id);
                if self.terminal.is_none() {
                    if let Some(element) = self.container_ref.cast::<HtmlElement>() {
                        let id = &ctx.props().id;
                        match XTermInstance::new(&element, id) {
                            Ok(term) => {
                                log::info!(
                                    "[XTerm#{}] Terminal instance created successfully",
                                    self.debug_id
                                );
                                // Set up input handler using current_on_input RefCell
                                if !ctx.props().read_only {
                                    let on_input = self.current_on_input.clone();
                                    term.on_data(move |data: String| {
                                        let cb = on_input.borrow();
                                        cb.emit(data.as_bytes().to_vec());
                                    });
                                }

                                // Write initial content if any
                                let content = &ctx.props().initial_content;
                                if !content.is_empty() {
                                    log::info!(
                                        "[XTerm#{}] Writing {} initial bytes",
                                        self.debug_id,
                                        content.len()
                                    );
                                    term.write(content);
                                    self.written_len = content.len();
                                }

                                // Also write any pending content that arrived before init
                                if !self.pending_content.is_empty()
                                    && self.pending_content.len() > self.written_len
                                {
                                    let pending = &self.pending_content[self.written_len..];
                                    if !pending.is_empty() {
                                        log::info!(
                                            "[XTerm#{}] Writing {} pending bytes after init",
                                            self.debug_id,
                                            pending.len()
                                        );
                                        term.write(pending);
                                        self.written_len = self.pending_content.len();
                                    }
                                }

                                // Set up key sender callback
                                let term_clone = term.clone();
                                let key_sender = Callback::from(move |key: String| {
                                    term_clone.send_key(&key);
                                });
                                ctx.props().on_key_sender.emit(key_sender);

                                // Set up writer callback for direct incremental writes
                                let link = ctx.link().clone();
                                let writer = Callback::from(move |bytes: Vec<u8>| {
                                    log::info!("[XTerm] Writer callback invoked with {} bytes", bytes.len());
                                    link.send_message(XTermMsg::WriteBytes(bytes));
                                });

                                // Clone the callback before moving into emit
                                let writer_for_storage = writer.clone();
                                ctx.props().on_writer.emit(writer);

                                // Store the writer in the component so we can use it for initial content
                                self.writer = Some(writer_for_storage);

                                log::info!("[XTerm#{}] Writer callback registered and stored", self.debug_id);

                                // Set up scroll-to-bottom callback
                                let scroll_term = term.clone();
                                let scroll_to_bottom_cb = Callback::from(move |_| {
                                    scroll_term.scroll_to_bottom();
                                });
                                ctx.props().on_scroll_to_bottom.emit(scroll_to_bottom_cb);

                                // Set up scroll event listener to detect user scrolling
                                let scroll_link = ctx.link().clone();
                                let scroll_term = term.clone();
                                let scroll_state_cb = ctx.props().on_scroll_state_change.clone();
                                let scroll_cb = Closure::wrap(Box::new(move || {
                                    // Check if user scrolled up (not at bottom)
                                    let is_at_bottom = scroll_term.is_at_bottom();
                                    scroll_link.send_message(XTermMsg::UserScrolled(!is_at_bottom));
                                    // Notify parent component about scroll state
                                    scroll_state_cb.emit(!is_at_bottom);
                                }) as Box<dyn FnMut()>);

                                if let Ok(on_scroll_method) = js_sys::Reflect::get(
                                    &term.terminal,
                                    &JsValue::from_str("onScroll"),
                                )
                                .and_then(|m| m.dyn_into::<js_sys::Function>())
                                {
                                    let _ = on_scroll_method.call1(
                                        &term.terminal,
                                        scroll_cb.as_ref().unchecked_ref(),
                                    );
                                }
                                self._scroll_closure = Some(scroll_cb);

                                self.terminal = Some(term);
                                return true;
                            }
                            Err(e) => {
                                log::error!(
                                    "[XTerm#{}] Failed to create terminal: {:?}",
                                    self.debug_id,
                                    e
                                );
                            }
                        }
                    } else {
                        log::warn!("[XTerm#{}] Container element not found yet", self.debug_id);
                    }
                }
                false
            }
            XTermMsg::Write(text) => {
                if let Some(term) = &self.terminal {
                    term.write(&text);
                }
                false
            }
            XTermMsg::WriteBytes(bytes) => {
                log::info!("[XTerm#{}] WriteBytes received: {} bytes, terminal ready: {}", self.debug_id, bytes.len(), self.terminal.is_some());
                if let Some(term) = &self.terminal {
                    // Use write_bytes to preserve binary data
                    term.write_bytes(&bytes);
                    log::info!("[XTerm#{}] WriteBytes written successfully", self.debug_id);
                    // Only auto-scroll if user hasn't manually scrolled up
                    if !self.user_scrolled_up {
                        term.scroll_to_bottom();
                    }
                } else {
                    log::warn!("[XTerm#{}] WriteBytes: terminal not ready, data lost!", self.debug_id);
                }
                false
            }
            XTermMsg::Clear => {
                if let Some(term) = &self.terminal {
                    term.clear();
                }
                false
            }
            XTermMsg::ScrollToBottom => {
                if let Some(term) = &self.terminal {
                    term.scroll_to_bottom();
                }
                false
            }
            XTermMsg::UserScrolled(scrolled_up) => {
                self.user_scrolled_up = scrolled_up;
                if scrolled_up {
                    log::debug!("[XTerm#{}] User scrolled up - disabling auto-scroll", self.debug_id);
                } else {
                    log::debug!("[XTerm#{}] User scrolled to bottom - enabling auto-scroll", self.debug_id);
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div
                ref={self.container_ref.clone()}
                id={ctx.props().id.clone()}
                class="xterm-container"
            />
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, first_render: bool) {
        if first_render {
            // Setup ResizeObserver to fit terminal when container size changes
            if let Some(terminal_instance) = &self.terminal {
                let terminal_clone = terminal_instance.clone();
                let on_resize = Closure::wrap(Box::new(
                    move |_entries: js_sys::Array, _observer: ResizeObserver| {
                        terminal_clone.fit();
                        terminal_clone.scroll_to_bottom();
                    },
                )
                    as Box<dyn FnMut(js_sys::Array, ResizeObserver)>);

                if let Ok(observer) = ResizeObserver::new(on_resize.as_ref().unchecked_ref()) {
                    if let Some(element) = self.container_ref.cast::<HtmlElement>() {
                        observer.observe(&element);
                        self.resize_observer = Some(observer);
                        self._resize_closure = Some(on_resize);
                        log::info!("[XTerm#{}] ResizeObserver attached", self.debug_id);
                    }
                } else {
                    log::error!("[XTerm#{}] Failed to create ResizeObserver", self.debug_id);
                }

                // Schedule delayed fit() calls to ensure layout has settled
                // The container may not have its final dimensions immediately
                let term_50 = terminal_instance.clone();
                let term_200 = terminal_instance.clone();
                let term_500 = terminal_instance.clone();

                if let Some(window) = web_sys::window() {
                    // Fit after a short delay (50ms - layout usually settled)
                    let cb_50 = Closure::once(Box::new(move || {
                        term_50.fit();
                        term_50.scroll_to_bottom();
                    }) as Box<dyn FnOnce()>);
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb_50.as_ref().unchecked_ref(),
                        50,
                    );
                    cb_50.forget();

                    // Fit after medium delay (200ms - for slow devices)
                    let cb_200 = Closure::once(Box::new(move || {
                        term_200.fit();
                        term_200.scroll_to_bottom();
                    }) as Box<dyn FnOnce()>);
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb_200.as_ref().unchecked_ref(),
                        200,
                    );
                    cb_200.forget();

                    // Fit after longer delay (500ms - final safety net)
                    let cb_500 = Closure::once(Box::new(move || {
                        term_500.fit();
                        term_500.scroll_to_bottom();
                    }) as Box<dyn FnOnce()>);
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb_500.as_ref().unchecked_ref(),
                        500,
                    );
                    cb_500.forget();
                }
            }
        }
    }
}

impl Drop for XTerm {
    fn drop(&mut self) {
        log::info!(
            "[XTerm#{}] drop() called - component being destroyed",
            self.debug_id
        );
    }
}

/// Wrapper around xterm.js Terminal instance
#[derive(Clone)]
pub struct XTermInstance {
    terminal: JsValue,
    fit_addon: JsValue,
}

impl XTermInstance {
    pub fn new(container: &HtmlElement, _id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().unwrap();
        let xterm = js_sys::Reflect::get(&window, &JsValue::from_str("Terminal"))?;

        // Create terminal options
        let opts = js_sys::Object::new();
        js_sys::Reflect::set(&opts, &JsValue::from_str("theme"), &Self::get_theme())?;
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("fontFamily"),
            &JsValue::from_str("'JetBrains Mono', 'Fira Code', 'SF Mono', monospace"),
        )?;
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("fontSize"),
            &JsValue::from_f64(14.0),
        )?;
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("lineHeight"),
            &JsValue::from_f64(1.2),
        )?;
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("cursorBlink"),
            &JsValue::from_bool(false),
        )?;
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("cursorStyle"),
            &JsValue::from_str("block"),
        )?;
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("scrollback"),
            &JsValue::from_f64(10000.0),
        )?;
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("convertEol"),
            &JsValue::from_bool(true), // Convert \n to \r\n
        )?;

        // Create terminal instance
        let terminal = js_sys::Reflect::construct(
            &xterm.dyn_into::<js_sys::Function>()?,
            &js_sys::Array::of1(&opts),
        )?;

        // Open terminal in container
        let open_method = js_sys::Reflect::get(&terminal, &JsValue::from_str("open"))?
            .dyn_into::<js_sys::Function>()?;
        open_method.call1(&terminal, container)?;

        // Load FitAddon and store it for reuse
        let fit_addon = Self::get_fit_addon(&terminal)?;

        // Initial fit
        if let Ok(fit_method) = js_sys::Reflect::get(&fit_addon, &JsValue::from_str("fit"))
            .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = fit_method.call0(&fit_addon);
        }

        Ok(Self {
            terminal,
            fit_addon,
        })
    }

    fn get_theme() -> JsValue {
        let theme = js_sys::Object::new();

        let colors = js_sys::Object::new();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("background"),
            &JsValue::from_str("#0d1117"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("foreground"),
            &JsValue::from_str("#c9d1d9"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("cursor"),
            &JsValue::from_str("#58a6ff"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("cursorAccent"),
            &JsValue::from_str("#0d1117"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("selectionBackground"),
            &JsValue::from_str("#264f78"),
        )
        .unwrap();

        // ANSI colors
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("black"),
            &JsValue::from_str("#484f58"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("red"),
            &JsValue::from_str("#ff7b72"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("green"),
            &JsValue::from_str("#3fb950"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("yellow"),
            &JsValue::from_str("#d29922"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("blue"),
            &JsValue::from_str("#58a6ff"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("magenta"),
            &JsValue::from_str("#bc8cff"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("cyan"),
            &JsValue::from_str("#39c5cf"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("white"),
            &JsValue::from_str("#b1bac4"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightBlack"),
            &JsValue::from_str("#6e7681"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightRed"),
            &JsValue::from_str("#ffa198"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightGreen"),
            &JsValue::from_str("#56d364"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightYellow"),
            &JsValue::from_str("#e3b341"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightBlue"),
            &JsValue::from_str("#79c0ff"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightMagenta"),
            &JsValue::from_str("#d2a8ff"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightCyan"),
            &JsValue::from_str("#56d4dd"),
        )
        .unwrap();
        js_sys::Reflect::set(
            &colors,
            &JsValue::from_str("brightWhite"),
            &JsValue::from_str("#f0f6fc"),
        )
        .unwrap();

        js_sys::Reflect::set(&theme, &JsValue::from_str("colors"), &colors).unwrap();

        theme.into()
    }

    fn get_fit_addon(terminal: &JsValue) -> Result<JsValue, JsValue> {
        let window = web_sys::window().unwrap();
        let fit = js_sys::Reflect::get(&window, &JsValue::from_str("FitAddon"))?;
        let fit_class = js_sys::Reflect::get(&fit, &JsValue::from_str("FitAddon"))?;

        let addon = js_sys::Reflect::construct(
            &fit_class.dyn_into::<js_sys::Function>()?,
            &js_sys::Array::new(),
        )?;

        // Load addon into terminal
        let load_method = js_sys::Reflect::get(terminal, &JsValue::from_str("loadAddon"))?
            .dyn_into::<js_sys::Function>()?;
        load_method.call1(terminal, &addon)?;

        Ok(addon)
    }

    pub fn write(&self, data: &str) {
        if let Ok(write_method) = js_sys::Reflect::get(&self.terminal, &JsValue::from_str("write"))
            .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = write_method.call1(&self.terminal, &JsValue::from_str(data));
        }
    }

    pub fn write_bytes(&self, data: &[u8]) {
        // Create a Uint8Array from the bytes and pass directly to xterm.js
        // This preserves binary data better than string conversion
        let uint8_array = js_sys::Uint8Array::from(data);
        if let Ok(write_method) = js_sys::Reflect::get(&self.terminal, &JsValue::from_str("write"))
            .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = write_method.call1(&self.terminal, &uint8_array);
        }
    }

    pub fn clear(&self) {
        if let Ok(clear_method) = js_sys::Reflect::get(&self.terminal, &JsValue::from_str("clear"))
            .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = clear_method.call0(&self.terminal);
        }
    }

    pub fn on_data<F>(&self, mut callback: F)
    where
        F: FnMut(String) + 'static,
    {
        let cb = Closure::wrap(Box::new(move |data: String| {
            callback(data);
        }) as Box<dyn FnMut(String)>);

        let on_data_method = js_sys::Reflect::get(&self.terminal, &JsValue::from_str("onData"))
            .and_then(|m| m.dyn_into::<js_sys::Function>());

        if let Ok(method) = on_data_method {
            let _ = method.call1(&self.terminal, cb.as_ref().unchecked_ref());
        }

        // Note: We can't clean up old callbacks in xterm.js since it doesn't provide
        // a way to remove handlers. The fix is to avoid calling on_data multiple times
        // - instead use a wrapper that dispatches to the current session's callback.
        cb.forget();
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Ok(resize_method) =
            js_sys::Reflect::get(&self.terminal, &JsValue::from_str("resize"))
                .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = resize_method.call2(
                &self.terminal,
                &JsValue::from_f64(cols as f64),
                &JsValue::from_f64(rows as f64),
            );
        }
    }

    pub fn fit(&self) {
        // Use the stored FitAddon instance
        if let Ok(fit_method) = js_sys::Reflect::get(&self.fit_addon, &JsValue::from_str("fit"))
            .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = fit_method.call0(&self.fit_addon);
        }
    }

    /// Send a key sequence to the terminal (for virtual keyboard)
    pub fn send_key(&self, key: &str) {
        // xterm.js uses input() method to simulate key input
        if let Ok(input_method) = js_sys::Reflect::get(&self.terminal, &JsValue::from_str("input"))
            .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = input_method.call1(&self.terminal, &JsValue::from_str(key));
        }
    }

    /// Scroll to the bottom of the terminal
    pub fn scroll_to_bottom(&self) {
        if let Ok(scroll_method) = js_sys::Reflect::get(&self.terminal, &JsValue::from_str("scrollToBottom"))
            .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = scroll_method.call0(&self.terminal);
        }
    }

    /// Check if the viewport is at the bottom (for smart scroll)
    pub fn is_at_bottom(&self) -> bool {
        // Get the buffer
        if let Ok(buffer) = js_sys::Reflect::get(&self.terminal, &JsValue::from_str("buffer")) {
            if let Ok(active) = js_sys::Reflect::get(&buffer, &JsValue::from_str("active")) {
                // Get viewport position
                if let Ok(viewport_y) = js_sys::Reflect::get(&active, &JsValue::from_str("viewportY")) {
                    if let Ok(base_y) = js_sys::Reflect::get(&active, &JsValue::from_str("baseY")) {
                        let viewport_y = viewport_y.as_f64().unwrap_or(0.0) as i32;
                        let base_y = base_y.as_f64().unwrap_or(0.0) as i32;
                        // If viewportY equals or is very close to baseY, we're at the bottom
                        return viewport_y >= base_y - 1;
                    }
                }
            }
        }
        true // Default to true (at bottom) if we can't determine
    }
}

impl Drop for XTermInstance {
    fn drop(&mut self) {
        // Dispose terminal
        if let Ok(dispose_method) =
            js_sys::Reflect::get(&self.terminal, &JsValue::from_str("dispose"))
                .and_then(|m| m.dyn_into::<js_sys::Function>())
        {
            let _ = dispose_method.call0(&self.terminal);
        }
    }
}
