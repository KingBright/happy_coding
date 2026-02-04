//! Platform adapters for Happy Coding

mod antigravity;
mod claude;
mod codex;

pub use antigravity::AntigravityAdapter;
pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;

use happy_core::{Adapter, AdapterFactory, Platform};

/// Create an adapter factory with all platform adapters registered
pub fn create_adapter_factory() -> AdapterFactory {
    let mut factory = AdapterFactory::new();
    factory.register(Box::new(ClaudeAdapter::new()));
    factory.register(Box::new(CodexAdapter::new()));
    factory.register(Box::new(AntigravityAdapter::new()));
    factory
}

/// Get an adapter for a specific platform
pub fn get_adapter(platform: Platform) -> Box<dyn Adapter> {
    match platform {
        Platform::Claude => Box::new(ClaudeAdapter::new()),
        Platform::Codex => Box::new(CodexAdapter::new()),
        Platform::Antigravity => Box::new(AntigravityAdapter::new()),
    }
}
