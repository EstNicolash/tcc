use crate::core::kripke_structure::{KripkeStructure, StateID};
use crate::modeling::expansion::eval;
use crate::modeling::symbolic::Model;
use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};
use fixedbitset::FixedBitSet;
use std::collections::HashMap;

/// The `LabelingProvider` acts as a centralized truth table for the Model Checking process.
/// It stores which states (StateID) satisfy which subformulas (FormulaID).
pub struct LabelingProvider {
    /// marks[formula_id.0] = { s ∈ S | s ⊨ φ }
    /// Represented as a FixedBitSet where the bit index is the StateID.
    pub marks: Vec<FixedBitSet>,
    num_states: usize,
}

impl LabelingProvider {
    /// Creates a new LabelingProvider.
    /// Needs the total number of states to correctly size the bitsets,
    /// and the current size of the formula arena to pre-allocate the outer vector.
    pub fn new(num_states: usize, num_formulas: usize) -> Self {
        Self {
            marks: vec![FixedBitSet::with_capacity(num_states); num_formulas],
            num_states,
        }
    }

    /// Returns the complete FixedBitSet for a given formula.
    /// Extremely useful for O(1) set operations (like bitwise AND for formulas).
    pub fn get_states_for_formula(&self, formula_id: FormulaID) -> Option<&FixedBitSet> {
        self.marks.get(formula_id.0 as usize)
    }

    /*
    pub fn debug_print(&self, num_states: usize, arena: &CtlFormulaArena) {
        println!("\n--- DEBUG: STATE LABELS ---");
        for s in 0..num_states {
            let mut labels = Vec::new();
            for f_idx in 0..self.marks.len() {
                if self.marks[f_idx].contains(s) {
                    let formula = arena.get(FormulaID(f_idx as u32));
                    labels.push(format!("{:?}", formula)); // Ou sua função de format
                }
            }
            println!("State {}: {:?}", s, labels);
        }
        println!("---------------------------\n");
    }
    */
}

/// Converts a CTL formula to its equivalent core form for the labelling algorithm.
///
/// # Arguments
///
/// * `f_id` - The ID of the formula to convert.
/// * `old_arena` - The old arena containing the formula.
/// * `new_arena` - The new arena to insert the converted formula into.
/// * `memo` - A memoization map to avoid recomputing already converted formulas.
///
/// # Returns
///
/// The ID of the converted formula in the new arena.
///
fn convert_to_core<P: Copy + Eq + std::hash::Hash>(
    f_id: FormulaID,
    old_arena: &CtlFormulaArena<P>,
    new_arena: &mut CtlFormulaArena<P>,
    memo: &mut std::collections::HashMap<FormulaID, FormulaID>,
) -> FormulaID {
    if let Some(&new_id) = memo.get(&f_id) {
        return new_id;
    }

    let formula = old_arena.get(f_id);

    let mut conv = |f| convert_to_core(f, old_arena, new_arena, memo);

    let new_id = match formula {
        // EG f => !AF !f
        CtlFormula::EG(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let af_not_f = new_arena.insert(CtlFormula::AF(not_f));
            new_arena.insert(CtlFormula::Not(af_not_f))
        }

        // AG f => !EF !f => !E[true U !f]
        CtlFormula::AG(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let t_id = new_arena.insert(CtlFormula::True);
            let eu = new_arena.insert(CtlFormula::EU(t_id, not_f));
            new_arena.insert(CtlFormula::Not(eu))
        }

        // EF f => E[true U f]
        CtlFormula::EF(f) => {
            let f_c = conv(*f);
            let t_id = new_arena.insert(CtlFormula::True);
            new_arena.insert(CtlFormula::EU(t_id, f_c))
        }

        // AX f => !EX !f
        CtlFormula::AX(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let ex_not_f = new_arena.insert(CtlFormula::EX(not_f));
            new_arena.insert(CtlFormula::Not(ex_not_f))
        }
        // A[f1 U f2] => !(E[!f2 U (!f1 and !f2)] or EG !f2)
        CtlFormula::AU(f1, f2) => {
            let f1_c = conv(*f1);
            let f2_c = conv(*f2);

            let not_f1 = new_arena.insert(CtlFormula::Not(f1_c));
            let not_f2 = new_arena.insert(CtlFormula::Not(f2_c));

            let and_n1_n2 = new_arena.insert(CtlFormula::And(not_f1, not_f2));
            let eu = new_arena.insert(CtlFormula::EU(not_f2, and_n1_n2));

            let af_f2 = new_arena.insert(CtlFormula::AF(f2_c));
            let not_af = new_arena.insert(CtlFormula::Not(af_f2));

            let or_f = new_arena.insert(CtlFormula::Or(eu, not_af));
            new_arena.insert(CtlFormula::Not(or_f))
        }

        // --- Direct Conversions (Only propagate conversion to children) ---
        CtlFormula::Not(f) => {
            let c = conv(*f);
            new_arena.insert(CtlFormula::Not(c))
        }
        CtlFormula::And(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::And(c1, c2))
        }
        CtlFormula::Or(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::Or(c1, c2))
        }
        CtlFormula::Imply(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::Imply(c1, c2))
        }

        CtlFormula::Iff(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::Iff(c1, c2))
        }
        CtlFormula::EX(f) => {
            let c = conv(*f);
            new_arena.insert(CtlFormula::EX(c))
        }
        CtlFormula::AF(f) => {
            let c = conv(*f);
            new_arena.insert(CtlFormula::AF(c))
        }
        CtlFormula::EU(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::EU(c1, c2))
        }

        CtlFormula::True => new_arena.insert(CtlFormula::True),
        CtlFormula::False => new_arena.insert(CtlFormula::False),
        CtlFormula::Prop(p) => new_arena.insert(CtlFormula::Prop(*p)),
    };

    memo.insert(f_id, new_id);
    new_id
}

pub fn purify_model_specs(model: &mut Model) {
    let mut core_arena = CtlFormulaArena::new();
    let mut memo = HashMap::new();
    let mut core_specs = Vec::new();

    for &old_spec_id in &model.specs {
        let new_core_id =
            convert_to_core(old_spec_id, &model.ctl_arena, &mut core_arena, &mut memo);

        core_specs.push(new_core_id);
    }

    model.ctl_arena = core_arena;
    model.specs = core_specs;
}

/// Labels the states in the Kripke structure according to the given CTL formula.
///
/// # Arguments
///
/// * `formula` - The CTL formula to label the states with.
/// * `structure` - The Kripke structure to label the states in.
/// * `provider` - The provider to add labels to the states.
///
fn label_formula(
    f_id: FormulaID,
    structure: &KripkeStructure,
    model: &Model,
    provider: &mut LabelingProvider,
) {
    let formula = model.ctl_arena.get(f_id);
    let num_states = structure.num_states();

    match formula {
        CtlFormula::True => {
            let mut bitset = FixedBitSet::with_capacity(num_states);
            bitset.insert_range(0..num_states);
            provider.marks[f_id.0 as usize] = bitset;
        }
        CtlFormula::False => {
            // False is never labeled in any state
            provider.marks[f_id.0 as usize] = FixedBitSet::with_capacity(num_states);
        }

        // Labels all states with the property if they have the label
        CtlFormula::Prop(sym_expr_id) => {
            let f_idx = f_id.0 as usize;
            let mut bitset = FixedBitSet::with_capacity(num_states);

            for s in 0..num_states {
                let state_id = StateID(s as u32);

                let state_values = structure.states.get_values(state_id);

                let results = eval(*sym_expr_id, state_values, model);

                if results.iter().any(|&v| v != 0) {
                    bitset.insert(s);
                }
            }

            provider.marks[f_idx] = bitset;
        }

        // Just add label if the subformula is not labeled
        CtlFormula::Not(sf) => {
            let f_idx = f_id.0 as usize;

            if let Some(child_marks) = provider.get_states_for_formula(*sf) {
                let mut result = FixedBitSet::with_capacity(num_states);
                result.insert_range(0..num_states);

                result.difference_with(child_marks);

                provider.marks[f_idx] = result;
            }
        }

        // Add label if both subformulas are labeled
        CtlFormula::And(f1, f2) => {
            let f_idx = f_id.0 as usize;

            let Some(f1_marks) = provider.get_states_for_formula(*f1) else {
                return;
            };
            let Some(f2_marks) = provider.get_states_for_formula(*f2) else {
                return;
            };

            let mut result = f1_marks.clone();

            result.intersect_with(&f2_marks);

            provider.marks[f_idx] = result;
        }

        // Add label if either subformula is labeled
        CtlFormula::Or(f1, f2) => {
            let f_idx = f_id.0 as usize;

            let Some(f1_marks) = provider.get_states_for_formula(*f1) else {
                return;
            };
            let Some(f2_marks) = provider.get_states_for_formula(*f2) else {
                return;
            };

            let mut result = f1_marks.clone();
            result.union_with(&f2_marks);

            provider.marks[f_idx] = result;
        }

        // Add label if the first subformula is not labeled or the second is labeled
        CtlFormula::Imply(f1, f2) => {
            let f_idx = f_id.0 as usize;

            let Some(f1_marks) = provider.get_states_for_formula(*f1) else {
                return;
            };
            let Some(f2_marks) = provider.get_states_for_formula(*f2) else {
                return;
            };

            let mut result = f1_marks.clone();
            result.toggle_range(0..num_states);
            result.union_with(&f2_marks);

            provider.marks[f_idx] = result;
        }
        CtlFormula::Iff(f1, f2) => {
            // p <-> q  ≡  (p -> q) & (q -> p)
            let f1_marks = provider.marks[f1.0 as usize].clone();
            let f2_marks = provider.marks[f2.0 as usize].clone();

            let mut imp1 = f1_marks.clone();
            imp1.toggle_range(0..num_states);
            imp1.union_with(&f2_marks);

            // (f2 -> f1): !f2 | f1
            let mut imp2 = f2_marks.clone();
            imp2.toggle_range(0..num_states);
            imp2.union_with(&f1_marks);

            imp1.intersect_with(&imp2);
            provider.marks[f_id.0 as usize] = imp1;
        }

        // Add label if there is a neighbor that satisfies the subformula
        CtlFormula::EX(child) => {
            let f_idx = f_id.0 as usize;

            let Some(child_marks) = provider.get_states_for_formula(*child) else {
                return;
            };

            let mut result = FixedBitSet::with_capacity(num_states);

            for s_idx in child_marks.ones() {
                let state_id = StateID(s_idx as u32);
                let predecessors = structure.get_predecessors(state_id);
                for &pred_id in predecessors {
                    result.insert(pred_id.0 as usize);
                }
            }

            provider.marks[f_idx] = result;
        }

        // Add label if all neighbors satisfy the subformula
        // Add label if there is a path from a state that satisfies f1 to a state that satisfies f2
        CtlFormula::EU(f1, f2) => {
            let f_idx = f_id.0 as usize;

            let f1_marks = provider.marks[f1.0 as usize].clone();
            let f2_marks = provider.marks[f2.0 as usize].clone();

            /*
            println!("DEBUG: Starting EU logic.");
            println!("  Target (f2) marks: {:?}", f2_marks.count_ones(..));
            println!("  Constraint (f1) marks: {:?}", f1_marks.count_ones(..));
            */

            let mut result = f2_marks.clone();

            let mut todo: Vec<StateID> = f2_marks.ones().map(|s| StateID(s as u32)).collect();

            while let Some(state_id) = todo.pop() {
                for &pred in structure.get_predecessors(state_id) {
                    let pred_idx = pred.0 as usize;

                    if f1_marks.contains(pred_idx) && !result.contains(pred_idx) {
                        result.insert(pred_idx);
                        todo.push(pred);

                        // println!("DEBUG: -> EU discovered new state: {}", pred_idx);
                    }
                }
            }

            /*
            println!(
                "DEBUG: EU Fixed-point reached. Total states: {:?}",
                result.count_ones(..)
            );*/

            provider.marks[f_idx] = result;
        }

        CtlFormula::AF(sf) => {
            let f_idx = f_id.0 as usize;
            let child_marks = provider.get_states_for_formula(*sf).unwrap();

            let mut result = FixedBitSet::with_capacity(num_states);
            let mut todo = Vec::new();

            let mut out_degree: Vec<u32> = (0..num_states)
                .map(|s| structure.get_successors(StateID(s as u32)).len() as u32)
                .collect();

            // If the state already satisfies sf, it satisfies AF sf
            for s_idx in child_marks.ones() {
                result.insert(s_idx);
                todo.push(StateID(s_idx as u32));
            }

            // 4. Backward propagation: if all successors of a parent are marked, the parent also enters
            while let Some(state_id) = todo.pop() {
                for &pred in structure.get_predecessors(state_id) {
                    let pred_idx = pred.0 as usize;

                    // If the predecessor is not already marked, check if all successors are marked
                    if !result.contains(pred_idx) {
                        if out_degree[pred_idx] > 0 {
                            out_degree[pred_idx] -= 1;
                        }

                        // If the count is zero, all successors are marked, so the predecessor also enters
                        if out_degree[pred_idx] == 0 {
                            result.insert(pred_idx);
                            todo.push(pred);
                        }
                    }
                }
            }

            provider.marks[f_idx] = result;
        }

        // Add label if all paths from a state satisfy f in some future
        _ => panic!("Error: Operator {:?} should be converted!", formula),
    }
}

pub fn verify(structure: &KripkeStructure, mut model: Model) -> Vec<bool> {
    purify_model_specs(&mut model);

    let num_states = structure.num_states();
    let num_formulas = model.ctl_arena.len();
    let mut provider = LabelingProvider::new(num_states, num_formulas);

    // The formula arena is always ordered by subformulas due to the recursive insertion process.
    for f_idx in 0..num_formulas {
        let f_id = FormulaID(f_idx as u32);
        label_formula(f_id, structure, &model, &mut provider);
    }

    let mut results = Vec::new();

    for &spec_id in &model.specs {
        if let Some(marks_bitset) = provider.get_states_for_formula(spec_id) {
            let initial = structure.get_initial_states();
            let mut diff = initial.clone();
            diff.difference_with(marks_bitset);

            /*
            if !diff.is_empty() {
                println!(
                    "DEBUG FAIL: Spec {:?} failed for initial states: {:?}",
                    spec_id, diff
                );
                println!("  Initial states bits: {:?}", initial);
                println!("  Marks bitset bits:   {:?}", marks_bitset);
            }*/

            results.push(diff.count_ones(..) == 0);
        } else {
            results.push(false);
        }
    }

    results
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_ef_conversion_to_core() {
        let mut old_arena = CtlFormulaArena::new();
        let mut new_arena = CtlFormulaArena::new();
        let mut memo = HashMap::new();

        let p_id = 42;
        let prop = old_arena.insert(CtlFormula::Prop(p_id));
        let ef_p = old_arena.insert(CtlFormula::EF(prop));

        let root_id = convert_to_core(ef_p, &old_arena, &mut new_arena, &mut memo);

        // 3. EU(True, p)
        let root_formula = new_arena.get(root_id);
        if let CtlFormula::EU(f1, f2) = root_formula {
            assert!(matches!(new_arena.get(*f1), CtlFormula::True));
            if let CtlFormula::Prop(p) = new_arena.get(*f2) {
                assert_eq!(*p, 42);
            } else {
                panic!("F2 should be a Prop p");
            }
        } else {
            panic!("EF should be converted to EU");
        }
    }

    #[test]
    fn test_ag_conversion_to_core() {
        let mut old_arena = CtlFormulaArena::new();
        let mut new_arena = CtlFormulaArena::new();
        let mut memo = HashMap::new();

        // 1. AG p
        let prop = old_arena.insert(CtlFormula::Prop(1));
        let ag_p = old_arena.insert(CtlFormula::AG(prop));

        //  AG p => !E[true U !p]
        let root_id = convert_to_core(ag_p, &old_arena, &mut new_arena, &mut memo);

        let f_root = new_arena.get(root_id); // Not(...)
        assert!(matches!(f_root, CtlFormula::Not(_)));

        if let CtlFormula::Not(eu_id) = f_root {
            let f_eu = new_arena.get(*eu_id); // EU(True, Not(p))
            if let CtlFormula::EU(t_id, not_p_id) = f_eu {
                assert!(matches!(new_arena.get(*t_id), CtlFormula::True));
                assert!(matches!(new_arena.get(*not_p_id), CtlFormula::Not(_)));
            } else {
                panic!("Should be EU");
            }
        }
    }
}
