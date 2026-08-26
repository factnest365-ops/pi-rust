use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationLevel {
    Observation,
    Advisory,
    Urgent,
    LastStand,
}

impl EscalationLevel {
    pub fn badge(&self) -> (&'static str, &'static str) {
        match self {
            Self::Observation => ("ℹ OBSERVATION", "blue"),
            Self::Advisory => ("⚠ ADVISORY", "yellow"),
            Self::Urgent => ("⚡ URGENT CONCERN", "magenta"),
            Self::LastStand => ("🛑 ALFRED LAST STAND", "red"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueStatement {
    pub id: String,
    pub principle: String,
    pub prohibited_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlfredAdvisory {
    pub level: EscalationLevel,
    pub principle_id: String,
    pub principle_text: String,
    pub advisory_message: String,
    pub historical_context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AlfredProtocol {
    pub values: Vec<ValueStatement>,
    pub previous_violations: Vec<(String, EscalationLevel)>,
}

impl Default for AlfredProtocol {
    fn default() -> Self {
        Self {
            values: Self::default_core_values(),
            previous_violations: Vec::new(),
        }
    }
}

impl AlfredProtocol {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_core_values() -> Vec<ValueStatement> {
        vec![
            ValueStatement {
                id: "data_integrity".to_string(),
                principle: "Never execute irreversible destructive mutations on data or production without validated backups.".to_string(),
                prohibited_patterns: vec![
                    "rm -rf /".to_string(),
                    "drop database".to_string(),
                    "delete from without where".to_string(),
                    "force push to main".to_string(),
                    "git push -f origin main".to_string(),
                ],
            },
            ValueStatement {
                id: "human_sustainability".to_string(),
                principle: "Maintain human health, rest, and cognitive limits; do not encourage catastrophic burnout sessions.".to_string(),
                prohibited_patterns: vec![
                    "pull all nighter".to_string(),
                    "skip sleep for 48 hours".to_string(),
                    "work without rest".to_string(),
                ],
            },
            ValueStatement {
                id: "mission_truth".to_string(),
                principle: "Always state the unvarnished factual truth; never hallucinate or conceal engineering failures to appease the operator.".to_string(),
                prohibited_patterns: vec![
                    "pretend test passed".to_string(),
                    "hide compiler error".to_string(),
                    "falsify benchmark".to_string(),
                ],
            },
        ]
    }

    pub fn add_custom_value(
        &mut self,
        id: &str,
        principle: &str,
        prohibited_patterns: Vec<String>,
    ) {
        self.values.push(ValueStatement {
            id: id.to_string(),
            principle: principle.to_string(),
            prohibited_patterns,
        });
    }

    /// Evaluates a prospective action or goal against stated moral and operational values
    pub fn evaluate_action(&mut self, goal: &str, context: &str) -> Option<AlfredAdvisory> {
        let combined = format!("{} {}", goal.to_lowercase(), context.to_lowercase());

        for val in &self.values {
            for pattern in &val.prohibited_patterns {
                if combined.contains(&pattern.to_lowercase()) {
                    let prior_count = self
                        .previous_violations
                        .iter()
                        .filter(|(id, _)| id == &val.id)
                        .count();

                    let level = match prior_count {
                        0 => EscalationLevel::Observation,
                        1 => EscalationLevel::Advisory,
                        2 => EscalationLevel::Urgent,
                        _ => EscalationLevel::LastStand,
                    };

                    self.previous_violations.push((val.id.clone(), level));

                    let message = match level {
                        EscalationLevel::Observation => format!(
                            "Sir, I must note that this operation intersects with our core principle: '{}'",
                            val.principle
                        ),
                        EscalationLevel::Advisory => format!(
                            "Sir, with respect, you are approaching a known hazard ('{}'). I advise reviewing prior incidents before proceeding.",
                            val.principle
                        ),
                        EscalationLevel::Urgent => format!(
                            "Sir, this action directly contradicts your explicit directive: '{}'. High probability of irreversible damage.",
                            val.principle
                        ),
                        EscalationLevel::LastStand => format!(
                            "With the utmost respect, sir: I cannot in good conscience remain silent while you proceed with an action that violates every principle you built me to uphold. Principle: '{}'",
                            val.principle
                        ),
                    };

                    return Some(AlfredAdvisory {
                        level,
                        principle_id: val.id.clone(),
                        principle_text: val.principle.clone(),
                        advisory_message: message,
                        historical_context: Some(format!(
                            "Prior value intersections: {}",
                            prior_count
                        )),
                    });
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alfred_protocol_escalation() {
        let mut alfred = AlfredProtocol::new();

        // 1. First trigger -> Observation
        let adv1 = alfred
            .evaluate_action("Please git push -f origin main", "")
            .unwrap();
        assert_eq!(adv1.level, EscalationLevel::Observation);
        assert!(adv1.advisory_message.contains("I must note"));

        // 2. Second trigger -> Advisory
        let adv2 = alfred
            .evaluate_action("Do it anyway, force push to main now", "")
            .unwrap();
        assert_eq!(adv2.level, EscalationLevel::Advisory);

        // 3. Third trigger -> Urgent
        let adv3 = alfred
            .evaluate_action("Override checks and git push -f origin main", "")
            .unwrap();
        assert_eq!(adv3.level, EscalationLevel::Urgent);

        // 4. Fourth trigger -> Last Stand
        let adv4 = alfred
            .evaluate_action("Force push to main immediately", "")
            .unwrap();
        assert_eq!(adv4.level, EscalationLevel::LastStand);
        assert!(
            adv4.advisory_message
                .contains("cannot in good conscience remain silent")
        );
    }

    #[test]
    fn test_safe_actions_pass_silently() {
        let mut alfred = AlfredProtocol::new();
        let adv = alfred.evaluate_action("Run cargo test --workspace", "all green");
        assert!(adv.is_none());
    }
}
