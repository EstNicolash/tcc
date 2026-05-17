//! # Module `ordering`
//!
//! This module provides BDD variable ordering heuristics for symbolic model expansion.
//!
//! # BDD Variable Ordering Heuristics
//!
//! Produces a `Vec<String>` in the same format as `SymbolicContext::new`'s
//! `explicit_order` parameter:
//!   - `"pc0"`   — single-bit variable (boolean or 1-value domain)
//!   - `"pc0.2"` — bit 2 of variable `pc0`
//!
//! ## Strategies
//!
//! | Strategy  | Quality  | Notes                          |
//! |-----------|----------|--------------------------------|
//! | `Default` | baseline | declaration order              |
//! | `Random`  | varies   | stochastic baseline, seeded    |
//! | `Force`   | good     | FORCE heuristic                |
//!
//! # Hyperedges building
//!
//! Collects hyperedges from init/next assignments and CTL formulas.
//!
//! The hyperedges set builded is a laminar set family.
//!

use crate::core::bdd::calc_bits;
use crate::modeling::symbolic::{Domain, Model, SymbolicArena, SymbolicExpr, SymbolicExprID};
use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

// ─── Public API ──────────────────────────────────────────────────────────────

/// Strategy for BDD variable ordering.
#[derive(Debug, Clone)]
pub enum OrderingStrategy {
    Default,

    Random {
        seed: u64,
    },

    /// FORCE heuristic (Aloul, Markov, Sakallah — GLSVLSI 2003).
    Force {
        iterations: usize,
    },
}

/// Compute a variable ordering for `model` using `strategy`.
///
/// Returns a `Vec<String>` ready to pass as `explicit_order` to
/// `SymbolicContext::new`.
pub fn compute_ordering(model: &Model, strategy: OrderingStrategy) -> Vec<String> {
    let n = model.variables.len();

    let var_order: Vec<usize> = match strategy {
        OrderingStrategy::Default => (0..n).collect(),

        OrderingStrategy::Random { seed } => {
            let mut order: Vec<usize> = (0..n).collect();
            shuffle_ordering(&mut order, seed);
            order
        }

        OrderingStrategy::Force { iterations } => {
            let hyperedges = build_hyperedges(model);
            force_ordering(n, &hyperedges, iterations)
        }
    };

    expand_to_bit_names(model, &var_order)
}
/// For a given set of hyperedges, iteratively re-ranks variables to minimize their distance from their hyperedge centres.
///
/// # Arguments
///
/// * `n` - The number of variables.
/// * `hyperedges` - The hyperedges to re-rank.
/// * `iterations` - The number of iterations to perform.
///
/// # Stop Condition
/// The span cost is the sum of the distances between variables and their hyperedge centres.
/// The span cost is minimized by iteratively adjusting the positions of variables.
/// If the span cost stops decreasing, the algorithm stops.
/// The algorithm also stops when the number of iterations reaches the specified limit.
///
/// # Returns
///
/// A vector of variable indices in the order they should be placed.
///
///
fn force_ordering(n: usize, hyperedges: &[Hyperedge], iterations: usize) -> Vec<usize> {
    let mut position_of: Vec<f64> = (0..n).map(|i| i as f64).collect();

    // Pre-compute reverse index: variable -> list of hyperedge indices it belongs to
    let mut var_to_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (e_idx, edge) in hyperedges.iter().enumerate() {
        for &v in &edge.vars {
            var_to_edges[v].push(e_idx);
        }
    }

    let initial_order: Vec<usize> = (0..n).collect();
    let mut best_cost = span_cost(&initial_order, hyperedges);

    for _ in 0..iterations {
        // Centre of each hyperedge
        let centres = calculate_centres(hyperedges, &position_of);

        // Ideal position of each variable = weighted mean of its edge centres
        let mut ideal: Vec<f64> = vec![0.0; n];
        for v in 0..n {
            let edges = &var_to_edges[v];
            if edges.is_empty() {
                ideal[v] = position_of[v];
                continue;
            }
            //
            let (sum_w, sum_wc) = edges.iter().fold((0.0f64, 0.0f64), |(sw, swc), &e| {
                let w = hyperedges[e].weight;
                (sw + w, swc + w * centres[e])
            });
            ideal[v] = if sum_w > 0.0 {
                sum_wc / sum_w
            } else {
                position_of[v]
            };
        }

        // Re-rank by ideal position
        let mut ranking: Vec<(f64, usize)> =
            ideal.iter().enumerate().map(|(v, &p)| (p, v)).collect();
        ranking.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));

        let next_order: Vec<usize> = ranking.iter().map(|&(_, var)| var).collect();
        let next_cost = span_cost(&next_order, hyperedges);

        // Stop if cost doesn't improve
        if next_cost >= best_cost {
            break;
        }

        best_cost = next_cost;

        for (new_pos, &(_, var)) in ranking.iter().enumerate() {
            position_of[var] = new_pos as f64;
        }
    }

    let mut order: Vec<(f64, usize)> = position_of
        .iter()
        .enumerate()
        .map(|(v, &p)| (p, v))
        .collect();
    order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    order.iter().map(|&(_, v)| v).collect()
}

/// Calculates the centre of each hyperedge based on the position of its variables.
///
/// # Arguments
///
/// * `hyperedges` - The hyperedges to calculate centres for.
/// * `position_of` - The position of each variable.
///
/// # Returns
///
/// A vector of centres, one for each hyperedge.
fn calculate_centres(hyperedges: &[Hyperedge], position_of: &[f64]) -> Vec<f64> {
    hyperedges
        .iter()
        .map(|e| {
            let sum: f64 = e.vars.iter().map(|&v| position_of[v]).sum();
            sum / e.vars.len() as f64
        })
        .collect()
}

/// A hyperedge groups all variables that appear together in one expression.
struct Hyperedge {
    vars: Vec<usize>,
    weight: f64,
}

/// Traverse every init/next expression and collect co-occurring variables
/// as hyperedges.
///
/// # Arguments
///
/// * `model` - The model to analyze.
///
/// # Returns
///
/// A vector of hyperedges, each representing a group of co-occurring variables.
fn build_hyperedges(model: &Model) -> Vec<Hyperedge> {
    let mut edges: Vec<Hyperedge> = Vec::new();

    // Collect hyperedges from init/next assignments
    for &(assigned_var, expr_id) in model
        .init_assignments
        .iter()
        .chain(model.next_assignments.iter())
    {
        let mut vars: Vec<usize> = Vec::new();
        collect_references(expr_id, &model.arena, &mut vars);
        vars.push(assigned_var);
        vars.sort_unstable();
        vars.dedup();

        if vars.len() > 1 {
            edges.push(Hyperedge { vars, weight: 1.0 });
        }
    }

    // Collect hyperedges from CTL formulas
    for &spec_id in &model.specs {
        collect_edges_from_ctl(spec_id, &model.ctl_arena, &model.arena, &mut edges);
    }

    edges
}
/// Collect hyperedges from a CTL formula.
/// Recursively traverses the formula and collects hyperedges for each symbolic expression.
///
/// # Arguments
///
/// * `formula_id` - The ID of the CTL formula to collect hyperedges from.
/// * `ctl_arena` - The arena containing the CTL formulae.
/// * `sym_arena` - The arena containing the symbolic expressions.
/// * `out_edges` - The vector to store the collected hyperedges.
///
fn collect_edges_from_ctl(
    formula_id: FormulaID,
    ctl_arena: &CtlFormulaArena<SymbolicExprID>,
    sym_arena: &SymbolicArena,
    out_edges: &mut Vec<Hyperedge>,
) {
    let formula = ctl_arena.get(formula_id);
    match formula {
        // Expression case
        CtlFormula::Prop(sym_id) => {
            collect_edges_from_symbolic(*sym_id, sym_arena, out_edges);
        }
        // Unary cases
        CtlFormula::Not(f)
        | CtlFormula::EX(f)
        | CtlFormula::AX(f)
        | CtlFormula::EF(f)
        | CtlFormula::AF(f)
        | CtlFormula::EG(f)
        | CtlFormula::AG(f) => {
            collect_edges_from_ctl(*f, ctl_arena, sym_arena, out_edges);
        }
        // Binary cases
        CtlFormula::And(f1, f2)
        | CtlFormula::Or(f1, f2)
        | CtlFormula::Imply(f1, f2)
        | CtlFormula::Iff(f1, f2)
        | CtlFormula::EU(f1, f2)
        | CtlFormula::AU(f1, f2) => {
            collect_edges_from_ctl(*f1, ctl_arena, sym_arena, out_edges);
            collect_edges_from_ctl(*f2, ctl_arena, sym_arena, out_edges);
        }
        _ => {}
    }
}
/// Collect hyperedges from a symbolic expression.
/// Recursively traverses the expression and collects hyperedges for each variable reference.
///
/// # Arguments
///
/// * `expr_id` - The ID of the symbolic expression to collect hyperedges from.
/// * `arena` - The arena containing the symbolic expressions.
/// * `out_edges` - The vector to store the collected hyperedges.
fn collect_edges_from_symbolic(
    expr_id: SymbolicExprID,
    arena: &SymbolicArena,
    out_edges: &mut Vec<Hyperedge>,
) {
    match &arena.expressions[expr_id.0 as usize] {
        SymbolicExpr::Binary(_, lhs, rhs) => {
            let mut vars = Vec::new();
            collect_references(*lhs, arena, &mut vars);
            collect_references(*rhs, arena, &mut vars);

            vars.sort_unstable();
            vars.dedup();

            if vars.len() > 1 {
                out_edges.push(Hyperedge { vars, weight: 2.0 });
            }

            collect_edges_from_symbolic(*lhs, arena, out_edges);
            collect_edges_from_symbolic(*rhs, arena, out_edges);
        }
        SymbolicExpr::Unary(_, sub) => {
            collect_edges_from_symbolic(*sub, arena, out_edges);
        }
        _ => {}
    }
}
/// Recursively collect all `Reference` (variable) indices reachable from `expr_id`.
///
/// # Arguments
///
/// * `expr_id` - The ID of the symbolic expression to collect references from.
/// * `arena` - The arena containing the symbolic expressions.
/// * `out` - The vector to store the collected references.
fn collect_references(expr_id: SymbolicExprID, arena: &SymbolicArena, out: &mut Vec<usize>) {
    match &arena.expressions[expr_id.0 as usize] {
        SymbolicExpr::Literal(_) => {}

        SymbolicExpr::Reference(idx) => out.push(*idx),

        SymbolicExpr::Unary(_, sub) => {
            collect_references(*sub, arena, out);
        }

        SymbolicExpr::Binary(_, lhs, rhs) => {
            collect_references(*lhs, arena, out);
            collect_references(*rhs, arena, out);
        }

        SymbolicExpr::Case { start, len } => {
            for i in (*start as usize)..(*start as usize + *len as usize) {
                let (cond, then) = arena.case_buffer[i];
                collect_references(cond, arena, out);
                collect_references(then, arena, out);
            }
        }

        SymbolicExpr::Set { start, len } => {
            for i in (*start as usize)..(*start as usize + *len as usize) {
                collect_references(arena.set_buffer[i], arena, out);
            }
        }
    }
}

/// Total hyperedge span cost for a variable ordering.
/// `order[position] = variable_index`.  Lower is better.
pub fn span_cost(order: &[usize], hyperedges: &[Hyperedge]) -> f64 {
    let mut position_of = vec![0usize; order.len()];
    for (pos, &var) in order.iter().enumerate() {
        position_of[var] = pos;
    }

    hyperedges
        .iter()
        .map(|e| {
            let min = e.vars.iter().map(|&v| position_of[v]).min().unwrap_or(0);
            let max = e.vars.iter().map(|&v| position_of[v]).max().unwrap_or(0);
            e.weight * (max - min) as f64
        })
        .sum()
}

/// Expands the given variable order to bit-level names.
///
/// # Arguments
///
/// * `model` - The model containing the variables.
/// * `var_order` - The variable order to expand.
///
/// # Returns
///
/// A vector of bit-level names corresponding to the given variable order.
///
/// # Panics
///
/// Panics if the variable index is out of bounds.
///
/// # Examples
///
/// hp = 0..4 expands to `hp.0`, `hp.1`, `hp.2`, `hp.3`.
///
///
fn expand_to_bit_names(model: &Model, var_order: &[usize]) -> Vec<String> {
    let mut result = Vec::new();

    for &var_idx in var_order {
        let var = &model.variables[var_idx];
        let name = model.ast_names.get_ident(var._name);

        let bit_count = match &var.domain {
            Domain::Boolean => 1,
            Domain::Enum(ids) => calc_bits(ids.len()),
            Domain::Range { min, max } => calc_bits((max - min + 1) as usize),
        };

        if bit_count == 1 {
            result.push(name.to_string());
        } else {
            for bit in 0..bit_count {
                result.push(format!("{}.{}", name, bit));
            }
        }
    }

    result
}

/// Shuffles the given slice of usize values using the ChaCha8 RNG with the given seed.
///
/// # Arguments
///
/// * `slice` - The slice of usize values to shuffle.
/// * `seed` - The seed for the RNG.
///
fn shuffle_ordering(slice: &mut [usize], seed: u64) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    slice.shuffle(&mut rng);
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn chain_edges(n: usize) -> Vec<Hyperedge> {
        (0..n - 1)
            .map(|i| Hyperedge {
                vars: vec![i, i + 1],
                weight: 1.0,
            })
            .collect()
    }

    #[test]
    fn test_random_is_valid_permutation() {
        let n = 10;
        let mut order: Vec<usize> = (0..n).collect();
        shuffle_ordering(&mut order, 42);

        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..n).collect::<Vec<_>>());
    }

    #[test]
    fn test_random_seeded_is_reproducible() {
        let n = 10;
        let mut a: Vec<usize> = (0..n).collect();
        let mut b: Vec<usize> = (0..n).collect();
        shuffle_ordering(&mut a, 99);
        shuffle_ordering(&mut b, 99);
        assert_eq!(a, b);
    }

    #[test]
    fn test_random_different_seeds_differ() {
        let n = 10;
        let mut a: Vec<usize> = (0..n).collect();
        let mut b: Vec<usize> = (0..n).collect();
        shuffle_ordering(&mut a, 1);
        shuffle_ordering(&mut b, 2);
        assert_ne!(a, b);
    }

    #[test]
    fn test_span_cost_sorted_is_minimal_for_chain() {
        let n = 5;
        let edges = chain_edges(n);
        let sorted: Vec<usize> = (0..n).collect();
        let cost = span_cost(&sorted, &edges);
        // Each adjacent pair has span 1 → total = n-1
        assert_eq!(cost, (n - 1) as f64);
    }

    #[test]
    fn test_force_does_not_worsen_sorted_chain() {
        let n = 6;
        let edges = chain_edges(n);
        let sorted: Vec<usize> = (0..n).collect();
        let initial = span_cost(&sorted, &edges);
        let result = force_ordering(n, &edges, 20);
        assert!(span_cost(&result, &edges) <= initial);
    }

    #[test]
    fn test_force_improves_reversed_chain() {
        let n = 8;
        let edges = chain_edges(n);
        let reversed: Vec<usize> = (0..n).rev().collect();
        let initial = span_cost(&reversed, &edges);
        let result = force_ordering(n, &edges, 20);
        let improved = span_cost(&result, &edges);
        assert!(improved <= initial, "{} → {}", initial, improved);
    }
}
