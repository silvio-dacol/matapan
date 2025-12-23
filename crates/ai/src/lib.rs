pub mod ollama;
pub mod orchestrator;
pub mod tools;

// Re export the important bits
pub use orchestrator::run;
pub use tools::{Tool, ToolResult};
