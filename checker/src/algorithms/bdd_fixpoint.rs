use crate::core::kripke_structure::{KripkeStructure, StateID};
use crate::modeling::expansion::eval;
use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};
use fixedbitset::FixedBitSet;
use std::collections::HashMap;

use crate::core::bdd::{
    SymbolicContext, bdd_number_eq, bdd_number_gt, bdd_number_gte, bdd_number_lt, bdd_number_lte,
    bdd_number_neq, bdd_number_sub, ripple_carry_adder, substitute_var_with_future_vars,
};
use crate::modeling::symbolic::{
    BinaryOp, Domain, Model, SymbolicArena, SymbolicExpr, SymbolicExprID, UnaryOp, Value,
};

use crate::modeling::bdd_compiler::{compile_model_to_bdd, eval_expr};

use oxidd::bdd::BDDFunction;
use oxidd::bdd::BDDManagerRef;
use oxidd::{
    BooleanFunction, BooleanFunctionQuant, BooleanOperator, FunctionSubst, Manager, ManagerRef,
    Subst, Substitution,
};

pub struct SymbolicSatProvider {
    pub bdds: Vec<BDDFunction>,
}

impl SymbolicSatProvider {
    pub fn new(num_formulas: usize) -> Self {
        Self {
            bdds: Vec::with_capacity(num_formulas),
        }
    }

    pub fn set_bdd_for_formula(&mut self, formula_id: FormulaID, bdd: BDDFunction) {
        if (formula_id.0 as usize) < self.bdds.len() {
            self.bdds[formula_id.0 as usize] = bdd;
        } else {
            self.bdds.push(bdd);
        }
    }

    pub fn get_bdd_for_formula(&self, formula_id: FormulaID) -> &BDDFunction {
        &self.bdds[formula_id.0 as usize]
    }
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
fn sat(
    f_id: FormulaID,
    symbolic_ctx: &SymbolicContext,
    model: &Model,
    provider: &mut SymbolicSatProvider,
) {
    let formula = model.ctl_arena.get(f_id);

    match formula {
        CtlFormula::True => {
            let bdd = symbolic_ctx
                .manager
                .with_manager_shared(|m| BDDFunction::t(m));

            provider.set_bdd_for_formula(f_id, bdd);
        }
        CtlFormula::False => {
            let bdd = symbolic_ctx
                .manager
                .with_manager_shared(|m| BDDFunction::f(m));
            provider.set_bdd_for_formula(f_id, bdd);
        }

        CtlFormula::Prop(sym_expr_id) => {
            let bdd_expr_vec = eval_expr(symbolic_ctx, *sym_expr_id, model);
            let bdd_expr = bdd_expr_vec.into_iter().next().unwrap(); //A propostiion always has a single BDD expression (boolean expression)
            provider.set_bdd_for_formula(f_id, bdd_expr);
        }

        CtlFormula::Not(sf) => {
            let bdd = provider.get_bdd_for_formula(*sf);
            let bdd = symbolic_ctx.manager.with_manager_shared(|_| bdd.not());
            provider.set_bdd_for_formula(f_id, bdd.unwrap());
        }

        CtlFormula::And(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            let bdd = symbolic_ctx.manager.with_manager_shared(|_| bdd1.and(bdd2));
            provider.set_bdd_for_formula(f_id, bdd.unwrap());
        }

        CtlFormula::Or(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            let bdd = symbolic_ctx.manager.with_manager_shared(|_| bdd1.or(bdd2));
            provider.set_bdd_for_formula(f_id, bdd.unwrap());
        }

        CtlFormula::Imply(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            let bdd = symbolic_ctx.manager.with_manager_shared(|_| bdd1.imp(bdd2));
            provider.set_bdd_for_formula(f_id, bdd.unwrap());
        }
        CtlFormula::Iff(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            let bdd = symbolic_ctx
                .manager
                .with_manager_shared(|_| bdd1.equiv(bdd2));
            provider.set_bdd_for_formula(f_id, bdd.unwrap());
        }

        // Add label if there is a neighbor that satisfies the subformula
        CtlFormula::EX(child) => {
            let child_bdd = provider.get_bdd_for_formula(*child);
            let subst_child_bdd = symbolic_ctx.shift_curr_to_next(child_bdd);

            let delta = symbolic_ctx
                .transition_relation
                .as_ref()
                .expect("Transition relation not compiled")
                .clone();

            let bdd = symbolic_ctx.manager.with_manager_shared(|_| {
                delta.apply_exists(
                    BooleanOperator::And,
                    &subst_child_bdd,
                    &symbolic_ctx.next_vars_cube,
                )
            });

            provider.set_bdd_for_formula(f_id, bdd.unwrap());
        }

        // Add label if all neighbors satisfy the subformula
        // Add label if there is a path from a state that satisfies f1 to a state that satisfies f2
        CtlFormula::EU(f1, f2) => {}

        CtlFormula::AF(sf) => {}

        // Add label if all paths from a state satisfy f in some future
        _ => panic!("Error: Operator {:?} should be converted!", formula),
    }
}

pub fn verify(symbolic_ctx: &SymbolicContext, mut model: Model) -> Vec<bool> {
    purify_model_specs(&mut model);

    let num_formulas = model.ctl_arena.len();
    let mut provider = SymbolicSatProvider::new(num_formulas);

    for f_idx in 0..num_formulas {
        let f_id = FormulaID(f_idx as u32);
        //label_formula(f_id, structure, &model, &mut provider);
    }

    let mut results = Vec::new();

    /*
    for &spec_id in &model.specs {
        if let Some(marks_bitset) = provider.get_states_for_formula(spec_id) {
            let initial = structure.get_initial_states();
            let mut diff = initial.clone();
            diff.difference_with(marks_bitset);

            results.push(diff.count_ones(..) == 0);
        } else {
            results.push(false);
        }
    }*/

    results
}
