use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};
use std::collections::HashMap;

use crate::core::bdd::SymbolicContext;
use crate::modeling::symbolic::Model;

use crate::modeling::bdd_compiler::eval_expr;

use oxidd::bdd::BDDFunction;
use oxidd::{BooleanFunction, BooleanFunctionQuant, BooleanOperator, ManagerRef};

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
/// Converts a CTL formula to its equivalent core form for the bdd algorithm.
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

/// Construct a BDD (Sat(f)) for the given CTL formula.
///
/// # Arguments
///
/// * `f_id` - The ID of the CTL formula to construct the BDD for.
/// * `symbolic_ctx` - The symbolic context containing the BDD manager.
/// * `model` - The model
/// * `provider` - The provider to add the BDD to.
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

        CtlFormula::EU(f1, f2) => {
            let mut sat_f = provider.get_bdd_for_formula(*f2).clone(); // f0 = B
            let sat_f1 = provider.get_bdd_for_formula(*f1).clone();

            let delta = symbolic_ctx.transition_relation.as_ref().expect("").clone();

            loop {
                let subst_sat_f = symbolic_ctx.shift_curr_to_next(&sat_f);

                // EX(sat_f)
                let ex_sat_f = delta
                    .apply_exists(
                        BooleanOperator::And,
                        &subst_sat_f,
                        &symbolic_ctx.next_vars_cube,
                    )
                    .expect("OOM: apply_exists");

                // f_{j+1} = f_j | (C & EX(f_j))
                let next_sat = sat_f1
                    .and(&ex_sat_f)
                    .expect("OOM: and")
                    .or(&sat_f)
                    .expect("OOM: or");

                if next_sat == sat_f {
                    break;
                }
                sat_f = next_sat;
            }
            provider.set_bdd_for_formula(f_id, sat_f);
        }

        CtlFormula::EG(sf) => {
            let mut sat_f = provider.get_bdd_for_formula(*sf).clone();

            let delta = symbolic_ctx
                .transition_relation
                .as_ref()
                .expect("Transition relation not compiled")
                .clone();

            loop {
                let subst_sat_f = symbolic_ctx.shift_curr_to_next(&sat_f);

                // 1.EX(sat_f)
                let ex_sat_f = delta
                    .apply_exists(
                        BooleanOperator::And,
                        &subst_sat_f,
                        &symbolic_ctx.next_vars_cube,
                    )
                    .expect("OOM: apply_exists");

                let next_sat = sat_f.and(&ex_sat_f).expect("OOM: and");

                if next_sat == sat_f {
                    break;
                }

                sat_f = next_sat;
            }

            provider.set_bdd_for_formula(f_id, sat_f);
        }

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
        sat(f_id, symbolic_ctx, &model, &mut provider);
    }

    let mut results = Vec::new();

    for &spec_id in &model.specs {
        let sat_bdd = provider.get_bdd_for_formula(spec_id);

        let initial_bdd = symbolic_ctx
            .initial_states
            .as_ref()
            .expect("Error: initial states not compiled");

        let holds = symbolic_ctx.manager.with_manager_shared(|_| {
            let not_sat = sat_bdd.not().expect("OOM: not");

            let violation_set = initial_bdd.and(&not_sat).expect("OOM: and");

            !violation_set.satisfiable()
        });

        results.push(holds);
    }

    results
}
