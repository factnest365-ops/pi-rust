use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

pub trait ToolPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    fn execute(&self, args: &Value) -> Result<String>;
}

/// Definition of a dynamically registered (extension-provided) tool.
#[derive(Debug, Clone)]
pub struct PluginToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl PluginToolDefinition {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "parameters": self.parameters,
        })
    }
}

/// One registered plugin entry.
///
/// QuickJS single-thread rule: the extension JS source is stored here and
/// every execution builds a fresh `Runtime` + `Context` on the calling
/// thread (see `crate::extensions::run_js_tool`). A QuickJS `Context` is
/// never shared across threads.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub def: PluginToolDefinition,
    pub js_source: String,
}

/// Registry of dynamically registered extension tools (P1 extension bridge).
/// Consulted by `ToolExecutor::execute`, `ToolExecutor::tool_definitions`,
/// and the `pi-core` markdown fallback parser (Dual Tool Protocol).
#[derive(Debug, Default)]
pub struct PluginRegistry {
    entries: BTreeMap<String, PluginEntry>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) a tool definition with its JS source.
    pub fn register(
        &mut self,
        name: String,
        description: String,
        parameters: Value,
        js_source: String,
    ) {
        self.entries.insert(
            name.clone(),
            PluginEntry {
                def: PluginToolDefinition {
                    name,
                    description,
                    parameters,
                },
                js_source,
            },
        );
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Sorted tool names (BTreeMap order; deterministic for prompts).
    pub fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// JSON tool definitions for the model (`tool_definitions` merge).
    pub fn definitions(&self) -> Vec<Value> {
        self.entries.values().map(|e| e.def.to_json()).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Execute a registered tool. Returns `None` when `name` is unknown so
    /// callers can fall through to the next backend (MCP, unknown-tool).
    /// Each call builds a fresh QuickJS `Runtime` on this thread.
    pub fn execute(&self, name: &str, args: &Value) -> Option<Result<String>> {
        let entry = self.entries.get(name)?;
        #[cfg(feature = "extensions")]
        {
            Some(crate::extensions::run_js_tool(&entry.js_source, name, args))
        }
        #[cfg(not(feature = "extensions"))]
        {
            let _ = entry;
            let _ = args;
            Some(Err(anyhow::anyhow!(
                "Extension tool '{name}' is registered but the `extensions` cargo feature is off (rebuild pi-tools with --features extensions)."
            )))
        }
    }
}

fn global_registry() -> &'static Mutex<PluginRegistry> {
    static REGISTRY: OnceLock<Mutex<PluginRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(PluginRegistry::new()))
}

/// Register an extension tool globally (called by the extension loader).
pub fn register_plugin_tool(
    name: String,
    description: String,
    parameters: Value,
    js_source: String,
) {
    if let Ok(mut reg) = global_registry().lock() {
        reg.register(name, description, parameters, js_source);
    }
}

/// JSON definitions of all registered extension tools.
pub fn plugin_tool_definitions() -> Vec<Value> {
    global_registry()
        .lock()
        .map(|reg| reg.definitions())
        .unwrap_or_default()
}

/// Names of all registered extension tools (fallback-parser allowlist).
pub fn plugin_tool_names() -> Vec<String> {
    global_registry()
        .lock()
        .map(|reg| reg.names())
        .unwrap_or_default()
}

/// Execute a registered extension tool; `None` when the name is unknown.
pub fn execute_plugin_tool(name: &str, args: &Value) -> Option<Result<String>> {
    global_registry().lock().ok()?.execute(name, args)
}

/// Drop all registered extension tools. Primarily for tests and
/// `--no-extensions` handling.
pub fn clear_plugin_tools() {
    if let Ok(mut reg) = global_registry().lock() {
        reg.clear();
    }
}

/// Serializes tests that mutate the global registry (parallel harness).
#[cfg(test)]
pub(crate) static TEST_REGISTRY_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_register_list_clear() {
        let mut reg = PluginRegistry::new();
        assert!(reg.execute("hello", &Value::Null).is_none());
        reg.register(
            "hello".to_string(),
            "A simple greeting tool".to_string(),
            serde_json::json!({"type": "object"}),
            "fixture-source".to_string(),
        );
        assert!(reg.contains("hello"));
        assert_eq!(reg.names(), vec!["hello".to_string()]);
        let defs = reg.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["name"], "hello");
        reg.clear();
        assert!(reg.definitions().is_empty());
    }

    #[test]
    fn global_registry_roundtrip_isolated() {
        let _guard = TEST_REGISTRY_LOCK.lock().unwrap();
        clear_plugin_tools();
        register_plugin_tool(
            "specimen".to_string(),
            "test tool".to_string(),
            serde_json::json!({"type": "object"}),
            String::new(),
        );
        assert!(plugin_tool_names().contains(&"specimen".to_string()));
        assert!(
            plugin_tool_definitions()
                .iter()
                .any(|d| d["name"] == "specimen")
        );
        clear_plugin_tools();
        assert!(plugin_tool_definitions().is_empty());
    }
}
