use pi_tools::{ToolCall, ToolExecutor};
use std::fs;
use tempfile::tempdir;

fn make_call(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "test-1".to_string(),
        name: name.to_string(),
        arguments: args,
    }
}

#[tokio::test]
async fn test_tool_read() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("test.txt");
    fs::write(&path, "line1\nline2\nline3\n").unwrap();

    let call = make_call(
        "read",
        serde_json::json!({"path": path.to_str().unwrap(), "offset": 1, "limit": 2}),
    );
    let result = ToolExecutor::execute(&call).await;
    assert!(result.output.contains("line1"));
    assert!(result.output.contains("line2"));
    assert!(!result.is_error);
}

#[tokio::test]
async fn test_tool_write() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("output.txt");

    let call = make_call(
        "write",
        serde_json::json!({"path": path.to_str().unwrap(), "content": "hello world\n"}),
    );
    let result = ToolExecutor::execute(&call).await;
    assert!(result.output.contains("Successfully"));
    assert!(fs::read_to_string(&path).unwrap().contains("hello world"));
}

#[tokio::test]
async fn test_tool_edit() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("edit.txt");
    fs::write(&path, "fn hello() {\n    println!(\"world\");\n}\n").unwrap();

    let call = make_call(
        "edit",
        serde_json::json!({
            "path": path.to_str().unwrap(),
            "target": "println!(\"world\")",
            "replacement": "println!(\"hello\")"
        }),
    );
    let _result = ToolExecutor::execute(&call).await;
    // Edit returns the diff or success message
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("println!(\"hello\")"));
}

#[tokio::test]
async fn test_tool_grep() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("a.rs"), "fn foo() {}\nfn bar() {}\n").unwrap();
    fs::write(tmp.path().join("b.rs"), "fn baz() {}\nfn foo() {}\n").unwrap();

    let call = make_call(
        "grep",
        serde_json::json!({"path": tmp.path().to_str().unwrap(), "pattern": "fn foo"}),
    );
    let result = ToolExecutor::execute(&call).await;
    assert!(result.output.contains("a.rs"));
    assert!(result.output.contains("b.rs"));
}

#[tokio::test]
async fn test_tool_find() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "").unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "").unwrap();

    let call = make_call(
        "find",
        serde_json::json!({"path": tmp.path().to_str().unwrap(), "name": "*.rs"}),
    );
    let result = ToolExecutor::execute(&call).await;
    assert!(result.output.contains("main.rs"));
}

#[tokio::test]
async fn test_tool_ls() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("file1.txt"), "").unwrap();
    fs::write(tmp.path().join("file2.rs"), "").unwrap();
    fs::create_dir(tmp.path().join("dir")).unwrap();

    let call = make_call(
        "ls",
        serde_json::json!({"path": tmp.path().to_str().unwrap()}),
    );
    let result = ToolExecutor::execute(&call).await;
    assert!(result.output.contains("file1.txt"));
    assert!(result.output.contains("file2.rs"));
    assert!(result.output.contains("dir"));
}

#[tokio::test]
async fn test_tool_bash() {
    let call = make_call("bash", serde_json::json!({"command": "echo hello"}));
    let result = ToolExecutor::execute(&call).await;
    assert!(result.output.contains("hello"));
}

#[tokio::test]
async fn test_tool_read_missing() {
    let call = make_call(
        "read",
        serde_json::json!({"path": "/nonexistent/file.txt", "offset": 1, "limit": 10}),
    );
    let result = ToolExecutor::execute(&call).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn test_tool_definitions() {
    let defs = ToolExecutor::tool_definitions();
    assert!(!defs.is_empty());
    let names: Vec<&str> = defs.iter().map(|d| d["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"read"));
    assert!(names.contains(&"write"));
    assert!(names.contains(&"edit"));
    assert!(names.contains(&"bash"));
    assert!(names.contains(&"grep"));
    assert!(names.contains(&"find"));
    assert!(names.contains(&"ls"));
}
