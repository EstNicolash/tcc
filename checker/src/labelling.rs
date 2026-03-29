use crate::formula::{self, CtlFormula};
use crate::kripke_structure::KripkeStructure;
use petgraph::Direction;
use petgraph::graph::{NodeIndex, NodeWeightsMut};
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

    pub fn remove_label(&mut self, state: NodeIndex, formula: CtlFormula) {
        if let Some(set) = self.marks.get_mut(&formula) {
            set.remove(&state);
        }
    }
}

pub fn convert_equivalence(formula: &CtlFormula) -> CtlFormula {
    match formula {
        // EG f => !AF !f
        CtlFormula::EG(f) => {
            let f_conv = Box::new(convert_equivalence(f));
            CtlFormula::Not(Box::new(CtlFormula::AF(Box::new(CtlFormula::Not(f_conv)))))
        }

        // AG f => !EF !f => !E[true U !f]
        CtlFormula::AG(f) => {
            let f_conv = Box::new(convert_equivalence(f));
            CtlFormula::Not(Box::new(CtlFormula::EU(
                Box::new(CtlFormula::True),
                Box::new(CtlFormula::Not(f_conv)),
            )))
        }

        // EF f => E[true U f]
        CtlFormula::EF(f) => {
            let f_conv = Box::new(convert_equivalence(f));
            CtlFormula::EU(Box::new(CtlFormula::True), f_conv)
        }

        // A[f1 U f2] => !(E[!f2 U (!f1 and !f2)] or EG !f2)
        // Since EG is not core, we expand it here too: EG !f2 => !AF f2
        CtlFormula::AU(f1, f2) => {
            let f1_c = Box::new(convert_equivalence(f1));
            let f2_c = Box::new(convert_equivalence(f2));

            let not_f1 = Box::new(CtlFormula::Not(f1_c));
            let not_f2 = Box::new(CtlFormula::Not(f2_c.clone()));

            CtlFormula::Not(Box::new(CtlFormula::Or(
                Box::new(CtlFormula::EU(
                    not_f2,
                    Box::new(CtlFormula::And(
                        not_f1,
                        Box::new(CtlFormula::Not(f2_c.clone())),
                    )),
                )),
                Box::new(CtlFormula::Not(Box::new(CtlFormula::AF(f2_c)))),
            )))
        }

        CtlFormula::Not(f) => CtlFormula::Not(Box::new(convert_equivalence(f))),
        CtlFormula::And(f1, f2) => CtlFormula::And(
            Box::new(convert_equivalence(f1)),
            Box::new(convert_equivalence(f2)),
        ),
        CtlFormula::Or(f1, f2) => CtlFormula::Or(
            Box::new(convert_equivalence(f1)),
            Box::new(convert_equivalence(f2)),
        ),
        CtlFormula::Imply(f1, f2) => CtlFormula::Imply(
            Box::new(convert_equivalence(f1)),
            Box::new(convert_equivalence(f2)),
        ),
        CtlFormula::EX(f) => CtlFormula::EX(Box::new(convert_equivalence(f))),
        CtlFormula::AX(f) => CtlFormula::AX(Box::new(convert_equivalence(f))),
        CtlFormula::AF(f) => CtlFormula::AF(Box::new(convert_equivalence(f))),
        CtlFormula::EU(f1, f2) => CtlFormula::EU(
            Box::new(convert_equivalence(f1)),
            Box::new(convert_equivalence(f2)),
        ),

        // Base cases (True, False, Prop)
        _ => formula.clone(),
    }
}

fn label_formula(
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
            for state in structure.get_all_states() {
                if !provider.is_labeled(state, f) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::And(f1, f2) => {
            label_formula(f1, structure, provider);
            label_formula(f2, structure, provider);
            for state in structure.get_all_states() {
                if provider.is_labeled(state, f1) && provider.is_labeled(state, f2) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::Or(f1, f2) => {
            label_formula(f1, structure, provider);
            label_formula(f2, structure, provider);
            for state in structure.get_all_states() {
                if provider.is_labeled(state, f1) || provider.is_labeled(state, f2) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::Imply(f1, f2) => {
            label_formula(f1, structure, provider);
            label_formula(f2, structure, provider);
            for state in structure.get_all_states() {
                if !provider.is_labeled(state, f1) || provider.is_labeled(state, f2) {
                    provider.add_label(state, formula.clone());
                }
            }
        }
        CtlFormula::EX(f) => {
            label_formula(f, structure, provider);
            for state in structure.get_all_states() {
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
            for state in structure.get_all_states() {
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
            for state in structure.get_all_states() {
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

        CtlFormula::AF(f) => {
            label_formula(f, structure, provider);

            let mut todo: Vec<NodeIndex> = Vec::new();

            let mut out_degree: std::collections::HashMap<NodeIndex, usize> = structure
                .get_all_states()
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
        _ => panic!("Error: Operator {:?} should be converted!", formula),
    }
}
fn verify(formula: CtlFormula, structure: &KripkeStructure) -> bool {
    let mut provider = LabelingProvider::new();

    let canonical_formula = convert_equivalence(&formula);

    label_formula(&canonical_formula, structure, &mut provider);

    structure
        .initial_states
        .iter()
        .all(|&s| provider.is_labeled(s, &canonical_formula))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to create a Propositional formula for testing
    fn prop(name: &str) -> CtlFormula {
        CtlFormula::Prop(name.to_string())
    }

    /// Helper function to wrap a formula in a Not operator
    fn not(f: CtlFormula) -> CtlFormula {
        CtlFormula::Not(Box::new(f))
    }

    /// Helper function for the True constant
    fn true_f() -> CtlFormula {
        CtlFormula::True
    }

    #[test]
    fn test_simple_prop_stay_same() {
        // Atomic propositions should remain unchanged after conversion
        let f = prop("p");
        assert_eq!(convert_equivalence(&f), prop("p"));
    }

    #[test]
    fn test_ef_conversion() {
        // Semantic equivalence: EF p  <=>  E [ true U p ]
        let f = CtlFormula::EF(Box::new(prop("p")));
        let expected = CtlFormula::EU(Box::new(true_f()), Box::new(prop("p")));

        assert_eq!(convert_equivalence(&f), expected);
    }

    #[test]
    fn test_ag_conversion() {
        // Semantic equivalence: AG p  <=>  ! EF !p  <=>  ! E [ true U !p ]
        // This also tests if the conversion is recursive (converting the internal EF)
        let f = CtlFormula::AG(Box::new(prop("p")));

        let expected = not(CtlFormula::EU(Box::new(true_f()), Box::new(not(prop("p")))));

        assert_eq!(convert_equivalence(&f), expected);
    }

    #[test]
    fn test_nested_conversion() {
        // Deep recursion test: AG(EF p)
        // The result should contain NO 'AG' or 'EF' operators
        let f = CtlFormula::AG(Box::new(CtlFormula::EF(Box::new(prop("p")))));
        let converted = convert_equivalence(&f);

        let debug_str = format!("{:?}", converted);

        // Assert that the forbidden operators were removed from the entire tree
        assert!(
            !debug_str.contains("AG"),
            "Result still contains AG: {:?}",
            converted
        );
        assert!(
            !debug_str.contains("EF"),
            "Result still contains EF: {:?}",
            converted
        );
    }

    #[test]
    fn test_imply_conversion_recursive() {
        // Test if conversion works inside an implication
        // Imply(EF p, q) => Imply(E[true U p], q)
        let f = CtlFormula::Imply(
            Box::new(CtlFormula::EF(Box::new(prop("p")))),
            Box::new(prop("q")),
        );
        let converted = convert_equivalence(&f);

        if let CtlFormula::Imply(f1, _) = converted {
            // Check if the left side of the implication was correctly converted
            assert!(matches!(*f1, CtlFormula::EU(_, _)));
        } else {
            panic!("Formula is no longer an Implication!");
        }
    }
}
