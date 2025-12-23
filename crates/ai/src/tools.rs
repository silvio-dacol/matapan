pub enum ToolResult {
    Json(serde_json::Value),
    Text(String),
}

pub trait Tool {
    fn name(&self) -> &str;
    fn call(&self, input: serde_json::Value) -> anyhow::Result<ToolResult>;
}
