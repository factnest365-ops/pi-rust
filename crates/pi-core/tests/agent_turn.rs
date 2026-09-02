use pi_core::AgentLoop;

#[test]
fn test_fallback_tool_calls_single() {
    let text = "Read this file:\n```read /path/to/file.txt\n```";
    let calls = AgentLoop::extract_fallback_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read");
    assert_eq!(calls[0].arguments["path"], "/path/to/file.txt");
}

#[test]
fn test_fallback_tool_calls_multiple() {
    let text = r#"
First read the config:
```read /etc/config.yaml
```
Then list the directory:
```ls /tmp
```
"#;
    let calls = AgentLoop::extract_fallback_tool_calls(text);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "read");
    assert_eq!(calls[1].name, "ls");
}

#[test]
fn test_tokenize_args() {
    let args = AgentLoop::tokenize_args(r#""hello world" "foo bar" baz"#);
    assert_eq!(args, vec!["hello world", "foo bar", "baz"]);
}

#[test]
fn test_extract_bash_command() {
    let text = "Run this:\n```bash echo hello\n```";
    let cmd = AgentLoop::extract_bash_command(text);
    assert_eq!(cmd, Some("echo hello"));
}

#[test]
fn test_no_tool_calls_in_plain_text() {
    let text = "Just a plain text response with no tools.";
    let calls = AgentLoop::extract_fallback_tool_calls(text);
    assert!(calls.is_empty());
}

#[test]
fn test_extract_fallback_web_git_github() {
    let text = r#"
```web_fetch https://example.com
```

```git status
```

```github pr_list
```

```lsp symbols src/lib.rs
```

```ast src/main.rs run
```
"#;
    let calls = AgentLoop::extract_fallback_tool_calls(text);
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0].name, "web_fetch");
    assert_eq!(calls[0].arguments["url"], "https://example.com");
    assert_eq!(calls[1].name, "git");
    assert_eq!(calls[2].name, "github");
    assert_eq!(calls[3].name, "lsp");
    assert_eq!(calls[4].name, "ast");
}
