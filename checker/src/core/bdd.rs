///! # Module bdd
///! BDD utilities for symbolic model checking.
///
use crate::modeling::symbolic::{Domain, Model};
use oxidd::bdd::BDDFunction;
use oxidd::bdd::BDDManagerRef;
use oxidd::{BooleanFunction, FunctionSubst, Manager, ManagerRef, Subst, VarNo};

/// A context for symbolic BDD operations, managing variable mappings and formal structures.
///
/// This context acts as the bridge between the high-level CTL formulas and the
/// low-level BDD representations, handling variable ordering and state transitions.
pub struct SymbolicContext {
    /// The OxiDD manager that handles node allocation.
    pub manager: BDDManagerRef,

    /// Maps each model variable to its corresponding bit indices for current and next states.
    pub var_map: Vec<VarBits>,

    /// BDD representing the set of initial states: I(s).
    pub initial_states: Option<BDDFunction>,

    /// BDD representing the transition relation: Delta(s, s').
    pub transition_relation: Option<BDDFunction>,

    /// Internal list of variable indices for the current state (used for substitution).
    curr_ids: Vec<VarNo>,

    /// Internal BDD functions for the next state variables (used for substitution).
    next_bdds: Vec<BDDFunction>,

    /// The conjunction of all next-state variables. Used as a 'cube' for existential
    /// quantification in the EX, EU, and EG operators.
    pub next_vars_cube: BDDFunction,
}

pub struct VarBits {
    pub curr: Vec<u32>,
    pub next: Vec<u32>,
}
/// Resolves a bit name to a variable index and bit index in the model.
fn resolve_bit_name(name: &str, model: &Model) -> Option<(usize, usize)> {
    if name.contains('.') {
        let parts: Vec<&str> = name.split('.').collect();
        let var_name = parts[0];
        let bit_idx = parts[1].parse::<usize>().ok()?;

        let var_idx = model
            .variables
            .iter()
            .position(|v| model.ast_names.get_ident(v._name) == var_name)?;

        Some((var_idx, bit_idx))
    } else {
        let var_idx = model
            .variables
            .iter()
            .position(|v| model.ast_names.get_ident(v._name) == name)?;

        Some((var_idx, 0))
    }
}
impl SymbolicContext {
    /// Creates a new symbolic context from the given model.
    ///
    /// Initializes the BDD manager and builds the variable map.
    /// The variables are ordered by their index in the model.
    /// Each model variable is mapped to two BDD variable, one for the current state and one for the next state.
    pub fn new(model: &Model, explicit_order: Option<Vec<String>>) -> Self {
        let manager = oxidd::bdd::new_manager(40_000_000, 1_000_000, 1);

        let mut var_map: Vec<VarBits> = (0..model.variables.len())
            .map(|_| VarBits {
                curr: vec![],
                next: vec![],
            })
            .collect();

        manager.with_manager_exclusive(|m| {
            // 1. Pre-allocate VarBits vectors with the correct size using a sentinel value
            for (var_idx, var) in model.variables.iter().enumerate() {
                let bit_count = match &var.domain {
                    Domain::Boolean => 1,
                    Domain::Enum(ids) => calc_bits(ids.len()),
                    Domain::Range { min, max } => {
                        let max_val = if *max > 0 { *max as usize } else { 0 };
                        calc_bits(max_val + 1)
                    }
                };

                // Initialize with u32::MAX to track which bits are still unassigned
                var_map[var_idx].curr.resize(bit_count, u32::MAX);
                var_map[var_idx].next.resize(bit_count, u32::MAX);
            }

            // 2. Process the explicit variable order by injecting bits into exact indices
            if let Some(order) = explicit_order {
                for bit_name in order {
                    if let Some((var_idx, bit_idx)) = resolve_bit_name(&bit_name, model) {
                        // Safety check: ensure the bit index is within the variable's domain range
                        if bit_idx < var_map[var_idx].curr.len() {
                            // Only allocate BDD levels if this bit has not been defined yet
                            if var_map[var_idx].curr[bit_idx] == u32::MAX {
                                let range = m.add_vars(2);
                                var_map[var_idx].curr[bit_idx] = range.start;
                                var_map[var_idx].next[bit_idx] = range.start + 1;
                            }
                        }
                    }
                }
            }

            // 3. Fallback: Allocate BDD variables for any bits missed by the oracle or order file
            for var_idx in 0..var_map.len() {
                for bit_idx in 0..var_map[var_idx].curr.len() {
                    if var_map[var_idx].curr[bit_idx] == u32::MAX {
                        let range = m.add_vars(2);
                        var_map[var_idx].curr[bit_idx] = range.start;
                        var_map[var_idx].next[bit_idx] = range.start + 1;
                    }
                }
            }
        });

        /*
        manager.with_manager_exclusive(|m| {
            for var in &model.variables {
                let bit_count = match &var.domain {
                    Domain::Boolean => 1,
                    Domain::Enum(ids) => calc_bits(ids.len()),
                    Domain::Range { min, max } => calc_bits((max - min + 1) as usize),
                };

                let mut curr_bits = Vec::with_capacity(bit_count);
                let mut next_bits = Vec::with_capacity(bit_count);

                for _ in 0..bit_count {
                    let range = m.add_vars(2);
                    curr_bits.push(range.start);
                    next_bits.push(range.start + 1);
                }

                var_map.push(VarBits {
                    curr: curr_bits,
                    next: next_bits,
                });
            }
        });*/

        let mut curr_ids = Vec::new();
        let mut next_bdds = Vec::new();

        for var_bits in &var_map {
            for (curr, next) in var_bits.curr.iter().zip(&var_bits.next) {
                curr_ids.push(*curr);
                let bdd = manager.with_manager_shared(|m| {
                    BDDFunction::var(m, *next).expect("Failed to create next var BDD")
                });
                next_bdds.push(bdd);
            }
        }

        let next_vars_cube = next_bdds.iter().fold(
            manager.with_manager_shared(|m| BDDFunction::t(m)),
            |acc, bdd| acc.and(bdd).unwrap(),
        );
        SymbolicContext {
            manager,
            var_map,
            initial_states: None,
            transition_relation: None,
            curr_ids,
            next_bdds,
            next_vars_cube,
        }
    }
    /// Computes the relational shift f[s -> s'].
    ///
    /// Replaces all current-state variables in the BDD with their next-state counterparts.
    /// This is a fundamental operation for computing the image of a set of states.
    ///
    /// # Panics
    /// Panics if the substitution operation fails within the OxiDD manager.
    pub fn shift_curr_to_next(&self, bdd: &BDDFunction) -> BDDFunction {
        let subst = Subst::new(&self.curr_ids, &self.next_bdds);
        bdd.substitute(&subst).expect("Substitution failed")
    }
}

/// Calculates the number of bits required to represent a given domain size.
///
/// This is used to determine the number of BDD variables needed for representation of ssmv variable.
///
/// # Arguments
///
/// * `domain_size` - The size of the domain to represent.
///
/// # Examples
///
/// # Returns
/// The number of bits required to represent the domain size.
pub fn calc_bits(domain_size: usize) -> usize {
    if domain_size <= 1 {
        1
    } else {
        (domain_size as f64).log2().ceil() as usize
    }
}

fn pad_to_max(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> (Vec<BDDFunction>, Vec<BDDFunction>) {
    let max_len = std::cmp::max(lhs.len(), rhs.len());
    let mut new_lhs = lhs.to_vec();
    let mut new_rhs = rhs.to_vec();

    if new_lhs.len() < max_len {
        let f_node = manager.with_manager_shared(|m| BDDFunction::f(m));
        new_lhs.resize(max_len, f_node);
    }
    if new_rhs.len() < max_len {
        let f_node = manager.with_manager_shared(|m| BDDFunction::f(m));
        new_rhs.resize(max_len, f_node);
    }

    (new_lhs, new_rhs)
}

/// Full Adder: Returns the bitwise sum of two numbers using a full adder
///
/// # Arguments
///
/// * `a` - The first operand.
/// * `b` - The second operand.
/// * `carry_in` - The carry input.
///
/// # Returns
/// The result of the full adder as a tuple of BDD functions.
pub fn bdd_full_adder(
    a: &BDDFunction,
    b: &BDDFunction,
    carry_in: &BDDFunction,
) -> (BDDFunction, BDDFunction) {
    // sum = a XOR b XOR carry_in
    let a_xor_b = a.xor(b).unwrap();
    let sum = a_xor_b.xor(carry_in).unwrap();

    // carry_out = (a AND b) OR (carry_in AND (a XOR b))
    let a_and_b = a.and(b).unwrap();
    let carry_and_xor = carry_in.and(&a_xor_b).unwrap();
    let carry_out = a_and_b.or(&carry_and_xor).unwrap();

    (sum, carry_out)
}
/// Ripple Carry Adder: Returns the bitwise sum of two numbers using a ripple carry adder
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the ripple carry adder as a vector of BDD functions.
pub fn ripple_carry_adder(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> Vec<BDDFunction> {
    let (padded_lhs, padded_rhs) = pad_to_max(lhs, rhs, manager);
    let mut result_bits = Vec::with_capacity(padded_lhs.len());
    let mut current_carry = manager.with_manager_shared(|m| BDDFunction::f(m));

    for i in 0..padded_lhs.len() {
        let (sum_bit, next_carry) = bdd_full_adder(&padded_lhs[i], &padded_rhs[i], &current_carry);
        result_bits.push(sum_bit);
        current_carry = next_carry;
    }
    result_bits
}

/// Equal (A == B): Returns True if A and B are bitwise equal
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the equality comparison as a BDD function.
pub fn bdd_number_eq(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    let (padded_lhs, padded_rhs) = pad_to_max(lhs, rhs, manager);
    let mut result = manager.with_manager_shared(|m| BDDFunction::t(m));

    for (l, r) in padded_lhs.iter().zip(padded_rhs.iter()) {
        let bit_equal = l.equiv(r);
        result = result.and(&bit_equal.unwrap()).unwrap();
    }
    result
}
/// Subtract (A - B): Returns the bitwise subtraction of B from A
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the subtraction as a vector of BDD functions.
pub fn bdd_number_sub(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> Vec<BDDFunction> {
    let (padded_lhs, padded_rhs) = pad_to_max(lhs, rhs, manager);
    let mut result_bits = Vec::with_capacity(padded_lhs.len());
    let mut current_carry = manager.with_manager_shared(|m| BDDFunction::t(m));

    for i in 0..padded_lhs.len() {
        // Invert the bit of the right-hand side (NOT B)
        let not_b = padded_rhs[i].not().unwrap();
        // Add A + (NOT B) + 1
        let (sum_bit, next_carry) = bdd_full_adder(&padded_lhs[i], &not_b, &current_carry);
        result_bits.push(sum_bit);
        current_carry = next_carry;
    }
    result_bits
}
/// Greater Than (A > B): Returns True if A is strictly greater than B
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the greater than comparison as a BDD function.
pub fn bdd_number_gt(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    let (padded_lhs, padded_rhs) = pad_to_max(lhs, rhs, manager);

    // Start with False (if numbers are identical, A is not strictly greater than B)
    let mut is_gt = manager.with_manager_shared(|m| BDDFunction::f(m));

    for i in 0..padded_lhs.len() {
        let a_gt_b_here = padded_rhs[i].imp_strict(&padded_lhs[i]).unwrap();
        // Condition 2: A and B are equal at this bit
        let a_eq_b_here = padded_lhs[i].equiv(&padded_rhs[i]).unwrap();
        // If they are equal here, the result depends on the lower bits evaluated so far
        let eq_and_prev_gt = a_eq_b_here.and(&is_gt).unwrap();
        // The final result for this stage
        is_gt = a_gt_b_here.or(&eq_and_prev_gt).unwrap();
    }
    is_gt
}

/// Not Equal (A != B): Simply NOT(A == B)
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the not equal comparison as a BDD function.
pub fn bdd_number_neq(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    bdd_number_eq(lhs, rhs, manager).not().unwrap()
}

/// Less Than (A < B): Equivalent to (B > A)
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the less than comparison as a BDD function.
pub fn bdd_number_lt(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    bdd_number_gt(rhs, lhs, manager)
}

/// Less Than or Equal (A <= B): Equivalent to NOT(A > B)
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the less than or equal comparison as a BDD function.
pub fn bdd_number_lte(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    bdd_number_gt(lhs, rhs, manager).not().unwrap()
}

/// Greater Than or Equal (A >= B): Equivalent to NOT(A < B)
///
/// # Arguments
///
/// * `lhs` - The left-hand side operand (A).
/// * `rhs` - The right-hand side operand (B).
/// * `manager` - The BDD manager reference.
///
/// # Returns
/// The result of the greater than or equal comparison as a BDD function.
pub fn bdd_number_gte(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    bdd_number_lt(lhs, rhs, manager).not().unwrap()
}

#[cfg(test)]
mod tests {
    use crate::core::bdd::{
        bdd_number_eq, bdd_number_gt, bdd_number_gte, bdd_number_lt, bdd_number_lte,
        bdd_number_neq, bdd_number_sub, ripple_carry_adder,
    };
    use oxidd::bdd::BDDFunction;
    use oxidd::{BooleanFunction, BooleanFunctionQuant, FunctionSubst, Manager, ManagerRef, Subst};

    const BIT_WIDTH: usize = 3;
    const MAX_VAL: u32 = 1 << BIT_WIDTH;

    #[test]
    fn test_variable_creation_and_logic() {
        let manager_ref = oxidd::bdd::new_manager(1024, 512, 1);

        let (x_id, y_id) = manager_ref.with_manager_exclusive(|manager| {
            let var_range = manager.add_vars(2);
            (var_range.start, var_range.start + 1)
        });

        let (x, y, false_node) = manager_ref.with_manager_shared(|manager| {
            (
                BDDFunction::var(manager, x_id).unwrap(),
                BDDFunction::var(manager, y_id).unwrap(),
                BDDFunction::f(manager),
            )
        });

        let func = x.and(&y).unwrap().or(&x.not().unwrap()).unwrap();
        assert!(func.satisfiable(), "The function should be satisfiable");

        let contradiction = x.and(&x.not().unwrap()).unwrap();
        assert!(
            !contradiction.satisfiable(),
            "x AND NOT x must be unsatisfiable"
        );
        assert!(
            contradiction == false_node,
            "Contradiction must equal the False terminal"
        );

        println!("Variable creation and basic logic test passed.");
    }

    #[test]
    fn test_structural_equality() {
        let manager_ref = oxidd::bdd::new_manager(1024, 512, 1);

        let (x_id, y_id) = manager_ref.with_manager_exclusive(|manager| {
            let r = manager.add_vars(2);
            (r.start, r.start + 1)
        });

        let (x, y) = manager_ref.with_manager_shared(|manager| {
            (
                BDDFunction::var(manager, x_id).unwrap(),
                BDDFunction::var(manager, y_id).unwrap(),
            )
        });

        // f1: x implies y  (x -> y)
        let f1 = x.imp(&y).unwrap();

        // f2: !x OR y
        let f2 = x.not().unwrap().or(&y).unwrap();

        assert!(
            f1 == f2,
            "Equivalent formulas must have identical BDD representations"
        );
        println!("Canonicity test passed.");
    }

    // 3. Existential Quantification (For CTL 'EX' operator)
    #[test]
    fn test_existential_quantification() {
        let manager_ref = oxidd::bdd::new_manager(1024, 512, 1);

        let (x_id, y_id) = manager_ref.with_manager_exclusive(|manager| {
            let r = manager.add_vars(2);
            (r.start, r.start + 1)
        });

        let (x, y) = manager_ref.with_manager_shared(|manager| {
            (
                BDDFunction::var(manager, x_id).unwrap(),
                BDDFunction::var(manager, y_id).unwrap(),
            )
        });

        // f = x AND y
        let f = x.and(&y).unwrap();

        // Existential quantification: exists x . (x AND y)
        let result = f.exists(&x).unwrap();

        assert!(result == y, "exists x. (x AND y) should simplify to just y");
        println!("Existential quantification test passed.");
    }

    // 4. Substitution (Renaming variables for 'Next State' transitions)
    #[test]
    fn test_variable_substitution() {
        let manager_ref = oxidd::bdd::new_manager(1024, 512, 1);

        let (x_curr_id, x_next_id) = manager_ref.with_manager_exclusive(|manager| {
            let r = manager.add_vars(2);
            (r.start, r.start + 1)
        });

        let (x_curr, x_next) = manager_ref.with_manager_shared(|manager| {
            (
                BDDFunction::var(manager, x_curr_id).unwrap(),
                BDDFunction::var(manager, x_next_id).unwrap(),
            )
        });

        let vars_to_replace = vec![x_curr_id];
        let replacements = vec![x_next.clone()];
        let substitution = Subst::new(&vars_to_replace, &replacements);

        let renamed_prop = x_curr.substitute(&substitution).unwrap();

        assert!(
            renamed_prop == x_next,
            "The renamed property should be identical to x_next"
        );
        assert!(
            renamed_prop != x_curr,
            "The renamed property should no longer be x_curr"
        );

        println!("Substitution (renaming) test passed.");
    }

    #[test]
    fn test_alu_exhaustive() {
        let manager_ref = oxidd::bdd::new_manager(1024, 512, 1);

        let (a_ids, b_ids) = manager_ref.with_manager_exclusive(|m| {
            let range_a = m.add_vars(BIT_WIDTH as u32);
            let range_b = m.add_vars(BIT_WIDTH as u32);

            let a: Vec<u32> = (range_a.start..range_a.end).collect();
            let b: Vec<u32> = (range_b.start..range_b.end).collect();
            (a, b)
        });

        let (a_vars, b_vars) = manager_ref.with_manager_shared(|m| {
            let a = a_ids
                .iter()
                .map(|&id| BDDFunction::var(m, id).unwrap())
                .collect::<Vec<_>>();
            let b = b_ids
                .iter()
                .map(|&id| BDDFunction::var(m, id).unwrap())
                .collect::<Vec<_>>();
            (a, b)
        });

        let eq_bdd = bdd_number_eq(&a_vars, &b_vars, &manager_ref);
        let neq_bdd = bdd_number_neq(&a_vars, &b_vars, &manager_ref);
        let gt_bdd = bdd_number_gt(&a_vars, &b_vars, &manager_ref);
        let lt_bdd = bdd_number_lt(&a_vars, &b_vars, &manager_ref);
        let gte_bdd = bdd_number_gte(&a_vars, &b_vars, &manager_ref);
        let lte_bdd = bdd_number_lte(&a_vars, &b_vars, &manager_ref);

        let add_bdds = ripple_carry_adder(&a_vars, &b_vars, &manager_ref);
        let sub_bdds = bdd_number_sub(&a_vars, &b_vars, &manager_ref);

        for a_val in 0..MAX_VAL {
            for b_val in 0..MAX_VAL {
                let mut assignment: Vec<(u32, bool)> = Vec::new();
                for bit in 0..BIT_WIDTH {
                    let a_bit_is_1 = (a_val >> bit) & 1 == 1;
                    let b_bit_is_1 = (b_val >> bit) & 1 == 1;

                    assignment.push((a_ids[bit], a_bit_is_1));
                    assignment.push((b_ids[bit], b_bit_is_1));
                }

                assert_eq!(
                    eq_bdd.eval(assignment.clone()),
                    a_val == b_val,
                    "Failed Eq for {} == {}",
                    a_val,
                    b_val
                );
                assert_eq!(
                    neq_bdd.eval(assignment.clone()),
                    a_val != b_val,
                    "Failed Neq for {} != {}",
                    a_val,
                    b_val
                );
                assert_eq!(
                    gt_bdd.eval(assignment.clone()),
                    a_val > b_val,
                    "Failed Gt for {} > {}",
                    a_val,
                    b_val
                );
                assert_eq!(
                    lt_bdd.eval(assignment.clone()),
                    a_val < b_val,
                    "Failed Lt for {} < {}",
                    a_val,
                    b_val
                );
                assert_eq!(
                    gte_bdd.eval(assignment.clone()),
                    a_val >= b_val,
                    "Failed Gte for {} >= {}",
                    a_val,
                    b_val
                );
                assert_eq!(
                    lte_bdd.eval(assignment.clone()),
                    a_val <= b_val,
                    "Failed Lte for {} <= {}",
                    a_val,
                    b_val
                );

                let mut add_result = 0;
                for bit in 0..BIT_WIDTH {
                    if add_bdds[bit].eval(assignment.clone()) {
                        add_result |= 1 << bit;
                    }
                }
                let expected_add = (a_val + b_val) & (MAX_VAL - 1);
                assert_eq!(
                    add_result, expected_add,
                    "Failed Add for {} + {}",
                    a_val, b_val
                );

                let mut sub_result = 0;
                for bit in 0..BIT_WIDTH {
                    if sub_bdds[bit].eval(assignment.clone()) {
                        sub_result |= 1 << bit;
                    }
                }
                let expected_sub = (a_val.wrapping_sub(b_val)) & (MAX_VAL - 1);
                assert_eq!(
                    sub_result, expected_sub,
                    "Failed Sub for {} - {}",
                    a_val, b_val
                );
            }
        }
        println!("All 64 combinations tested successfully for 3-bit ALU!");
    }
}
