use serde::{Deserialize, Serialize};

/// Minimal MCTS over tool-call prefixes.
/// Node holds visit/wins stats and UCT selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MctsNode {
    pub tool_prefix: Vec<String>,
    pub visits: u32,
    pub wins: f64,
    pub children: Vec<MctsNode>,
    pub is_expanded: bool,
}

impl MctsNode {
    pub fn new(prefix: Vec<String>) -> Self {
        Self { tool_prefix: prefix, visits: 0, wins: 0.0, children: Vec::new(), is_expanded: false }
    }

    pub fn uct(&self, parent_visits: u32) -> f64 {
        if self.visits == 0 { return f64::INFINITY; }
        let exploit = self.wins / self.visits as f64;
        let explore = (2.0 * (parent_visits as f64).ln() / self.visits as f64).sqrt();
        exploit + 1.414 * explore
    }

    pub fn select_best_child(&self) -> Option<usize> {
        if self.children.is_empty() { return None; }
        let pv = self.visits.max(1);
        self.children
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.uct(pv).total_cmp(&b.uct(pv)))
            .map(|(i, _)| i)
    }

    pub fn expand(&mut self, candidates: Vec<Vec<String>>) {
        if self.is_expanded { return; }
        for c in candidates { self.children.push(MctsNode::new(c)); }
        self.is_expanded = true;
    }

    pub fn backprop(&mut self, reward: f64) {
        self.visits += 1;
        self.wins += reward;
    }

    pub fn backprop_path(&mut self, path: &[usize], reward: f64) {
        self.backprop(reward);
        if let Some((&idx, rest)) = path.split_first()
            && let Some(child) = self.children.get_mut(idx) { child.backprop_path(rest, reward); }
    }

    /// Reward: 1.0 if tool output contains success signal, 0.0 on error, 0.5 otherwise.
    pub fn verification_reward(is_error: bool, output: &str) -> f64 {
        if is_error { return 0.0; }
        let lower = output.to_lowercase();
        if lower.contains("passed") || lower.contains("ok") || lower.contains("success") { 1.0 } else { 0.5 }
    }
}

#[derive(Debug, Clone)]
pub struct MctsConfig {
    pub max_rollouts: usize,
    pub max_depth: usize,
}

impl Default for MctsConfig {
    fn default() -> Self { Self { max_rollouts: 4, max_depth: 3 } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uct_unvisited_is_infinite() {
        let n = MctsNode::new(vec!["read".into()]);
        assert!(n.uct(10).is_infinite());
    }

    #[test]
    fn test_uct_exploit_plus_explore() {
        let mut n = MctsNode::new(vec!["bash".into()]);
        n.visits = 10; n.wins = 7.0;
        let v = n.uct(100);
        assert!(v > 0.7 && v < 3.0, "uct={v}");
    }

    #[test]
    fn test_select_best_child_prefers_higher_uct() {
        let mut root = MctsNode::new(vec![]);
        root.visits = 10;
        let mut good = MctsNode::new(vec!["read".into()]);
        good.visits = 5; good.wins = 4.5;
        let mut bad = MctsNode::new(vec!["bash".into()]);
        bad.visits = 5; bad.wins = 0.5;
        root.children = vec![bad, good];
        assert_eq!(root.select_best_child(), Some(1));
    }

    #[test]
    fn test_backprop_path() {
        let mut root = MctsNode::new(vec![]);
        root.expand(vec![vec!["a".into()], vec!["b".into()]]);
        root.backprop_path(&[0], 1.0);
        assert_eq!(root.visits, 1);
        assert_eq!(root.children[0].visits, 1);
        assert_eq!(root.children[1].visits, 0);
        root.backprop_path(&[1], 0.0);
        assert_eq!(root.visits, 2);
        assert_eq!(root.children[1].visits, 1);
    }

    #[test]
    fn test_verification_reward() {
        assert_eq!(MctsNode::verification_reward(true, "anything"), 0.0);
        assert_eq!(MctsNode::verification_reward(false, "tests passed"), 1.0);
        assert_eq!(MctsNode::verification_reward(false, "output ok"), 1.0);
        assert_eq!(MctsNode::verification_reward(false, "some output"), 0.5);
    }

    #[test]
    fn test_expand_idempotent() {
        let mut n = MctsNode::new(vec![]);
        n.expand(vec![vec!["x".into()]]);
        n.expand(vec![vec!["y".into()]]);
        assert_eq!(n.children.len(), 1);
    }
}

#[derive(Debug, Clone, Default)]
pub struct Tier3Flags {
    pub enabled: bool,
    pub mcts_exploration_weight: f64,
}

