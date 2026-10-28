use serde_json::Value;
use std::sync::OnceLock;

static CONTEXT: OnceLock<Value> = OnceLock::new();

pub fn initialize(value: Value) -> Result<(), String> {
    CONTEXT
        .set(value)
        .map_err(|_| "Account extension already initialized".into())
}

pub fn context() -> &'static Value {
    CONTEXT.get_or_init(|| Value::Null)
}
pub fn get_tool_exe_path(id: &str) -> Option<String> {
    context()[id]["executable"].as_str().map(str::to_owned)
}
pub fn get_tool_launch_uri(id: &str) -> Option<String> {
    context()[id]["launchUri"].as_str().map(str::to_owned)
}
pub fn doubao_desktop_metadata(id: &str) -> Option<Value> {
    context()[id]
        .get("metadata")
        .filter(|value| !value.is_null())
        .cloned()
}
pub fn expand_path(value: &str) -> std::path::PathBuf {
    value
        .strip_prefix('~')
        .and_then(|rest| {
            dirs::home_dir()
                .map(|home| std::path::PathBuf::from(format!("{}{rest}", home.display())))
        })
        .unwrap_or_else(|| value.into())
}
