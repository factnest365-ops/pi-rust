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
        // 1. OSC 1337 SetUserVar for Herdr / multiplexer status detection
        // 2. OSC 0 Window/Tab Title prefix with state badge
        format!(
            "\x1b]1337;SetUserVar=herdr_state={}\x07\x1b]0;pi [{}]\x07",
            state.as_str(),
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
    }

    #[test]
    fn test_format_osc_state() {
        let osc = HerdrProtocol::format_osc_state(HerdrAgentState::Working);
        assert!(osc.contains("herdr_state=working"));
        assert!(osc.contains("pi [working]"));
    }

    #[test]
    fn test_detect_environment() {
        let env = HerdrProtocol::detect_environment();
        // Should detect valid boolean flags without panicking
        assert_eq!(env.is_herdr, std::env::var("HERDR_SESSION").is_ok() || std::env::var("HERDR").is_ok());
    }
}
