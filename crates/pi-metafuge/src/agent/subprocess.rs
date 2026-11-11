use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::agent::{
    AgentDescription, AgentKind, DestroyRequest, DestroyResponse, ForgeRequest, ForgeResponse,
    JudgeRequest, JudgeResponse, MetafugeAgent, SpecRequest, SpecResponse,
};
use crate::manifest::MANIFEST_SCHEMA;
use crate::support::{CommandResult, preview};

/// Default wall-clock budget for one agent exchange.
pub const DEFAULT_AGENT_TIMEOUT: Duration = Duration::from_secs(120);

/// The seam for a real harness: one JSON request on stdin, one JSON response on
/// stdout, hard timeout, no chat on the wire.
///
/// The command is run through the user's shell, exactly as handed to
/// `--agent-cmd`. No provider is wired here in this ship.
#[derive(Debug, Clone)]
pub struct SubprocessAgent {
    command: String,
    timeout: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
struct Envelope<T> {
    schema: u32,
    op: String,
    payload: T,
}

impl SubprocessAgent {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            timeout: DEFAULT_AGENT_TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_timeout(command: impl Into<String>, timeout: Duration) -> Self {
        Self {
            command: command.into(),
            timeout,
        }
    }

    async fn call<Request, Response>(&self, op: &str, request: &Request) -> Result<Response>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let envelope = Envelope {
            schema: MANIFEST_SCHEMA,
            op: op.to_string(),
            payload: request,
        };
        let body = serde_json::to_vec(&envelope)
            .with_context(|| format!("could not encode the `{op}` request"))?;
        let result = self.exchange(&body).await?;
        if !result.succeeded() {
            bail!(
                "agent command `{}` exited with {:?} during `{op}`: {}",
                self.command,
                result.exit_code,
                preview(&result.stderr, 400)
            );
        }
        let envelope: Envelope<Response> =
            serde_json::from_str(&result.stdout).with_context(|| {
                format!(
                    "agent `{op}` response is not a valid JSON envelope (stdout was `{}`)",
                    preview(&result.stdout, 200)
                )
            })?;
        if envelope.schema != MANIFEST_SCHEMA {
            bail!(
                "agent `{op}` responded with schema {} but this build speaks {MANIFEST_SCHEMA}",
                envelope.schema
            );
        }
        if envelope.op != op {
            bail!(
                "agent answered `{}` while `{op}` was requested",
                envelope.op
            );
        }
        Ok(envelope.payload)
    }

    async fn exchange(&self, body: &[u8]) -> Result<CommandResult> {
        use tokio::io::AsyncWriteExt;

        let mut command = tokio::process::Command::new("sh");
        command
            .arg("-c")
            .arg(&self.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .with_context(|| format!("could not spawn agent command `{}`", self.command))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(body)
                .await
                .with_context(|| format!("could not write to agent `{}`", self.command))?;
            stdin
                .shutdown()
                .await
                .with_context(|| format!("could not close agent stdin `{}`", self.command))?;
        }

        let started = std::time::Instant::now();
        match tokio::time::timeout(self.timeout, child.wait_with_output()).await {
            Ok(output) => {
                let output = output
                    .with_context(|| format!("agent `{}` could not be joined", self.command))?;
                Ok(CommandResult {
                    command: self.command.clone(),
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                    duration_ms: started.elapsed().as_millis() as u64,
                    timed_out: false,
                })
            }
            Err(_) => bail!(
                "agent command `{}` exceeded its {}s budget",
                self.command,
                self.timeout.as_secs()
            ),
        }
    }
}

#[async_trait]
impl MetafugeAgent for SubprocessAgent {
    fn describe(&self) -> AgentDescription {
        AgentDescription {
            kind: AgentKind::Subprocess,
            name: "subprocess".to_string(),
            note: format!(
                "external harness process `{}`; METAFUGE does not describe its internals",
                self.command
            ),
            model_backed: true,
        }
    }

    async fn spec(&self, request: &SpecRequest) -> Result<SpecResponse> {
        self.call("spec", request).await
    }

    async fn forge(&self, request: &ForgeRequest) -> Result<ForgeResponse> {
        self.call("forge", request).await
    }

    async fn destroy(&self, request: &DestroyRequest) -> Result<DestroyResponse> {
        self.call("destroy", request).await
    }

    async fn judge(&self, request: &JudgeRequest) -> Result<JudgeResponse> {
        self.call("judge", request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct Ping {
        value: u32,
    }

    #[tokio::test]
    async fn round_trips_one_json_envelope() {
        let agent = SubprocessAgent::with_timeout("cat", Duration::from_secs(5));
        let response: Ping = agent.call("ping", &Ping { value: 7 }).await.expect("call");
        assert_eq!(response.value, 7);
    }

    #[tokio::test]
    async fn malformed_output_is_a_clear_error() {
        let agent =
            SubprocessAgent::with_timeout("cat >/dev/null; echo not-json", Duration::from_secs(5));
        let error = agent
            .call::<Ping, Ping>("ping", &Ping { value: 1 })
            .await
            .expect_err("malformed output must fail");
        let message = error.to_string();
        assert!(message.contains("not a valid JSON envelope"));
        assert!(message.contains("not-json"));
    }

    #[tokio::test]
    async fn non_zero_exit_is_a_clear_error() {
        let agent = SubprocessAgent::with_timeout(
            "cat >/dev/null; echo boom >&2; exit 3",
            Duration::from_secs(5),
        );
        let error = agent
            .call::<Ping, Ping>("ping", &Ping { value: 1 })
            .await
            .expect_err("non-zero exit must fail");
        assert!(error.to_string().contains("exited with Some(3)"));
        assert!(error.to_string().contains("boom"));
    }

    #[tokio::test]
    async fn a_hung_agent_hits_the_timeout() {
        let agent =
            SubprocessAgent::with_timeout("cat >/dev/null; sleep 30", Duration::from_millis(50));
        let error = agent
            .call::<Ping, Ping>("ping", &Ping { value: 1 })
            .await
            .expect_err("hung agent must time out");
        assert!(error.to_string().contains("budget"));
    }
}
