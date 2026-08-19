use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, Write};

/// Terminal agent states recognized by Herdr multiplexer and FirstMate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HerdrAgentState {
    Working,
    Blocked,
    Done,
    Idle,
}

impl HerdrAgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Idle => "idle",
        }
    }
}

impl std::fmt::Display for HerdrAgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Active execution environment detected by pi-rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HerdrEnvironment {
    pub is_herdr: bool,
    pub is_tmux: bool,
    pub is_zellij: bool,
    pub is_firstmate: bool,
    pub session_id: Option<String>,
}

pub struct HerdrProtocol;

impl HerdrProtocol {
    /// Detects if running inside Herdr, Tmux, Zellij, or FirstMate
    pub fn detect_environment() -> HerdrEnvironment {
        let is_herdr = env::var("HERDR_SESSION").is_ok() || env::var("HERDR").is_ok();
        let is_tmux = env::var("TMUX").is_ok();
        let is_zellij = env::var("ZELLIJ").is_ok();
        let is_firstmate = env::var("FIRSTMATE_HOME").is_ok() || env::var("FM_HOME").is_ok();

        let session_id = env::var("HERDR_SESSION")
            .or_else(|_| env::var("TMUX_PANE"))
            .or_else(|_| env::var("ZELLIJ_SESSION_NAME"))
            .ok();

        HerdrEnvironment {
            is_herdr,
            is_tmux,
            is_zellij,
            is_firstmate,
            session_id,
        }
    }

    /// Formats OSC sequence to set agent state in Herdr / iTerm / modern terminals
    pub fn format_osc_state(state: HerdrAgentState) -> String {
        // 1. OSC 1337 SetUserVar for Herdr / multiplexer status detection (value must be base64-encoded)
        // 2. OSC 0 Window/Tab Title prefix with state badge
        let b64_val = match state {
            HerdrAgentState::Working => "d29ya2luZw==",
            HerdrAgentState::Blocked => "YmxvY2tlZA==",
            HerdrAgentState::Done => "ZG9uZQ==",
            HerdrAgentState::Idle => "aWRsZQ==",
        };
        format!(
            "\x1b]1337;SetUserVar=herdr_state={}\x07\x1b]0;pi [{}]\x07",
            b64_val,
            state.as_str()
        )
    }

    /// Emits state change escape sequence to stderr to preserve stdout purity for RPC/pipes
    pub fn emit_state(state: HerdrAgentState) {
        let seq = Self::format_osc_state(state);
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(seq.as_bytes());
        let _ = stderr.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_herdr_agent_states() {
        assert_eq!(HerdrAgentState::Working.as_str(), "working");
        assert_eq!(HerdrAgentState::Blocked.as_str(), "blocked");
        assert_eq!(HerdrAgentState::Done.as_str(), "done");
        assert_eq!(HerdrAgentState::Idle.as_str(), "idle");

        assert_eq!(HerdrAgentState::Working.to_string(), "working");
        assert_eq!(HerdrAgentState::Blocked.to_string(), "blocked");
        assert_eq!(HerdrAgentState::Done.to_string(), "done");
        assert_eq!(HerdrAgentState::Idle.to_string(), "idle");
    }

    #[test]
    fn test_format_osc_state_all_variants() {
        // 1. Working: "working" -> d29ya2luZw==
        let osc_working = HerdrProtocol::format_osc_state(HerdrAgentState::Working);
        assert_eq!(
            osc_working,
            "\x1b]1337;SetUserVar=herdr_state=d29ya2luZw==\x07\x1b]0;pi [working]\x07"
        );

        // 2. Blocked: "blocked" -> YmxvY2tlZA==
        let osc_blocked = HerdrProtocol::format_osc_state(HerdrAgentState::Blocked);
        assert_eq!(
            osc_blocked,
            "\x1b]1337;SetUserVar=herdr_state=YmxvY2tlZA==\x07\x1b]0;pi [blocked]\x07"
        );

        // 3. Done: "done" -> ZG9uZQ==
        let osc_done = HerdrProtocol::format_osc_state(HerdrAgentState::Done);
        assert_eq!(
            osc_done,
            "\x1b]1337;SetUserVar=herdr_state=ZG9uZQ==\x07\x1b]0;pi [done]\x07"
        );

        // 4. Idle: "idle" -> aWRsZQ==
        let osc_idle = HerdrProtocol::format_osc_state(HerdrAgentState::Idle);
        assert_eq!(
            osc_idle,
            "\x1b]1337;SetUserVar=herdr_state=aWRsZQ==\x07\x1b]0;pi [idle]\x07"
        );
    }

    #[test]
    fn test_herdr_agent_state_serde() {
        let json_working = serde_json::to_string(&HerdrAgentState::Working).unwrap();
        assert_eq!(json_working, "\"working\"");
        let parsed: HerdrAgentState = serde_json::from_str(&json_working).unwrap();
        assert_eq!(parsed, HerdrAgentState::Working);

        let json_blocked = serde_json::to_string(&HerdrAgentState::Blocked).unwrap();
        assert_eq!(json_blocked, "\"blocked\"");
        let parsed_blocked: HerdrAgentState = serde_json::from_str(&json_blocked).unwrap();
        assert_eq!(parsed_blocked, HerdrAgentState::Blocked);
    }

    #[test]
    fn test_detect_environment() {
        let env = HerdrProtocol::detect_environment();
        assert_eq!(
            env.is_herdr,
            std::env::var("HERDR_SESSION").is_ok() || std::env::var("HERDR").is_ok()
        );
    }

    #[test]
    fn test_emit_state_smoke() {
        // Smoke test to ensure emit_state does not panic or error
        HerdrProtocol::emit_state(HerdrAgentState::Working);
        HerdrProtocol::emit_state(HerdrAgentState::Blocked);
        HerdrProtocol::emit_state(HerdrAgentState::Done);
        HerdrProtocol::emit_state(HerdrAgentState::Idle);
    }
}
