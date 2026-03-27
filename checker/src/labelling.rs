use crate::formula::CtlFormula;
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};

/// The `LabelingProvider` acts as a centralized truth table for the Model Checking process.
/// It stores which states (NodeIndex) satisfy which subformulas (CtlFormula).
pub struct LabelingProvider {
    /// marks(φ) = { s ∈ S | s ⊨ φ }
    marks: HashMap<CtlFormula, HashSet<NodeIndex>>,
}

impl LabelingProvider {
    pub fn new() -> Self {
        Self {
            marks: HashMap::new(),
        }
    }

    /// Checks if a specific state satisfies a given formula.
    /// # Arguments
    /// * `state` - The index of the state in the Kripke Structure.
    /// * `formula` - The CTL (sub)formula to check.
    /// Returns `true` if the state is already marked with this formula.
    pub fn is_labeled(&self, state: NodeIndex, formula: &CtlFormula) -> bool {
        self.marks
            .get(formula)
            .map_or(false, |set| set.contains(&state))
    }

    /// Records that a state satisfies a formula.
    /// * `state` - The index of the state to be marked.
    /// * `formula` - The formula that is true in this state.
    pub fn add_label(&mut self, state: NodeIndex, formula: CtlFormula) {
        self.marks.entry(formula).or_default().insert(state);
    }

    /// Returns all states that satisfy a specific formula.
    pub fn get_states_for_formula(&self, formula: &CtlFormula) -> Option<&HashSet<NodeIndex>> {
        self.marks.get(formula)
    }
}
