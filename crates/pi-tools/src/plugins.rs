use anyhow::Result;
use serde_json::Value;

pub trait ToolPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    fn execute(&self, args: &Value) -> Result<String>;
}
