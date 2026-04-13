use crate::core::kripke_structure::KripkeStructure;
use crate::modeling::symbolic::{BinaryOp, Domain, Expr, Model, UnaryOp, Value};
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, VecDeque};

type State = Vec<Value>;

pub fn expand_to_kripke(model: &Model) -> KripkeStructure {
    let mut ks = KripkeStructure::new();

    let mut state_map: HashMap<State, NodeIndex> = HashMap::new();
    let mut queue: VecDeque<State> = VecDeque::new();

    let initial_states = compute_initial_states(model);

    /*
    println!("=== INITIAL STATES ({}) ===", initial_states.len());
    for (i, state) in initial_states.iter().enumerate() {
        let labels = generate_labels(model, state);
        println!("  [{}] {:?}", i, labels);
    }

    let first_state = &initial_states[0];
    let next = compute_next_states(model, first_state);
    println!("=== NEXT STATES of state 0 ({}) ===", next.len());
    for (i, state) in next.iter().enumerate() {
        println!("  [{}] {:?}", i, generate_labels(model, state));
    }

    println!("=== Variables ===");
    for var in &model.variables {
        let has_init = model.init_assignments.iter().any(|(idx, _)| {
            *idx == model
                .variables
                .iter()
                .position(|v| v.name == var.name)
                .unwrap()
        });
        let has_next = model.next_assignments.iter().any(|(idx, _)| {
            *idx == model
                .variables
                .iter()
                .position(|v| v.name == var.name)
                .unwrap()
        });
        println!(
            "  '{}': {} valores | init={} | next={}",
            var.name,
            var.domain.size(),
            has_init,
            has_next
        );
    }*/

    for state in initial_states {
        let labels = generate_labels(model, &state);
        let state_name = format!("s_{}", state_map.len());

        let node_idx = ks.add_state(&state_name, labels, true);
        state_map.insert(state.clone(), node_idx);
        queue.push_back(state);
    }

    while let Some(current_state) = queue.pop_front() {
        let current_node = state_map[&current_state];

        let next_states = compute_next_states(model, &current_state);

        for next_state in next_states {
            let next_node = match state_map.get(&next_state) {
                Some(&idx) => idx,
                None => {
                    let labels = generate_labels(model, &next_state);
                    let state_name = format!("s_{}", state_map.len());

                    let new_node_idx = ks.add_state(&state_name, labels, false);
                    state_map.insert(next_state.clone(), new_node_idx);
                    queue.push_back(next_state.clone());

                    new_node_idx
                }
            };

            ks.add_transition(current_node, next_node);
        }
    }

    ks.make_total();
    ks
}

fn expect_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        other => panic!("Expected Bool, received: {:?}", other),
    }
}

fn expect_int(v: &Value) -> i32 {
    match v {
        Value::Int(i) => *i,
        other => panic!("Expected Int, received: {:?}", other),
    }
}
fn eval(expr: &Expr, state: &State, model: &Model) -> Vec<Value> {
    match expr {
        Expr::Literal(value) => vec![value.clone()],
        Expr::Reference(var_idx) => vec![state[*var_idx].clone()],
        Expr::Unary(op, sub) => {
            let val = eval(sub, state, model).into_iter().next().unwrap();
            let result = match op {
                UnaryOp::Not => vec![Value::Bool(!expect_bool(&val))],
                UnaryOp::Neg => vec![Value::Int(-(expect_int(&val)))],
            };
            result
        }
        Expr::Binary(op, lhs, rhs) => {
            let lval = eval(lhs, state, model).into_iter().next().unwrap();
            let rval = eval(rhs, state, model).into_iter().next().unwrap();
            let result = match op {
                BinaryOp::And => vec![Value::Bool(expect_bool(&lval) && expect_bool(&rval))],
                BinaryOp::Or => vec![Value::Bool(expect_bool(&lval) || expect_bool(&rval))],
                BinaryOp::Imply => vec![Value::Bool(!expect_bool(&lval) || expect_bool(&rval))],
                BinaryOp::Eq => vec![Value::Bool(lval == rval)],
                BinaryOp::Neq => vec![Value::Bool(lval != rval)],
                BinaryOp::Lt => vec![Value::Bool(expect_int(&lval) < expect_int(&rval))],
                BinaryOp::Lte => vec![Value::Bool(expect_int(&lval) <= expect_int(&rval))],
                BinaryOp::Gt => vec![Value::Bool(expect_int(&lval) > expect_int(&rval))],
                BinaryOp::Gte => vec![Value::Bool(expect_int(&lval) >= expect_int(&rval))],
                BinaryOp::Add => vec![Value::Int(expect_int(&lval) + expect_int(&rval))],
                BinaryOp::Sub => vec![Value::Int(expect_int(&lval) - expect_int(&rval))],
                BinaryOp::Mul => vec![Value::Int(expect_int(&lval) * expect_int(&rval))],
                BinaryOp::Div => vec![Value::Int(expect_int(&lval) / expect_int(&rval))],
            };
            result
        }
        Expr::Case(arms) => {
            let mut result: Vec<Value> = vec![];
            for (cond, then_expr) in arms {
                let cond_val = eval(&cond, state, model).into_iter().next().unwrap();
                if expect_bool(&cond_val) {
                    result = eval(then_expr, state, model);
                    break;
                }
            }
            result
        }
        Expr::Set(exprs) => exprs
            .iter()
            .flat_map(|expr| eval(expr, state, model))
            .collect(),
    }
}

fn generate_labels(model: &Model, state: &State) -> Vec<String> {
    let mut labels = Vec::new();
    for (var_idx, value) in state.iter().enumerate() {
        let var_name = &model.variables[var_idx].name;
        match value {
            Value::Bool(b) => {
                if *b {
                    labels.push(var_name.clone());
                }
            }
            Value::Int(i) => labels.push(format!("{}={}", var_name, i)),

            Value::Enum(e_idx) => {
                if let Domain::Enum(vals) = &model.variables[var_idx].domain {
                    labels.push(format!("{}={}", var_name, vals[*e_idx]));
                }
            }
        }
    }
    labels
}

fn compute_initial_states(model: &Model) -> Vec<State> {
    let mut values_per_var: Vec<Vec<Value>> = vec![];

    for (idx, var) in model.variables.iter().enumerate() {
        let assignment = model
            .init_assignments
            .iter()
            .find(|(var_idx, _)| *var_idx == idx);

        if let Some((_, expr)) = assignment {
            let dummy_state = model
                .variables
                .iter()
                .map(|v| v.domain.values()[0].clone())
                .collect::<State>();

            let values = eval(&expr, &dummy_state, model);
            values_per_var.push(values);
        } else {
            values_per_var.push(var.domain.values());
        }
    }

    cartesian_product(&values_per_var)
}

/// Computes the Cartesian product of a set of value sets.
/// # Arguments
///
/// * `sets` - A vector of value sets to compute the product of.
///
/// # Returns
///
/// A vector of states representing the Cartesian product of the input sets.
///
/// # Example:
/// * `VAR x : {a,b}; y : boolean;`
///
/// * `init(x) := {a, b};`    Set → values_per_var\[0\] = \[Enum(0), Enum(1)\]
/// * `init(y) := FALSE;`     Lit → values_per_var\[1\] = \[Bool(false)\]
///
/// * `cartesian_product(&values_per_var)` → [[Enum(0), Bool(false)],
///                                       [Enum(1), Bool(false)]]
fn cartesian_product(sets: &Vec<Vec<Value>>) -> Vec<State> {
    let mut result: Vec<State> = vec![vec![]];

    for set in sets {
        let mut new_result = Vec::new();
        for state in &result {
            for val in set {
                let mut concat = state.clone();
                concat.push(val.clone());
                new_result.push(concat);
            }
        }
        result = new_result;
    }
    result
}

fn compute_next_states(model: &Model, state: &State) -> Vec<State> {
    let mut values_per_var: Vec<Vec<Value>> = vec![];

    for (idx, _) in model.variables.iter().enumerate() {
        let assignment = model
            .next_assignments
            .iter()
            .find(|(var_idx, _)| *var_idx == idx);

        if let Some((_, expr)) = assignment {
            let vals = eval(expr, state, model);
            /*
            println!(
                "  next('{}') → {} valores: {:?}",
                var.name,
                vals.len(),
                vals.iter()
                    .map(|v| generate_labels_single(model, idx, v))
                    .collect::<Vec<_>>()
            );*/

            values_per_var.push(vals);
        } else {
            values_per_var.push(vec![state[idx].clone()]);
        }
    }

    cartesian_product(&values_per_var)
}

fn generate_labels_single(model: &Model, var_idx: usize, value: &Value) -> String {
    let var_name = &model.variables[var_idx].name;
    match value {
        Value::Bool(b) => format!("{}={}", var_name, b),
        Value::Int(i) => format!("{}={}", var_name, i),
        Value::Enum(e) => {
            if let Domain::Enum(vals) = &model.variables[var_idx].domain {
                format!("{}={}", var_name, vals[*e])
            } else {
                format!("{}={}", var_name, e)
            }
        }
    }
}
