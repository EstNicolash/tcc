use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};
use std::collections::HashMap;

use crate::core::bdd::SymbolicContext;
use crate::modeling::symbolic::Model;

use crate::modeling::bdd_compiler::eval_expr;

use oxidd::bdd::BDDFunction;
use oxidd::{BooleanFunction, BooleanFunctionQuant, BooleanOperator, ManagerRef};

/// A provider for symbolic SAT queries using BDDs.
/// Just stores the BDDs for each formula for use in SAT queries.
pub struct SymbolicSatProvider {
    /// The BDDs for each formula.
    pub bdds: Vec<BDDFunction>,
}

impl SymbolicSatProvider {
    /// Creates a new `SymbolicSatProvider` with the specified number of formulas.
    pub fn new(num_formulas: usize) -> Self {
        Self {
            bdds: Vec::with_capacity(num_formulas),
        }
    }

    /// Sets the BDD for the specified formula.
    ///
    /// # Arguments
    ///
    /// * `formula_id` - The ID of the formula to set the BDD for.
    /// * `bdd` - The BDD representation of the formula.

    pub fn set_bdd_for_formula(&mut self, formula_id: FormulaID, bdd: BDDFunction) {
        let idx = formula_id.0 as usize;
        if idx >= self.bdds.len() {
            self.bdds.resize(idx + 1, bdd.clone());
        }
        self.bdds[idx] = bdd;
    }

    /// Returns a reference to the BDD for the specified formula.
    ///
    /// # Arguments
    ///
    /// * `formula_id` - The ID of the formula to get the BDD for.
    ///
    /// # Panics
    ///
    /// Panics if the formula BDD has not been set.
    pub fn get_bdd_for_formula(&self, formula_id: FormulaID) -> &BDDFunction {
        self.bdds
            .get(formula_id.0 as usize)
            .expect("Attempted to access a formula BDD that was never set.")
    }
}
/// Converts a CTL formula to its equivalent core form for the bdd algorithm.
///
///
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

/// Constructs the BDD representing the set of states that satisfy a CTL formula (Sat(f)).
///
/// # Arguments
/// * `f_id` - The unique identifier of the CTL formula in the arena.
/// * `symbolic_ctx` - The context providing BDD variables and transition relations.
/// * `model` - The system model containing the formula arena.
/// * `provider` - A memoization store for subformula BDDs.
///
/// # Errors
/// Returns an error if BDD operations fail (e.g., Out of Memory) or if the
/// transition relation is missing.
/// Constructs a BDD (Sat(f)) for the given CTL formula.
///
/// This implements the recursive symbolic labelling algorithm.
///
/// # Arguments
/// * `f_id` - The ID of the CTL formula to construct the BDD for.
/// * `symbolic_ctx` - The symbolic context containing the BDD manager.
/// * `model` - The system model.
/// * `provider` - The provider to store and retrieve subformula BDDs.
fn sat(
    f_id: FormulaID,
    symbolic_ctx: &SymbolicContext,
    model: &Model,
    provider: &mut SymbolicSatProvider,
) -> Result<(), String> {
    let formula = model.ctl_arena.get(f_id);

    // Each match arm MUST return Result<BDDFunction, String>
    let bdd = match formula {
        CtlFormula::True => Ok(symbolic_ctx
            .manager
            .with_manager_shared(|m| BDDFunction::t(m))),

        CtlFormula::False => Ok(symbolic_ctx
            .manager
            .with_manager_shared(|m| BDDFunction::f(m))),

        CtlFormula::Prop(sym_expr_id) => {
            let bdd_expr_vec = eval_expr(symbolic_ctx, *sym_expr_id, model);
            bdd_expr_vec
                .into_iter()
                .next()
                .ok_or_else(|| format!("Empty expression for proposition {:?}", sym_expr_id))
        }

        CtlFormula::Not(sf) => {
            let inner = provider.get_bdd_for_formula(*sf);
            symbolic_ctx
                .manager
                .with_manager_shared(|_| inner.not())
                .map_err(|e| format!("OOM during NOT operation: {:?}", e))
        }

        CtlFormula::And(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            symbolic_ctx
                .manager
                .with_manager_shared(|_| bdd1.and(bdd2))
                .map_err(|e| format!("OOM during AND operation: {:?}", e))
        }

        CtlFormula::Or(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            symbolic_ctx
                .manager
                .with_manager_shared(|_| bdd1.or(bdd2))
                .map_err(|e| format!("OOM during OR operation: {:?}", e))
        }

        CtlFormula::Imply(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            symbolic_ctx
                .manager
                .with_manager_shared(|_| bdd1.imp(bdd2))
                .map_err(|e| format!("OOM during IMPLY operation: {:?}", e))
        }

        CtlFormula::Iff(f1, f2) => {
            let bdd1 = provider.get_bdd_for_formula(*f1);
            let bdd2 = provider.get_bdd_for_formula(*f2);
            symbolic_ctx
                .manager
                .with_manager_shared(|_| bdd1.equiv(bdd2))
                .map_err(|e| format!("OOM during IFF operation: {:?}", e))
        }

        CtlFormula::EX(child) => {
            let child_bdd = provider.get_bdd_for_formula(*child);
            let subst_child_bdd = symbolic_ctx.shift_curr_to_next(child_bdd);

            let delta = symbolic_ctx
                .transition_relation
                .as_ref()
                .ok_or_else(|| "Transition relation not compiled".to_string())?;

            // Compute EX(sat_f) using existential quantification over next-state variables
            symbolic_ctx
                .manager
                .with_manager_shared(|_| {
                    delta.apply_exists(
                        BooleanOperator::And,
                        &subst_child_bdd,
                        &symbolic_ctx.next_vars_cube,
                    )
                })
                .map_err(|e| format!("OOM during EX operation: {:?}", e))
        }

        CtlFormula::EU(f1, f2) => {
            let mut sat_f = provider.get_bdd_for_formula(*f2).clone(); // f0 = B (initial seed)
            let sat_f1 = provider.get_bdd_for_formula(*f1);

            let delta = symbolic_ctx
                .transition_relation
                .as_ref()
                .ok_or_else(|| "Transition relation not compiled".to_string())?;

            // Least Fixed Point iteration for EU
            loop {
                let subst_sat_f = symbolic_ctx.shift_curr_to_next(&sat_f);

                // Compute EX(sat_f)
                let ex_sat_f = delta
                    .apply_exists(
                        BooleanOperator::And,
                        &subst_sat_f,
                        &symbolic_ctx.next_vars_cube,
                    )
                    .map_err(|e| format!("OOM during EU apply_exists: {:?}", e))?;

                // Fixed point step: f_{j+1} = f_j | (f1 & EX(f_j))
                let next_sat = sat_f1
                    .and(&ex_sat_f)
                    .map_err(|e| format!("OOM during EU and: {:?}", e))?
                    .or(&sat_f)
                    .map_err(|e| format!("OOM during EU or: {:?}", e))?;

                if next_sat == sat_f {
                    break;
                }
                sat_f = next_sat;
            }
            Ok(sat_f)
        }

        CtlFormula::EG(sf) => {
            let mut sat_f = provider.get_bdd_for_formula(*sf).clone();

            let delta = symbolic_ctx
                .transition_relation
                .as_ref()
                .ok_or_else(|| "Transition relation not compiled".to_string())?;

            // Greatest Fixed Point iteration for EG
            loop {
                let subst_sat_f = symbolic_ctx.shift_curr_to_next(&sat_f);

                // Compute EX(sat_f)
                let ex_sat_f = delta
                    .apply_exists(
                        BooleanOperator::And,
                        &subst_sat_f,
                        &symbolic_ctx.next_vars_cube,
                    )
                    .map_err(|e| format!("OOM during EG apply_exists: {:?}", e))?;

                // Fixed point step: f_{j+1} = f_j & EX(f_j)
                let next_sat = sat_f
                    .and(&ex_sat_f)
                    .map_err(|e| format!("OOM during EG and: {:?}", e))?;

                if next_sat == sat_f {
                    break;
                }

                sat_f = next_sat;
            }
            Ok(sat_f)
        }

        _ => {
            return Err(format!(
                "Error: Operator {:?} should be converted!",
                formula
            ));
        }
    }?; // The final '?' unpacks the Result into a bare BDDFunction

    provider.set_bdd_for_formula(f_id, bdd);
    Ok(())
}

/// Verifies the model using BDD fixpoint iteration.
///
/// # Arguments
///
/// * `symbolic_ctx` - The symbolic context.
/// * `model` - The model to verify.
///
/// # Returns
///
/// A vector of boolean results, one for each specification.
pub fn verify(symbolic_ctx: &SymbolicContext, mut model: Model) -> Result<Vec<bool>, String> {
    purify_model_specs(&mut model);

    let num_formulas = model.ctl_arena.len();
    let mut provider = SymbolicSatProvider::new(num_formulas);

    // The formula arena is always ordered by subformulas due to the recursive insertion process.
    (0..num_formulas).try_for_each(|f_idx| {
        let f_id = FormulaID(f_idx as u32);
        sat(f_id, symbolic_ctx, &model, &mut provider)
    })?;

    let initial_bdd = symbolic_ctx
        .initial_states
        .as_ref()
        .ok_or_else(|| "Error: initial states not compiled".to_string())?;

    // All initial states must satisfy the formula (I ⊆ Sat(formula)).
    // Therefore, the intersection between the initial states and the violating states (!Sat(formula)) must be empty.
    // Property holds if no initial state violates the specification: I ∩ ¬Sat(formula) = ∅.
    model
        .specs
        .iter()
        .map(|&spec_id| {
            let sat_bdd = provider.get_bdd_for_formula(spec_id);

            symbolic_ctx.manager.with_manager_shared(|_| {
                let not_sat = sat_bdd.not().map_err(|_| "OOM: not")?;

                let violation_set = initial_bdd
                    .and(&not_sat)
                    .map_err(|_| "OOM during verification intersection".to_string())?;

                Ok(!violation_set.satisfiable())
            })
        })
        .collect::<Result<Vec<bool>, String>>()
}
