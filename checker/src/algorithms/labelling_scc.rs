use crate::algorithms::labelling::LabelingProvider;
use crate::core::kripke_structure::{KripkeStructure, StateID};
use crate::modeling::expansion::eval;
use crate::modeling::symbolic::{Model, SymbolicArena};
use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};
use fixedbitset::FixedBitSet;
use std::collections::{HashMap, VecDeque};

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

    let core_formula = match formula {
        // AG f => !E[true U !f]
        CtlFormula::AG(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let true_id = new_arena.insert(CtlFormula::True);
            let eu = new_arena.insert(CtlFormula::EU(true_id, not_f));
            CtlFormula::Not(eu)
        }

        // EF f => E[true U f]
        CtlFormula::EF(f) => {
            let f_c = conv(*f);
            let true_id = new_arena.insert(CtlFormula::True);
            CtlFormula::EU(true_id, f_c)
        }

        // AX f => !EX !f
        CtlFormula::AX(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let ex = new_arena.insert(CtlFormula::EX(not_f));
            CtlFormula::Not(ex)
        }

        // AF f => !EG !f
        CtlFormula::AF(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let eg = new_arena.insert(CtlFormula::EG(not_f));
            CtlFormula::Not(eg)
        }

        // A[f1 U f2] => !(E[!f2 U (!f1 and !f2)] or EG !f2)
        CtlFormula::AU(f1, f2) => {
            let (f1_c, f2_c) = (conv(*f1), conv(*f2));
            let not_f1 = new_arena.insert(CtlFormula::Not(f1_c));
            let not_f2 = new_arena.insert(CtlFormula::Not(f2_c));

            let inner_and = new_arena.insert(CtlFormula::And(not_f1, not_f2));
            let eu = new_arena.insert(CtlFormula::EU(not_f2, inner_and));
            let eg = new_arena.insert(CtlFormula::EG(not_f2));

            let or_f = new_arena.insert(CtlFormula::Or(eu, eg));
            CtlFormula::Not(or_f)
        }

        CtlFormula::Not(f) => CtlFormula::Not(conv(*f)),
        CtlFormula::EX(f) => CtlFormula::EX(conv(*f)),
        CtlFormula::EG(f) => CtlFormula::EG(conv(*f)),
        CtlFormula::And(f1, f2) => CtlFormula::And(conv(*f1), conv(*f2)),
        CtlFormula::Or(f1, f2) => CtlFormula::Or(conv(*f1), conv(*f2)),
        CtlFormula::EU(f1, f2) => CtlFormula::EU(conv(*f1), conv(*f2)),
        CtlFormula::Imply(f1, f2) => CtlFormula::Imply(conv(*f1), conv(*f2)),
        CtlFormula::Iff(f1, f2) => CtlFormula::Iff(conv(*f1), conv(*f2)),

        CtlFormula::True => CtlFormula::True,
        CtlFormula::False => CtlFormula::False,
        CtlFormula::Prop(p) => CtlFormula::Prop(*p),
    };

    let new_id = new_arena.insert(core_formula);
    memo.insert(f_id, new_id);
    new_id
}
fn purify_model_specs(model: &mut Model) {
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

struct TarjanContext {
    indices: Vec<Option<usize>>,
    lowlinks: Vec<usize>,
    stack: Vec<StateID>,
    on_stack: Vec<bool>,
    next_index: usize,
    sccs: Vec<Vec<StateID>>,
}

fn strong_connect(
    structure: &KripkeStructure,
    f_sat: &FixedBitSet,
    u: StateID,
    ctx: &mut TarjanContext,
) {
    let u_vec_idx = u.0 as usize;
    ctx.indices[u_vec_idx] = Some(ctx.next_index);
    ctx.lowlinks[u_vec_idx] = ctx.next_index;
    ctx.stack.push(u);
    ctx.on_stack[u_vec_idx] = true;
    ctx.next_index += 1;

    for &v in structure.get_successors(u) {
        if !f_sat.contains(v.0 as usize) {
            continue;
        }
        let v_vec_idx = v.0 as usize;
        if ctx.indices[v_vec_idx].is_none() {
            strong_connect(structure, f_sat, v, ctx);
            ctx.lowlinks[u_vec_idx] = ctx.lowlinks[u_vec_idx].min(ctx.lowlinks[v_vec_idx]);
        } else if ctx.on_stack[v_vec_idx] {
            ctx.lowlinks[u_vec_idx] = ctx.lowlinks[u_vec_idx].min(ctx.indices[v_vec_idx].unwrap());
        }
    }

    if ctx.lowlinks[u_vec_idx] == ctx.indices[u_vec_idx].unwrap() {
        let mut scc = Vec::new();
        loop {
            let v = ctx.stack.pop().unwrap();
            let v_vec_idx = v.0 as usize;
            scc.push(v);
            ctx.on_stack[v_vec_idx] = false;
            if v == u {
                break;
            }
        }
        ctx.sccs.push(scc);
    }
}

pub fn tarjan_scc(structure: &KripkeStructure, f_sat: &FixedBitSet) -> Vec<Vec<StateID>> {
    let mut ctx = TarjanContext {
        indices: vec![None; structure.num_states()],
        lowlinks: vec![0; structure.num_states()],
        stack: Vec::with_capacity(structure.num_states()),
        on_stack: vec![false; structure.num_states()],
        next_index: 0,
        sccs: Vec::new(),
    };

    for s in f_sat.ones() {
        if ctx.indices[s].is_none() {
            strong_connect(structure, f_sat, StateID(s as u32), &mut ctx);
        }
    }

    ctx.sccs
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

    // No caso CtlFormula::EU(f1, f2)
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

        CtlFormula::EG(sf) => {
            let sf_idx = sf.0 as usize;
            let sf_sat = &provider.marks[sf_idx];
            let all_sccs = tarjan_scc(structure, &sf_sat);
            let mut eg_sat = FixedBitSet::with_capacity(structure.num_states());
            let mut queue = Vec::new();

            for scc in all_sccs {
                let is_nontrivial = if scc.len() > 1 {
                    true
                } else {
                    // Check if the the "trivial" SCC has a self-loop, if so, it's nontrivial
                    let u = scc[0];
                    structure
                        .get_successors(u)
                        .iter()
                        .any(|&v| v == u && sf_sat.contains(v.0 as usize))
                };

                // If the SCC is nontrivial, mark all states and enqueue them for processing
                if is_nontrivial {
                    for state in scc {
                        eg_sat.insert(state.0 as usize);
                        queue.push(state);
                    }
                }
            }

            //Backpropagation
            while let Some(state) = queue.pop() {
                for &pred in structure.get_predecessors(state) {
                    if !eg_sat.contains(pred.0 as usize) && sf_sat.contains(pred.0 as usize) {
                        eg_sat.insert(pred.0 as usize);
                        queue.push(pred);
                    }
                }
            }

            provider.marks[f_id.0 as usize] = eg_sat;
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
    fn test_af_conversion_to_core_scc() {
        let mut old_arena = CtlFormulaArena::new();
        let mut new_arena = CtlFormulaArena::new();
        let mut memo = HashMap::new();

        let p_id = 1;
        let prop = old_arena.insert(CtlFormula::Prop(p_id));
        let af_p = old_arena.insert(CtlFormula::AF(prop));

        let root_id = convert_to_core(af_p, &old_arena, &mut new_arena, &mut memo);

        let f_root = new_arena.get(root_id);
        if let CtlFormula::Not(eg_id) = f_root {
            let f_eg = new_arena.get(*eg_id);
            if let CtlFormula::EG(not_p_id) = f_eg {
                let f_not_p = new_arena.get(*not_p_id);
                if let CtlFormula::Not(p_id_inner) = f_not_p {
                    if let CtlFormula::Prop(p) = new_arena.get(*p_id_inner) {
                        assert_eq!(*p, 1);
                    } else {
                        panic!("Inner element should be a Prop(1)");
                    }
                } else {
                    panic!("Inner element should be a Not(p)");
                }
            } else {
                panic!("Should be an EG operator");
            }
        } else {
            panic!("AF conversion should start with a Not operator");
        }
    }

    #[test]
    fn test_au_conversion_to_core_scc() {
        let mut old_arena = CtlFormulaArena::new();
        let mut new_arena = CtlFormulaArena::new();
        let mut memo = HashMap::new();

        let p = old_arena.insert(CtlFormula::Prop(1));
        let q = old_arena.insert(CtlFormula::Prop(2));
        let au_pq = old_arena.insert(CtlFormula::AU(p, q));

        let root_id = convert_to_core(au_pq, &old_arena, &mut new_arena, &mut memo);

        let f_root = new_arena.get(root_id); // Not(...)
        assert!(matches!(f_root, CtlFormula::Not(_)));

        if let CtlFormula::Not(or_id) = f_root {
            let f_or = new_arena.get(*or_id); // Or(EU, EG)
            if let CtlFormula::Or(eu_id, eg_id) = f_or {
                // Verify EG part: EG(!q)
                if let CtlFormula::EG(not_q_id) = new_arena.get(*eg_id) {
                    assert!(matches!(new_arena.get(*not_q_id), CtlFormula::Not(_)));
                } else {
                    panic!("Should contain an EG as part of the AU negation");
                }

                // Verify EU part: E[!q U (!p and !q)]
                assert!(matches!(new_arena.get(*eu_id), CtlFormula::EU(_, _)));
            } else {
                panic!("AU should be converted to a negation of an OR expression");
            }
        }
    }

    #[test]
    fn test_passthrough_core_operators() {
        let mut old_arena = CtlFormulaArena::new();
        let mut new_arena = CtlFormulaArena::new();
        let mut memo = HashMap::new();

        let p = old_arena.insert(CtlFormula::Prop(100));
        let eg_p = old_arena.insert(CtlFormula::EG(p));

        let root_id = convert_to_core(eg_p, &old_arena, &mut new_arena, &mut memo);

        let f_root = new_arena.get(root_id);
        if let CtlFormula::EG(inner_id) = f_root {
            if let CtlFormula::Prop(val) = new_arena.get(*inner_id) {
                assert_eq!(*val, 100);
            } else {
                panic!("Child should be Prop(100)");
            }
        } else {
            panic!("EG should be preserved as a core operator");
        }
    }
}
