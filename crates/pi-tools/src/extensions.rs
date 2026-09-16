//! P1 extension bridge: minimal JS extension loader + runner.
//!
//! Scope (spike step 2): `session_start` + `registerTool` only. All other
//! `pi.on(...)` event types are accepted-but-ignored by the harness (`pi.on`
//! is a no-op) so upstream examples parse without crashing.
//!
//! # Threading (QuickJS single-thread rule)
//!
//! QuickJS is single-threaded: every load and every tool call builds a fresh
//! `rquickjs::Runtime` + `Context` on the calling thread and drops it before
//! returning. A `Context` is never stored, shared, or sent across threads.
//! Per-extension isolation falls out naturally: one `Runtime` per extension
//! file per call, no cross-extension globals.
//!
//! # Trust gate (best-effort static check)
//!
//! Upstream Pi extensions run with full user permissions; sandboxing is
//! opt-in per tool. This bridge keeps the default-deny posture for the
//! spike: the loader exposes NO `fs`, network (`fetch`, `WebSocket`,
//! `XMLHttpRequest`), child-process, or `node:` bindings inside the QuickJS
//! context, and [`trust_gate_check`] additionally refuses sources that
//! reference common network/process escape hatches. This is a static string
//! screen, not a sandbox — a determined script can obfuscate around it.
//! Full trust-manager port (per-cwd trust store + prompt) is a follow-up.
//! The `hello` fixture needs none of these capabilities and passes cleanly.
//!
//! # Cargo feature
//!
//! The `rquickjs` dependency is optional behind the crate `extensions`
//! feature (default off), so the default build keeps the repo's 100% pure
//! Rust invariant. Without the feature, loading/executing returns a clear
//! error telling the operator to rebuild with `--features extensions`.

use anyhow::{Context as _, Result};
use serde_json::Value;

/// One tool definition captured from an extension JS file.
#[derive(Debug, Clone)]
pub struct LoadedToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Harness installed as `globalThis.__pi` / `globalThis.pi` before the
/// extension source is evaluated. Mirrors `/tmp/ext-spike/spike/src/main.rs`:
/// captures every `registerTool` definition plus its `execute` fn, and
/// provides a no-op `on()` for out-of-scope lifecycle events.
#[cfg_attr(not(feature = "extensions"), allow(dead_code))]
const HARNESS_INIT_JS: &str = r#"
globalThis.__toolDefs = [];
globalThis.__toolFns = {};
globalThis.__pi = {
    registerTool: function (def) {
        globalThis.__toolDefs.push({
            name: def.name,
            description: def.description,
            parameters: def.parameters
        });
        globalThis.__toolFns[def.name] = def.execute;
    },
    on: function () { /* session_start + registerTool only; ignore rest */ }
};
globalThis.pi = globalThis.__pi;
"#;

/// After evaluating the source, invoke the hello-style factory when present
/// (`registerHello(pi)` convention from `fixture/hello.js`). Files that call
/// `pi.registerTool(...)` at top level need nothing further.
#[cfg_attr(not(feature = "extensions"), allow(dead_code))]
const HARNESS_INVOKE_JS: &str = r#"
if (typeof registerHello === 'function') { registerHello(globalThis.__pi); }
JSON.stringify(globalThis.__toolDefs);
"#;

/// Best-effort static trust screen. Refuses sources referencing network /
/// process escape hatches that the harness deliberately does not bind.
/// Case-sensitive substring match; documented as non-exhaustive.
pub fn trust_gate_check(source: &str) -> Result<()> {
    const DENY: &[&str] = &[
        "require(",
        "fetch(",
        "XMLHttpRequest",
        "WebSocket(",
        "child_process",
        "node:",
        "Deno.",
        "process.",
    ];
    let hits: Vec<&str> = DENY
        .iter()
        .copied()
        .filter(|pat| source.contains(pat))
        .collect();
    if hits.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Extension trust gate: refused pattern(s) {} (network/file/process ops are not available to extensions in this build)",
            hits.join(", ")
        ))
    }
}

/// Load tool definitions from JS source without registering them.
#[cfg(feature = "extensions")]
pub fn load_extension_defs(source: &str, origin: &str) -> Result<Vec<LoadedToolDef>> {
    use rquickjs::{Context, Runtime};
    let rt = Runtime::new().context("create QuickJS runtime")?;
    let ctx = Context::full(&rt).context("create QuickJS context")?;
    ctx.with(|ctx| -> Result<Vec<LoadedToolDef>> {
        ctx.eval::<(), _>(HARNESS_INIT_JS)
            .map_err(|e| anyhow::anyhow!("eval extension harness: {e}"))?;
        ctx.eval::<(), _>(source)
            .map_err(|e| anyhow::anyhow!("eval extension {origin}: {e}"))?;
        let defs_json: String = ctx
            .eval(HARNESS_INVOKE_JS)
            .map_err(|e| anyhow::anyhow!("invoke extension {origin}: {e}"))?;
        let defs: Vec<LoadedToolDefJson> = serde_json::from_str(&defs_json)
            .context("parse registered tool defs as JSON")?;
        Ok(defs
            .into_iter()
            .map(|d| LoadedToolDef {
                name: d.name,
                description: d.description.unwrap_or_default(),
                parameters: d.parameters.unwrap_or(Value::Bool(false)),
            })
            .collect())
    })
}

/// No-`extensions`-feature stub: loading is unavailable in pure-Rust builds.
#[cfg(not(feature = "extensions"))]
pub fn load_extension_defs(_source: &str, _origin: &str) -> Result<Vec<LoadedToolDef>> {
    Err(anyhow::anyhow!(
        "Extensions require the pi-tools `extensions` cargo feature (rebuild with --features extensions)."
    ))
}

/// Execute one registered tool: fresh QuickJS `Runtime` on this thread,
/// re-evaluates the stored source, captures `execute` fns, calls the one
/// matching `name` with `args` (JSON in), returns string (JSON out for
/// non-string returns). `pub(crate)` — called via `PluginRegistry::execute`.
#[cfg(feature = "extensions")]
pub(crate) fn run_js_tool(source: &str, name: &str, args: &Value) -> Result<String> {
    use rquickjs::{Context, Runtime};
    let args_json = serde_json::to_string(args).unwrap_or_else(|_| "null".to_string());
    let name_json = serde_json::to_string(name).unwrap_or_else(|_| "\"\"".to_string());
    let rt = Runtime::new().context("create QuickJS runtime")?;
    let ctx = Context::full(&rt).context("create QuickJS context")?;
    ctx.with(|ctx| -> Result<String> {
        ctx.eval::<(), _>(HARNESS_INIT_JS)
            .map_err(|e| anyhow::anyhow!("eval extension harness: {e}"))?;
        ctx.eval::<(), _>(source)
            .map_err(|e| anyhow::anyhow!("eval extension tool '{name}': {e}"))?;
        ctx.eval::<(), _>(HARNESS_INVOKE_JS)
            .map_err(|e| anyhow::anyhow!("invoke extension tool '{name}': {e}"))?;
        let template = concat!(
            "(function(){ var f = globalThis.__toolFns[__NAME__]; ",
            "if (typeof f !== 'function') { throw new Error('tool not found'); } ",
            "var r = f(__ARGS__); ",
            "return (typeof r === 'string') ? r : JSON.stringify(r); })()",
        );
        let call_js: String = template
            .replace("__NAME__", &name_json)
            .replace("__ARGS__", &args_json);
        ctx.eval(call_js)
            .map_err(|e| anyhow::anyhow!("call extension tool '{name}': {e}"))
    })
}

#[cfg(feature = "extensions")]
#[derive(serde::Deserialize)]
struct LoadedToolDefJson {
    name: String,
    description: Option<String>,
    parameters: Option<Value>,
}

/// Validate trust gate, load defs, and register each into the global
/// [`crate::plugins`] registry with the source retained for execution.
/// Returns the registered tool names. `session_start` needs no explicit
/// emit here: registration happens synchronously at load.
pub fn register_extension_source(source: &str, origin: &str) -> Result<Vec<String>> {
    trust_gate_check(source)?;
    let defs = load_extension_defs(source, origin)?;
    let mut names = Vec::with_capacity(defs.len());
    for def in defs {
        crate::plugins::register_plugin_tool(
            def.name.clone(),
            def.description.clone(),
            def.parameters.clone(),
            source.to_string(),
        );
        names.push(def.name);
    }
    Ok(names)
}

/// Read a JS extension file and register its tools. Used by CLI `-e`.
pub fn load_extension_file(path: &std::path::Path) -> Result<Vec<String>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("read extension file '{}'", path.display()))?;
    register_extension_source(&source, &path.display().to_string())
}

/// Lifecycle stub: `session_start` is implicit at load time (registration
/// runs synchronously in [`register_extension_source`]). Kept so runners
/// have a named hook to call; currently a no-op.
pub fn emit_session_start() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_gate_allows_hello_denies_network() {
        let hello = include_str!("../tests/fixtures/hello.js");
        assert!(trust_gate_check(hello).is_ok());
        assert!(trust_gate_check("fetch('https://example.com')").is_err());
        assert!(trust_gate_check("const fs = require('fs');").is_err());
        assert!(trust_gate_check("import x from 'node:fs';").is_err());
    }

    #[cfg(feature = "extensions")]
    #[test]
    fn hello_fixture_end_to_end() {
        let _guard = crate::plugins::TEST_REGISTRY_LOCK.lock().unwrap();
        let source = include_str!("../tests/fixtures/hello.js");
        crate::plugins::clear_plugin_tools();
        let names = register_extension_source(source, "hello.js").expect("load hello.js");
        assert_eq!(names, vec!["hello".to_string()]);

        let defs = crate::plugins::plugin_tool_definitions();
        assert!(
            defs.iter()
                .any(|d| d.get("name").and_then(|n| n.as_str()) == Some("hello")),
            "defs should contain hello: {defs:?}"
        );

        let out = crate::plugins::execute_plugin_tool("hello", &serde_json::json!({"name": "Ada"}))
            .expect("hello is registered")
            .expect("hello executes");
        assert_eq!(out, "Hello, Ada!");

        // Unknown tool falls through (None) so executor can try MCP/next.
        assert!(
            crate::plugins::execute_plugin_tool("nope", &Value::Null).is_none()
        );
        crate::plugins::clear_plugin_tools();
    }

    #[cfg(feature = "extensions")]
    #[test]
    fn fallback_parser_sees_extension_tool() {
        let _guard = crate::plugins::TEST_REGISTRY_LOCK.lock().unwrap();
        crate::plugins::clear_plugin_tools();
        register_extension_source(include_str!("../tests/fixtures/hello.js"), "hello.js")
            .expect("load hello.js");
        assert!(crate::plugins::plugin_tool_names().contains(&"hello".to_string()));
        crate::plugins::clear_plugin_tools();
    }

    #[cfg(not(feature = "extensions"))]
    #[test]
    fn loader_errors_without_feature() {
        let err = load_extension_defs("var x = 1;", "t.js").unwrap_err();
        assert!(err.to_string().contains("extensions"));
    }
}
