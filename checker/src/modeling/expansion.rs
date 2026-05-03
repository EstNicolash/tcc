use crate::core::kripke_structure::{KripkeBuilder, KripkeStructure, StateID};
use crate::modeling::symbolic::{
    BinaryOp, Domain, Model, SymbolicArena, SymbolicExpr, SymbolicExprID, UnaryOp, Value,
};
use std::collections::{HashMap, HashSet, VecDeque};

type State = Vec<i32>;

fn is_true(v: i32) -> bool {
    v != 0
}

fn from_bool(b: bool) -> i32 {
    if b { 1 } else { 0 }
}

/// Converts a `Value` to the flat `i32` format used in states.
fn value_to_i32(v: &Value) -> i32 {
    match v {
        Value::Bool(b) => from_bool(*b),
        Value::Int(i) => *i,
        Value::Enum(idx) => *idx as i32,
    }
}

pub fn expand_to_kripke(model: &Model) -> KripkeStructure {
    let num_vars = model.variables.len();
    let mut builder = KripkeBuilder::new(num_vars);
    let mut queue: VecDeque<StateID> = VecDeque::new();

    for state_data in compute_initial_states(model) {
        let id = builder.states.get_or_insert(&state_data);
        builder.add_initial_state(id);
        queue.push_back(id);
    }

    // BFS in the state space
    while let Some(current_id) = queue.pop_front() {
        let current_state = builder.states.get_values(current_id);
        let next_states_data = compute_next_states(model, current_state);

        // Avoid adding duplicate edges from the same current state
        // by keeping track of seen next states
        let mut seen_next: HashSet<StateID> = HashSet::new();

        for next_data in next_states_data {
            let is_new = !builder.states.contains(&next_data);
            let next_id = builder.states.get_or_insert(&next_data);

            if seen_next.insert(next_id) {
                // Add edge only once per (current, next) pair
                builder.add_transition(current_id, next_id);
            }

            if is_new {
                // queue only for new states
                queue.push_back(next_id);
            }
        }
    }

    KripkeStructure::from_builder(builder)
}

/// Evaluates a symbolic expression given a flat `&[i32]` state.
///
/// # Returns
///
/// `Vec<i32>` with a single element for scalar expressions,
/// or multiple elements for `Set` (non-determinism).
pub fn eval(expr_id: SymbolicExprID, state: &[i32], model: &Model) -> Vec<i32> {
    let expr = &model.arena.expressions[expr_id.0 as usize];

    match expr {
        SymbolicExpr::Literal(value) => vec![value_to_i32(value)],
        SymbolicExpr::Reference(var_idx) => vec![state[*var_idx]],

        SymbolicExpr::Unary(op, sub_id) => {
            let val = eval(*sub_id, state, model).into_iter().next().unwrap();
            match op {
                UnaryOp::Not => vec![from_bool(!is_true(val))],
                UnaryOp::Neg => vec![-val],
            }
        }

        SymbolicExpr::Binary(op, lhs_id, rhs_id) => {
            let lval = eval(*lhs_id, state, model).into_iter().next().unwrap();
            let rval = eval(*rhs_id, state, model).into_iter().next().unwrap();

            let result = match op {
                BinaryOp::And => from_bool(is_true(lval) && is_true(rval)),
                BinaryOp::Or => from_bool(is_true(lval) || is_true(rval)),
                BinaryOp::Imply => from_bool(!is_true(lval) || is_true(rval)),
                BinaryOp::Eq => from_bool(lval == rval),
                BinaryOp::Neq => from_bool(lval != rval),
                BinaryOp::Lt => from_bool(lval < rval),
                BinaryOp::Lte => from_bool(lval <= rval),
                BinaryOp::Gt => from_bool(lval > rval),
                BinaryOp::Gte => from_bool(lval >= rval),
                BinaryOp::Add => lval + rval,
                BinaryOp::Sub => lval - rval,
                BinaryOp::Mul => lval * rval,
                BinaryOp::Div => lval / rval,
            };
            vec![result]
        }

        // Case is first-match — returns as soon as a true arm is found.
        // If no arm is satisfied, the model is malformed (case not total).
        SymbolicExpr::Case { start, len } => {
            for i in (*start as usize)..(*start as usize + *len as usize) {
                let (cond_id, then_id) = model.arena.case_buffer[i];
                let cond_val = eval(cond_id, state, model).into_iter().next().unwrap();
                if is_true(cond_val) {
                    return eval(then_id, state, model);
                }
            }
            panic!(
                "Non-total case expression: no arm matched.\n\
                 State: {:?}\n\
                 Case start={}, len={}",
                state, start, len
            )
        }

        // Set represents non-determinism: all possible values.
        SymbolicExpr::Set { start, len } => {
            let mut results = Vec::new();
            for i in (*start as usize)..(*start as usize + *len as usize) {
                let elem_id = model.arena.set_buffer[i];
                results.extend(eval(elem_id, state, model));
            }
            results
        }
    }
}

fn compute_initial_states(model: &Model) -> Vec<State> {
    let init_map: HashMap<usize, SymbolicExprID> = model
        .init_assignments
        .iter()
        .map(|&(idx, eid)| (idx, eid))
        .collect();

    // Dummy state with zeros — used only for init expressions
    // that do not reference other variables.
    let dummy_state = vec![0i32; model.variables.len()];

    let mut values_per_var: Vec<Vec<i32>> = Vec::with_capacity(model.variables.len());

    for (idx, var) in model.variables.iter().enumerate() {
        if let Some(&expr_id) = init_map.get(&idx) {
            if expr_contains_reference(expr_id, &model.arena) {
                // The init expression references other variables.
                // dummy_state can have an invalid value for it, so
                // we use the full domain as a conservative fallback.
                eprintln!(
                    "Warning: init(var[{}]) references other variables — \
                     using full domain as fallback",
                    idx
                );
                values_per_var.push(get_domain_values(&var.domain));
            } else {
                // Without reference: safe to evaluate with dummy_state
                values_per_var.push(eval(expr_id, &dummy_state, model));
            }
        } else {
            // Without init: non-deterministic variable — use full domain
            values_per_var.push(get_domain_values(&var.domain));
        }
    }

    cartesian_product(&values_per_var)
}

fn compute_next_states(model: &Model, state: &[i32]) -> Vec<State> {
    let next_map: HashMap<usize, SymbolicExprID> = model
        .next_assignments
        .iter()
        .map(|&(idx, eid)| (idx, eid))
        .collect();

    let mut values_per_var = Vec::with_capacity(model.variables.len());

    for (idx, var) in model.variables.iter().enumerate() {
        if let Some(&expr_id) = next_map.get(&idx) {
            // With next: evaluate next expression
            values_per_var.push(eval(expr_id, state, model));
        } else {
            values_per_var.push(get_domain_values(&var.domain));
        }
    }

    cartesian_product(&values_per_var)
}

/// Returns true if the expression `expr_id` contains any `Reference`
/// (i.e., references a state variable).
///
/// Used to decide if it's safe to evaluate an `init` expression with
/// a dummy state of zeros.
fn expr_contains_reference(expr_id: SymbolicExprID, arena: &SymbolicArena) -> bool {
    let expr = &arena.expressions[expr_id.0 as usize];
    match expr {
        SymbolicExpr::Reference(_) => true,
        SymbolicExpr::Literal(_) => false,

        SymbolicExpr::Unary(_, sub) => expr_contains_reference(*sub, arena),
        SymbolicExpr::Binary(_, lhs, rhs) => {
            expr_contains_reference(*lhs, arena) || expr_contains_reference(*rhs, arena)
        }

        SymbolicExpr::Case { start, len } => (0..*len as usize).any(|i| {
            let (cond, then) = arena.case_buffer[*start as usize + i];
            expr_contains_reference(cond, arena) || expr_contains_reference(then, arena)
        }),

        SymbolicExpr::Set { start, len } => (0..*len as usize)
            .any(|i| expr_contains_reference(arena.set_buffer[*start as usize + i], arena)),
    }
}

fn get_domain_values(domain: &Domain) -> Vec<i32> {
    match domain {
        Domain::Boolean => vec![0, 1],
        Domain::Range { min, max } => (*min..=*max).collect(),
        Domain::Enum(vals) => (0..vals.len() as i32).collect(),
    }
}

/// Compute the Cartesian product of a list of sets.
///
/// # Example:
/// ```
/// let sets = vec![vec![1, 2], vec![3, 4]];
/// // → [[1,3], [1,4], [2,3], [2,4]]
/// ```
fn cartesian_product(sets: &[Vec<i32>]) -> Vec<State> {
    let mut result: Vec<State> = vec![vec![]];

    for set in sets {
        let mut new_result = Vec::new();
        for state in &result {
            for &val in set {
                let mut concat = state.clone();
                concat.push(val);
                new_result.push(concat);
            }
        }
        result = new_result;
    }
    result
}
