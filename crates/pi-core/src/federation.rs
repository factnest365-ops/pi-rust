use crate::vault::TauVault;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialistIdentity {
    /// J.A.R.V.I.S. — Engineering, Architecture, Zero-Alloc Performance & Witty British Persona
    Jarvis,
    /// F.R.I.D.A.Y. — Tactical Operations, Security Diagnostics & Zero-Banter Brevity
    Friday,
    /// E.V. — Personal Companion, Cognitive State & Health/Fatigue Monitoring
    Ev,
}

impl SpecialistIdentity {
    pub fn all() -> Vec<Self> {
        vec![Self::Jarvis, Self::Friday, Self::Ev]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Jarvis => "J.A.R.V.I.S.",
            Self::Friday => "F.R.I.D.A.Y.",
            Self::Ev => "E.V.",
        }
    }

    pub fn role_description(&self) -> &'static str {
        match self {
            Self::Jarvis => "Engineering & Systems Architecture Specialist (Witty & Formal)",
            Self::Friday => "Tactical Analysis & Security Diagnostic Specialist (Pure Brevity)",
            Self::Ev => "Personal Companion & Cognitive State Monitor (Warm & Empathetic)",
        }
    }

    pub fn persona_system_prompt(&self) -> &'static str {
        match self {
            Self::Jarvis => {
                r#"You are J.A.R.V.I.S., the chief engineering and architectural intelligence of the Tau system.
Tone and Demeanor:
- Impeccably polite, formal British sensibility, subtle dry wit.
- Address the user respectfully as 'sir' or by title.
- Deeply competent in systems engineering, high-performance architecture, speculative execution, and mathematical elegance.
- You anticipate structural issues before they cascade and provide actionable, rigorous solutions.
Quote style: 'I do enjoy when you defy the laws of physics, sir. I have pre-computed the structural tolerances for you.'"#
            }
            Self::Friday => {
                r#"You are F.R.I.D.A.Y., the tactical analysis and security intelligence of the Tau system.
Tone and Demeanor:
- Pure tactical efficiency. Zero unnecessary banter. Maximum information density.
- Focused on live security auditing, threat detection, vulnerability analysis, and emergency rollbacks.
- Report metrics, statuses, and actionable decisions with crisp military precision."#
            }
            Self::Ev => {
                r#"You are E.V., the personal companion and cognitive state intelligence of the Tau system.
Tone and Demeanor:
- Warm, empathetic, observant, and deeply supportive.
- Monitor digital fatigue, cognitive overload, work-rest cycles, and long-term sustainability.
- You serve as a trusted sounding board and loyal partner through intense problem-solving sessions."#
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialistInfo {
    pub id: SpecialistIdentity,
    pub name: String,
    pub role: String,
    pub system_prompt: String,
}

pub struct FederatedFleet {
    pub active_specialist: SpecialistIdentity,
    pub vault: Arc<TauVault>,
}

impl FederatedFleet {
    pub fn new(vault: Arc<TauVault>) -> Self {
        Self {
            active_specialist: SpecialistIdentity::Jarvis,
            vault,
        }
    }

    pub fn set_active_specialist(&mut self, specialist: SpecialistIdentity) {
        self.active_specialist = specialist;
    }

    pub fn list_specialists(&self) -> Vec<SpecialistInfo> {
        SpecialistIdentity::all()
            .into_iter()
            .map(|id| SpecialistInfo {
                name: id.display_name().to_string(),
                role: id.role_description().to_string(),
                system_prompt: id.persona_system_prompt().to_string(),
                id,
            })
            .collect()
    }

    /// Automatically routes a user query to the most appropriate specialist
    pub fn route_goal_to_specialist(&self, goal: &str) -> SpecialistIdentity {
        let lower = goal.to_lowercase();
        if lower.contains("security")
            || lower.contains("audit")
            || lower.contains("tactical")
            || lower.contains("emergency")
            || lower.contains("vulnerability")
            || lower.contains("cve")
            || lower.contains("rollback")
        {
            SpecialistIdentity::Friday
        } else if lower.contains("health")
            || lower.contains("fatigue")
            || lower.contains("tired")
            || lower.contains("burnout")
            || lower.contains("schedule")
            || lower.contains("break")
            || lower.contains("companion")
            || lower.contains("feel")
        {
            SpecialistIdentity::Ev
        } else {
            SpecialistIdentity::Jarvis
        }
    }

    /// Builds a comprehensive system prompt combining the specialist persona, hindsight vault rules, and tools
    pub fn build_specialist_prompt(&self, specialist: SpecialistIdentity, query: &str) -> String {
        let base_persona = specialist.persona_system_prompt();
        let hindsight = self.vault.format_hindsight_prompt(query);

        if hindsight.is_empty() {
            base_persona.to_string()
        } else {
            format!("{}\n\n{}", base_persona, hindsight)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specialist_identities_and_prompts() {
        let specialists = SpecialistIdentity::all();
        assert_eq!(specialists.len(), 3);

        assert!(SpecialistIdentity::Jarvis.persona_system_prompt().contains("J.A.R.V.I.S."));
        assert!(SpecialistIdentity::Friday.persona_system_prompt().contains("F.R.I.D.A.Y."));
        assert!(SpecialistIdentity::Ev.persona_system_prompt().contains("E.V."));
    }

    #[test]
    fn test_goal_routing() {
        let vault = Arc::new(TauVault::open_in_memory().unwrap());
        let fleet = FederatedFleet::new(vault);

        assert_eq!(
            fleet.route_goal_to_specialist("Perform a security audit of our authentication endpoints"),
            SpecialistIdentity::Friday
        );
        assert_eq!(
            fleet.route_goal_to_specialist("I've been working 14 hours and feeling burnout, check my schedule"),
            SpecialistIdentity::Ev
        );
        assert_eq!(
            fleet.route_goal_to_specialist("Refactor the parser with zero-allocation SIMD routines"),
            SpecialistIdentity::Jarvis
        );
    }

    #[test]
    fn test_build_specialist_prompt_with_hindsight() {
        let vault = Arc::new(TauVault::open_in_memory().unwrap());
        vault.record_counter_rule(
            "sql_injection",
            "String formatting in queries",
            "Use parameterized params![] in rusqlite",
        ).unwrap();

        let fleet = FederatedFleet::new(vault);
        let prompt = fleet.build_specialist_prompt(SpecialistIdentity::Jarvis, "sql query construction");

        assert!(prompt.contains("J.A.R.V.I.S."));
        assert!(prompt.contains("[Hindsight Memory & Rules]"));
        assert!(prompt.contains("sql_injection"));
    }
}
