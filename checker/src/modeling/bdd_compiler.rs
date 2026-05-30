//! # Module `bdd_compiler`
//!
//! This module provides utilities for compiling a model to a BDD.
//!
//! # Public Functions
//!
//! * [`compile_model_to_bdd`] - Compiles the model to a BDD.
//!

use crate::core::bdd::{
    SymbolicContext, bdd_number_eq, bdd_number_gt, bdd_number_gte, bdd_number_lt, bdd_number_lte,
    bdd_number_neq, bdd_number_sub, calc_bits, ripple_carry_adder,
};
use crate::modeling::symbolic::{BinaryOp, Model, SymbolicExpr, SymbolicExprID, UnaryOp, Value};

use oxidd::bdd::BDDFunction;
use oxidd::{BooleanFunction, ManagerRef};

/// Compiles the model to a BDD
///
/// # Arguments
///
/// * `model` - The model to compile.
/// * `explicit_order` - The explicit order of variables, if any.
///
/// # Returns
///
/// The compiled symbolic context.
pub fn compile_model_to_bdd(model: &Model, explicit_order: Option<Vec<String>>) -> SymbolicContext {
    let mut symbolic_ctx = SymbolicContext::new(model, explicit_order);

    compile_initial_states(model, &mut symbolic_ctx);

    compile_transition_relation(model, &mut symbolic_ctx);

    symbolic_ctx
}

fn compile_initial_states(model: &Model, symbolic_ctx: &mut SymbolicContext) {
    let mut global_init = symbolic_ctx
        .manager
        .with_manager_shared(|m| BDDFunction::t(m));

    for (var_idx, expr_id) in &model.init_assignments {
        // Get the current IDs for the variable
        let var_curr_bdds = symbolic_ctx.manager.with_manager_shared(|m| {
            symbolic_ctx.var_map[*var_idx]
                .curr
                .iter()
                .map(|&id| BDDFunction::var(m, id).unwrap())
                .collect::<Vec<_>>()
        });

        // Build the assignment relation for the variable
        let var_init_relation =
            build_assignment_relation(&symbolic_ctx, &var_curr_bdds, *expr_id, model);

        // Combine the assignment relation with the global init BDD
        global_init = global_init.and(&var_init_relation).unwrap();
    }

    symbolic_ctx.initial_states = Some(global_init);
}

fn compile_transition_relation(model: &Model, symbolic_ctx: &mut SymbolicContext) {
    let mut global_delta = symbolic_ctx
        .manager
        .with_manager_shared(|m| BDDFunction::t(m));

    let mut has_next_assignment = vec![false; model.variables.len()];

    for (var_idx, expr_id) in &model.next_assignments {
        has_next_assignment[*var_idx] = true;
        let var_next_bdds = symbolic_ctx.manager.with_manager_shared(|m| {
            symbolic_ctx.var_map[*var_idx]
                .next
                .iter()
                .map(|&id| BDDFunction::var(m, id).unwrap())
                .collect::<Vec<_>>()
        });

        let var_transition =
            build_assignment_relation(&symbolic_ctx, &var_next_bdds, *expr_id, model);

        global_delta = global_delta.and(&var_transition).unwrap();
    }

    symbolic_ctx.transition_relation = Some(global_delta);
}
fn build_assignment_relation(
    symbolic_ctx: &SymbolicContext,
    target_bdds: &[BDDFunction],
    expr_id: SymbolicExprID,
    model: &Model,
) -> BDDFunction {
    let expr = &model.arena.expressions[expr_id.0 as usize];

    match expr {
        SymbolicExpr::Set { start, len } => {
            let mut set_relation = symbolic_ctx
                .manager
                .with_manager_shared(|m| BDDFunction::f(m));

            for i in 0..*len {
                let elem_id = model.arena.set_buffer[*start as usize + i as usize];
                let elem_relation =
                    build_assignment_relation(symbolic_ctx, target_bdds, elem_id, model);
                set_relation = set_relation.or(&elem_relation).unwrap();
            }
            set_relation
        }

        SymbolicExpr::Case { start, len } => {
            let mut case_relation = symbolic_ctx
                .manager
                .with_manager_shared(|m| BDDFunction::f(m));

            for i in (0..*len).rev() {
                let (cond_id, then_id) = model.arena.case_buffer[*start as usize + i as usize];

                let cond_bdd = eval_expr(symbolic_ctx, cond_id, model, 1)
                    .into_iter()
                    .next()
                    .unwrap();

                let then_relation =
                    build_assignment_relation(symbolic_ctx, target_bdds, then_id, model);

                case_relation = cond_bdd.ite(&then_relation, &case_relation).unwrap();
            }
            case_relation
        }
        _ => {
            let expr_bdds = eval_expr(symbolic_ctx, expr_id, model, target_bdds.len());
            bdd_number_eq(target_bdds, &expr_bdds, &symbolic_ctx.manager)
        }
    }
}

pub fn eval_expr(
    symbolic_ctx: &SymbolicContext,
    expr_id: SymbolicExprID,
    model: &Model,
    expected_bits: usize,
) -> Vec<BDDFunction> {
    let expr = &model.arena.expressions[expr_id.0 as usize];

    match expr {
        SymbolicExpr::Literal(value) => match value {
            Value::Bool(b) => symbolic_ctx.manager.with_manager_shared(|m| {
                if *b {
                    vec![BDDFunction::t(m)]
                } else {
                    vec![BDDFunction::f(m)]
                }
            }),

            Value::Int(n) => {
                let needed_bits = if *n > 0 {
                    calc_bits((*n + 1) as usize)
                } else {
                    1
                };

                let final_bits = std::cmp::max(needed_bits, expected_bits);

                let mut bits = Vec::with_capacity(final_bits);
                symbolic_ctx.manager.with_manager_shared(|m| {
                    for i in 0..final_bits {
                        if (n >> i) & 1 == 1 {
                            bits.push(BDDFunction::t(m));
                        } else {
                            bits.push(BDDFunction::f(m));
                        }
                    }
                });
                bits
            }
            /*
            Value::Int(n) => {
                let mut bits = Vec::with_capacity(32);
                symbolic_ctx.manager.with_manager_shared(|m| {
                    for i in 0..32 {
                        if (n >> i) & 1 == 1 {
                            bits.push(BDDFunction::t(m));
                        } else {
                            bits.push(BDDFunction::f(m));
                        }
                    }
                });
                bits
            }*/
            Value::Enum(idx) => {
                let final_bits = std::cmp::max(1, expected_bits);
                let mut bits = Vec::with_capacity(final_bits);
                symbolic_ctx.manager.with_manager_shared(|m| {
                    for i in 0..final_bits {
                        if (idx >> i) & 1 == 1 {
                            bits.push(BDDFunction::t(m));
                        } else {
                            bits.push(BDDFunction::f(m));
                        }
                    }
                });
                bits
            }
        },
        SymbolicExpr::Reference(var_idx) => {
            let bits_info = &symbolic_ctx.var_map[*var_idx];

            symbolic_ctx.manager.with_manager_shared(|m| {
                bits_info
                    .curr
                    .iter()
                    .map(|&id| BDDFunction::var(m, id).unwrap())
                    .collect()
            })
        }

        SymbolicExpr::Unary(op, sub_id) => {
            // Propaga a largura esperada para a sub-expressão unária
            let sub_bdds = eval_expr(symbolic_ctx, *sub_id, model, expected_bits);

            match op {
                UnaryOp::Not => {
                    vec![sub_bdds[0].not().unwrap()]
                }
                UnaryOp::Neg => {
                    let zero_vec = symbolic_ctx
                        .manager
                        .with_manager_shared(|m| vec![BDDFunction::f(m); sub_bdds.len()]);
                    bdd_number_sub(&zero_vec, &sub_bdds, &symbolic_ctx.manager)
                }
            }
        }

        SymbolicExpr::Binary(op, lhs, rhs) => match op {
            BinaryOp::And | BinaryOp::Or | BinaryOp::Imply => {
                let left_bdds = eval_expr(symbolic_ctx, *lhs, model, 1);
                let right_bdds = eval_expr(symbolic_ctx, *rhs, model, 1);

                let left = left_bdds.into_iter().next().unwrap();
                let right = right_bdds.into_iter().next().unwrap();

                let res = match op {
                    BinaryOp::And => left.and(&right).unwrap(),
                    BinaryOp::Or => left.or(&right).unwrap(),
                    BinaryOp::Imply => left.imp(&right).unwrap(),
                    _ => unreachable!(),
                };
                vec![res]
            }

            BinaryOp::Eq
            | BinaryOp::Neq
            | BinaryOp::Lt
            | BinaryOp::Lte
            | BinaryOp::Gt
            | BinaryOp::Gte => {
                let left_bdds = eval_expr(symbolic_ctx, *lhs, model, 0);

                let right_bdds = eval_expr(symbolic_ctx, *rhs, model, left_bdds.len());

                let bdd = match op {
                    BinaryOp::Eq => bdd_number_eq(&left_bdds, &right_bdds, &symbolic_ctx.manager),
                    BinaryOp::Neq => bdd_number_neq(&left_bdds, &right_bdds, &symbolic_ctx.manager),
                    BinaryOp::Lt => bdd_number_lt(&left_bdds, &right_bdds, &symbolic_ctx.manager),
                    BinaryOp::Lte => bdd_number_lte(&left_bdds, &right_bdds, &symbolic_ctx.manager),
                    BinaryOp::Gt => bdd_number_gt(&left_bdds, &right_bdds, &symbolic_ctx.manager),
                    BinaryOp::Gte => bdd_number_gte(&left_bdds, &right_bdds, &symbolic_ctx.manager),
                    _ => unreachable!(),
                };
                vec![bdd]
            }

            BinaryOp::Add | BinaryOp::Sub => {
                let left_bdds = eval_expr(symbolic_ctx, *lhs, model, expected_bits);
                let right_bdds = eval_expr(symbolic_ctx, *rhs, model, expected_bits);

                match op {
                    BinaryOp::Add => {
                        ripple_carry_adder(&left_bdds, &right_bdds, &symbolic_ctx.manager)
                    }
                    BinaryOp::Sub => bdd_number_sub(&left_bdds, &right_bdds, &symbolic_ctx.manager),
                    _ => unreachable!(),
                }
            }
            _ => {
                panic!("Not implemented");
            }
        },

        SymbolicExpr::Case { .. } | SymbolicExpr::Set { .. } => {
            panic!("Case and Set are valid only at the top level");
        }
    }
}
