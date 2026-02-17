//! Port traits (interfaces) for dependency injection

pub mod encryption;
pub mod storage;
pub mod terminal;

pub use encryption::EncryptionPort;
pub use storage::{MachineStore, SessionStore, UserStore};
pub use terminal::{TerminalBackend, TerminalSession};
