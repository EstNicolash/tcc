use crate::formula::{self, CtlFormula};
use crate::kripke_structure::KripkeStructure;
use petgraph::Direction;
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

pub fn label_formula(
    formula: &CtlFormula,
    structure: &KripkeStructure,
    provider: &mut LabelingProvider,
) {
    match formula {
        CtlFormula::True => {}
        CtlFormula::False => {}
        CtlFormula::Prop(p) => {
            let current_formula = CtlFormula::Prop(p.clone());
            for (state, labels) in &structure.initial_labels {
                if labels.contains(p) {
                    provider.add_label(*state, current_formula.clone());
                }
            }
        }
        CtlFormula::Not(f) => {
            label_formula(f, structure, provider);
            for state in structure.graph.node_indices() {
                if !provider.is_labeled(state, f) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::And(f1, f2) => {
            label_formula(f1, structure, provider);
            label_formula(f2, structure, provider);
            for state in structure.graph.node_indices() {
                if provider.is_labeled(state, f1) && provider.is_labeled(state, f2) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::Or(f1, f2) => {
            label_formula(f1, structure, provider);
            label_formula(f2, structure, provider);
            for state in structure.graph.node_indices() {
                if provider.is_labeled(state, f1) || provider.is_labeled(state, f2) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::Imply(f1, f2) => {
            label_formula(f1, structure, provider);
            label_formula(f2, structure, provider);
            for state in structure.graph.node_indices() {
                if !provider.is_labeled(state, f1) || provider.is_labeled(state, f2) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::EX(f) => {
            label_formula(f, structure, provider);
            for state in structure.graph.node_indices() {
                let has_neighbor_satisfying = structure
                    .graph
                    .neighbors(state)
                    .any(|next_state| provider.is_labeled(next_state, f));

                if has_neighbor_satisfying {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::AX(f) => {
            label_formula(f, structure, provider);
            for state in structure.graph.node_indices() {
                let all_neighbor_satisfy = structure
                    .graph
                    .neighbors(state)
                    .all(|next_state| provider.is_labeled(next_state, f));

                if all_neighbor_satisfy {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::EU(f1, f2) => {
            label_formula(f1, structure, provider);
            label_formula(f2, structure, provider);

            let mut todo: Vec<NodeIndex> = Vec::new();
            for state in structure.graph.node_indices() {
                if provider.is_labeled(state, f2) {
                    todo.push(state);
                    provider.add_label(state, formula.clone());
                }
            }
            while let Some(state) = todo.pop() {
                let predecessors = structure
                    .graph
                    .neighbors_directed(state, Direction::Incoming);

                for pred in predecessors {
                    if provider.is_labeled(pred, f1) && !provider.is_labeled(pred, &formula) {
                        provider.add_label(pred, formula.clone());
                        todo.push(pred);
                    }
                }
            }
        }
        CtlFormula::AU(box f1, box f2) => {}

        CtlFormula::AF(f) => {
            label_formula(f, structure, provider);

            let mut todo: Vec<NodeIndex> = Vec::new();

            let mut out_degree: std::collections::HashMap<NodeIndex, usize> = structure
                .graph
                .node_indices()
                .map(|s| (s, structure.graph.neighbors(s).count()))
                .collect();

            for state in structure.graph.node_indices() {
                if provider.is_labeled(state, f) {
                    provider.add_label(state, formula.clone());
                    todo.push(state);
                }
            }

            while let Some(state) = todo.pop() {
                for pred in structure
                    .graph
                    .neighbors_directed(state, Direction::Incoming)
                {
                    if provider.is_labeled(pred, &formula) {
                        continue;
                    }
                    if let Some(count) = out_degree.get_mut(&pred) {
                        
                        if *count > 0 {
                            *count -= 1;
                        }

                        if *count == 0 {
                            provider.add_label(pred, formula.clone());
                            todo.push(pred);
                        }
                    }
                }
            }
        }
        CtlFormula::AG(box f) => {}
        CtlFormula::EF(box f) => {}
        CtlFormula::EG(box f) => {}
    }
}
