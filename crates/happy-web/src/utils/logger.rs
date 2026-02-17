//! Console Log Capture and Viewer
//!
//! Captures all console logs and stores them for in-app viewing.

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug = 0,
    Log = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Log => "LOG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            LogLevel::Debug => "#6e7681",
            LogLevel::Log => "#c9d1d9",
            LogLevel::Info => "#58a6ff",
            LogLevel::Warn => "#d29922",
            LogLevel::Error => "#ff7b72",
        }
    }
}

/// A single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: f64,
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
}

/// Log store
pub struct LogStore {
    entries: Vec<LogEntry>,
    max_entries: usize,
}

impl LogStore {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn filter_by_level(&self, min_level: LogLevel) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.level >= min_level)
            .collect()
    }

    pub fn to_string(&self) -> String {
        self.entries
            .iter()
            .map(|e| {
                let time = js_sys::Date::new(&JsValue::from_f64(e.timestamp));
                let time_str = format!(
                    "{:02}:{:02}:{:02}.{:03}",
                    time.get_hours(),
                    time.get_minutes(),
                    time.get_seconds(),
                    time.get_milliseconds()
                );
                format!("[{}] [{}] {}", time_str, e.level.as_str(), e.message)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

thread_local! {
    static LOG_STORE: RefCell<LogStore> = RefCell::new(LogStore::new(1000));
    static ORIGINAL_CONSOLE: RefCell<Option<js_sys::Object>> = RefCell::new(None);
}

/// Add a log entry
pub fn add_log(level: LogLevel, message: impl Into<String>) {
    let entry = LogEntry {
        timestamp: js_sys::Date::now(),
        level,
        message: message.into(),
        source: None,
    };
    LOG_STORE.with(|store| {
        store.borrow_mut().push(entry);
    });
}

/// Get all log entries
pub fn get_logs() -> Vec<LogEntry> {
    LOG_STORE.with(|store| store.borrow().entries().to_vec())
}

/// Get logs filtered by minimum level
pub fn get_logs_filtered(min_level: LogLevel) -> Vec<LogEntry> {
    LOG_STORE.with(|store| {
        store
            .borrow()
            .filter_by_level(min_level)
            .into_iter()
            .cloned()
            .collect()
    })
}

/// Clear all logs
pub fn clear_logs() {
    LOG_STORE.with(|store| store.borrow_mut().clear());
}

/// Copy logs to clipboard using JavaScript interop
pub fn copy_logs_to_clipboard() -> Result<(), String> {
    let logs = LOG_STORE.with(|store| store.borrow().to_string());

    // Use js_sys::eval to execute copy command
    let js_code = format!(
        r#"(function() {{
            const text = {:?};
            const textarea = document.createElement('textarea');
            textarea.value = text;
            textarea.style.position = 'fixed';
            textarea.style.opacity = '0';
            document.body.appendChild(textarea);
            textarea.select();
            const result = document.execCommand('copy');
            document.body.removeChild(textarea);
            return result;
        }})()"#,
        logs
    );

    let result = js_sys::eval(&js_code)
        .map_err(|_| "Failed to execute copy script")?;

    if result.as_bool().unwrap_or(false) {
        Ok(())
    } else {
        Err("Copy command failed".to_string())
    }
}

/// Get logs as formatted string
pub fn get_logs_as_string() -> String {
    LOG_STORE.with(|store| store.borrow().to_string())
}

/// Initialize console log capture by injecting a JavaScript hook
pub fn init_console_capture() {
    // Clear existing logs
    clear_logs();

    // Store original console reference
    let window = web_sys::window().expect("no window");
    let console_obj = js_sys::Reflect::get(&window, &"console".into())
        .expect("no console")
        .dyn_into::<js_sys::Object>()
        .expect("console not an object");

    ORIGINAL_CONSOLE.with(|orig| {
        *orig.borrow_mut() = Some(console_obj.clone());
    });

    // Create hook functions using JavaScript
    let js_hook = r#"
        (function() {
            // Store reference to Rust log function
            const rustLog = window.__rust_log || function(level, message) {
                console.log('[RUST_LOG]', level, message);
            };

            const originalConsole = window.console;
            const originalLog = originalConsole.log;
            const originalInfo = originalConsole.info;
            const originalWarn = originalConsole.warn;
            const originalError = originalConsole.error;
            const originalDebug = originalConsole.debug;

            function formatArgs(args) {
                return args.map(arg => {
                    if (typeof arg === 'string') return arg;
                    try {
                        return JSON.stringify(arg);
                    } catch (e) {
                        return String(arg);
                    }
                }).join(' ');
            }

            window.console.log = function(...args) {
                rustLog('LOG', formatArgs(args));
                originalLog.apply(originalConsole, args);
            };

            window.console.info = function(...args) {
                rustLog('INFO', formatArgs(args));
                originalInfo.apply(originalConsole, args);
            };

            window.console.warn = function(...args) {
                rustLog('WARN', formatArgs(args));
                originalWarn.apply(originalConsole, args);
            };

            window.console.error = function(...args) {
                rustLog('ERROR', formatArgs(args));
                originalError.apply(originalConsole, args);
            };

            window.console.debug = function(...args) {
                rustLog('DEBUG', formatArgs(args));
                originalDebug.apply(originalConsole, args);
            };

            return true;
        })()
    "#;

    // Set up the Rust callback that JS will call
    let rust_log_callback = Closure::wrap(Box::new(|level: String, message: String| {
        let log_level = match level.as_str() {
            "DEBUG" => LogLevel::Debug,
            "INFO" => LogLevel::Info,
            "WARN" => LogLevel::Warn,
            "ERROR" => LogLevel::Error,
            _ => LogLevel::Log,
        };
        add_log(log_level, message);
    }) as Box<dyn FnMut(String, String)>);

    // Expose the callback to JavaScript
    js_sys::Reflect::set(
        &window,
        &"__rust_log".into(),
        rust_log_callback.as_ref()
    ).unwrap();

    // Forget the closure to keep it alive
    rust_log_callback.forget();

    // Execute the JS hook
    let result = js_sys::eval(js_hook);
    match result {
        Ok(_) => {
            add_log(LogLevel::Info, "Console log capture initialized");
        }
        Err(e) => {
            web_sys::console::error_1(&format!("Failed to init console capture: {:?}", e).into());
        }
    }
}

/// Macro to log at different levels
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::utils::logger::add_log($crate::utils::logger::LogLevel::Debug, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::utils::logger::add_log($crate::utils::logger::LogLevel::Info, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::utils::logger::add_log($crate::utils::logger::LogLevel::Warn, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::utils::logger::add_log($crate::utils::logger::LogLevel::Error, format!($($arg)*))
    };
}
