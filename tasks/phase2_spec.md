# Phase 2 Specification: Native Tool Protocol & Real-Time Streaming

## 1. Objective
Complete Phase 2 of `pi-rust` roadmap to provide:
1. **Real-time Server-Sent Events (SSE) Streaming**: Token-by-token streaming response processing for Anthropic, OpenAI, OpenRouter, Kilo, Agnes, and Ollama with live token emission.
2. **Dual Tool Protocol**: Robust schema-based tool calling with automatic fallback extraction for markdown code blocks (`bash`, `write`, `edit`, `read`, etc.).
3. **Interruptible Async Execution**: Async cancellation handling across `pi-core`, `pi-providers`, and `pi-tui` (via cancellation signals/handles) when `Escape` or steer inputs occur.
4. **Local Model Autodiscovery**: Live probing for Ollama (`:11434`), llama.cpp (`:8080`), and LM Studio (`:1234`) endpoints to dynamically populate the model catalog.

---

## 2. Component Design & Changes

### 2.1. `pi-providers`
- **Streaming Parser**: Implement SSE line parser reading `bytes_stream()` from `reqwest::Response`.
  - Anthropic Messages streaming (`content_block_start`, `content_block_delta`, `tool_use` chunk assembly).
  - OpenAI-compatible chat completion streaming (`data: {"choices":[{"delta":{"content":...,"tool_calls":...}}]}`).
- **`ProviderClient::stream_with_tools`**:
  ```rust
  pub async fn stream_with_tools<F>(
      config: &ModelConfig,
      system_prompt: &str,
      user_prompt: &str,
      tools: &[serde_json::Value],
      mut on_chunk: F,
  ) -> Result<ProviderResponse>
  where
      F: FnMut(String) + Send,
  ```
- **Local Endpoint Probing**:
  - Probe Ollama (`http://localhost:11434/api/tags`)
  - Probe llama.cpp (`http://localhost:8080/v1/models`)
  - Probe LM Studio (`http://localhost:1234/v1/models`)
  - Update `ModelConfig::resolve` to support `llamacpp/*` and `lmstudio/*`.

### 2.2. `pi-core`
- **Streaming Event Emission**:
  - Wire `on_chunk` to emit `TurnEvent::ModelStreaming { chunk }`.
- **Dual Tool Protocol**:
  - Native JSON tool calls returned from `stream_with_tools`.
  - Fallback tool extraction parser if `tool_calls` is empty:
    - Markdown ```` ```bash\n<command>\n``` ````
    - Markdown ```` ```write <path>\n<content>\n``` ````
    - Markdown ```` ```edit <path>\n<target>\n====\n<replacement>\n``` ````
    - Markdown ```` ```read <path>\n``` ````
- **Execution Loop**:
  - Iteratively executes tool calls, appends results to `SessionTree`, and feeds back into prompt up to `max_tool_iterations`.

### 2.3. `pi-tui`
- **Async Execution & Interruption**:
  - Handle `Escape` to cancel in-flight agent tasks cleanly.
  - Live stream rendering in transcript viewport as chunks arrive.

---

## 3. Verification & Quality Gates
- `cargo check --workspace --all-targets` (0 warnings)
- `cargo test --workspace` (100% tests pass)
- `cargo clippy --workspace --all-targets` (0 lints)
- Full regression test coverage for SSE stream parser, markdown fallback extraction, autodiscovery, and interruption.
