use crate::core::bdd::{
    SymbolicContext, bdd_number_eq, bdd_number_gt, bdd_number_gte, bdd_number_lt, bdd_number_lte,
    bdd_number_neq, bdd_number_sub, ripple_carry_adder,
};
use crate::modeling::symbolic::{BinaryOp, Model, SymbolicExpr, SymbolicExprID, UnaryOp, Value};

use oxidd::bdd::BDDFunction;
use oxidd::{BooleanFunction, ManagerRef};

pub fn compile_model_to_bdd(model: &Model) -> SymbolicContext {
    // Create the symbolic context and allocate variables
    let mut symbolic_ctx = SymbolicContext::new(model);

    // Compile the initial states
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

    for var_idx in 0..model.variables.len() {
        if !has_next_assignment[var_idx] {
            let (curr_bdds, next_bdds) = symbolic_ctx.manager.with_manager_shared(|m| {
                let curr = symbolic_ctx.var_map[var_idx]
                    .curr
                    .iter()
                    .map(|&id| BDDFunction::var(m, id).unwrap())
                    .collect::<Vec<_>>();
                let next = symbolic_ctx.var_map[var_idx]
                    .next
                    .iter()
                    .map(|&id| BDDFunction::var(m, id).unwrap())
                    .collect::<Vec<_>>();
                (curr, next)
            });

            let frame_condition = bdd_number_eq(&curr_bdds, &next_bdds, &symbolic_ctx.manager);

            global_delta = global_delta.and(&frame_condition).unwrap();
        }
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

                let cond_bdd = eval_expr(symbolic_ctx, cond_id, model)
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
            let expr_bdds = eval_expr(symbolic_ctx, expr_id, model);
            bdd_number_eq(target_bdds, &expr_bdds, &symbolic_ctx.manager)
        }
    }
}

pub fn eval_expr(
    symbolic_ctx: &SymbolicContext,
    expr_id: SymbolicExprID,
    model: &Model,
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
            }
            Value::Enum(idx) => {
                let mut bits = Vec::with_capacity(32);
                symbolic_ctx.manager.with_manager_shared(|m| {
                    for i in 0..32 {
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
            let sub_bdds = eval_expr(symbolic_ctx, *sub_id, model);

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

        SymbolicExpr::Binary(op, lhs, rhs) => {
            let left_bdds = eval_expr(symbolic_ctx, *lhs, model);
            let right_bdds = eval_expr(symbolic_ctx, *rhs, model);

            match op {
                BinaryOp::And => {
                    let left = left_bdds.into_iter().next().unwrap();
                    let right = right_bdds.into_iter().next().unwrap();
                    let and = left.and(&right);
                    vec![and.unwrap()]
                }

                BinaryOp::Or => {
                    let left = left_bdds.into_iter().next().unwrap();
                    let right = right_bdds.into_iter().next().unwrap();
                    let or = left.or(&right);
                    vec![or.unwrap()]
                }
                BinaryOp::Imply => {
                    let left = left_bdds.into_iter().next().unwrap();
                    let right = right_bdds.into_iter().next().unwrap();
                    let imply = left.imp(&right);
                    vec![imply.unwrap()]
                }

                BinaryOp::Eq => {
                    let bdd = bdd_number_eq(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    vec![bdd]
                }
                BinaryOp::Neq => {
                    let bdd = bdd_number_neq(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    vec![bdd]
                }
                BinaryOp::Lt => {
                    let bdd = bdd_number_lt(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    vec![bdd]
                }
                BinaryOp::Lte => {
                    let bdd = bdd_number_lte(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    vec![bdd]
                }
                BinaryOp::Gt => {
                    let bdd = bdd_number_gt(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    vec![bdd]
                }
                BinaryOp::Gte => {
                    let bdd = bdd_number_gte(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    vec![bdd]
                }

                BinaryOp::Add => {
                    let bdd = ripple_carry_adder(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    bdd
                }
                BinaryOp::Sub => {
                    let bdd = bdd_number_sub(&left_bdds, &right_bdds, &symbolic_ctx.manager);
                    bdd
                }
                _ => {
                    panic!("Not implemented");
                }
            }
        }

        SymbolicExpr::Case { .. } | SymbolicExpr::Set { .. } => {
            panic!("Case and Set are valid only at the top level");
        }
    }
}
