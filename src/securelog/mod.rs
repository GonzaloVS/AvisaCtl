pub mod logger;
mod entry;
mod file;
mod integrity;
mod crypto;

// Re-exports para facilitar el uso desde otros módulos
pub use file::read_secure_log_formatted;
pub use integrity::validate_secure_log_integrity;
