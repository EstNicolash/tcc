// bdd.rs
use crate::modeling::symbolic::{Domain, Model};
use oxidd::bdd::BDDFunction;
use oxidd::bdd::BDDManagerRef;
use oxidd::{BooleanFunction, Function, Manager, ManagerRef};

pub struct SymbolicContext {
    pub manager: BDDManagerRef,
    pub var_map: Vec<VarBits>,
    pub initial_states: BDDFunction,
    pub transition_relation: BDDFunction,
}

pub struct VarBits {
    pub curr: Vec<u32>,
    pub next: Vec<u32>,
}

impl SymbolicContext {
    pub fn new(model: &Model) -> Self {
        let manager = oxidd::bdd::new_manager(2_000_000, 1_000_000, 1);

        let mut var_map = Vec::with_capacity(model.variables.len());

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
        });

        SymbolicContext { manager, var_map }
    }
}

fn calc_bits(states: usize) -> usize {
    if states <= 1 {
        1
    } else {
        (states as f64).log2().ceil() as usize
    }
}

/// Full Adder: Returns the bitwise sum of two numbers using a full adder
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
pub fn ripple_carry_adder(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> Vec<BDDFunction> {
    let mut result_bits = Vec::with_capacity(lhs.len());

    let mut current_carry = manager.with_manager_shared(|m| BDDFunction::f(m));

    for i in 0..lhs.len() {
        let (sum_bit, next_carry) = bdd_full_adder(&lhs[i], &rhs[i], &current_carry);

        result_bits.push(sum_bit);

        current_carry = next_carry;
    }

    result_bits
}

/// Equal (A == B): Returns True if A and B are bitwise equal
pub fn bdd_number_eq(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    let mut result = manager.with_manager_shared(|m| BDDFunction::t(m));

    for (l, r) in lhs.iter().zip(rhs.iter()) {
        let bit_equal = l.equiv(r);

        result = result.and(&bit_equal.unwrap()).unwrap();
    }

    result
}
/// Subtract (A - B): Returns the bitwise subtraction of B from A
pub fn bdd_number_sub(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> Vec<BDDFunction> {
    let mut result_bits = Vec::with_capacity(lhs.len());

    let mut current_carry = manager.with_manager_shared(|m| BDDFunction::t(m));

    for i in 0..lhs.len() {
        // Invert the bit of the right-hand side (NOT B)
        let not_b = rhs[i].not().unwrap();

        // Add A + (NOT B) + 1
        let (sum_bit, next_carry) = bdd_full_adder(&lhs[i], &not_b, &current_carry);

        result_bits.push(sum_bit);
        current_carry = next_carry;
    }

    result_bits
}

/// Greater Than (A > B): Returns True if A is strictly greater than B
pub fn bdd_number_gt(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    // Start with False (if numbers are identical, A is not strictly greater than B)
    let mut is_gt = manager.with_manager_shared(|m| BDDFunction::f(m));

    for i in 0..lhs.len() {
        let a_gt_b_here = rhs[i].imp_strict(&lhs[i]).unwrap();
        // Condition 2: A and B are equal at this bit
        let a_eq_b_here = lhs[i].equiv(&rhs[i]).unwrap();

        // If they are equal here, the result depends on the lower bits evaluated so far
        let eq_and_prev_gt = a_eq_b_here.and(&is_gt).unwrap();

        // The final result for this stage
        is_gt = a_gt_b_here.or(&eq_and_prev_gt).unwrap();
    }

    is_gt
}

/// Not Equal (A != B): Simply NOT(A == B)
pub fn bdd_number_neq(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    bdd_number_eq(lhs, rhs, manager).not().unwrap()
}

/// Less Than (A < B): Equivalent to (B > A)
pub fn bdd_number_lt(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    bdd_number_gt(rhs, lhs, manager)
}

/// Less Than or Equal (A <= B): Equivalent to NOT(A > B)
pub fn bdd_number_lte(
    lhs: &[BDDFunction],
    rhs: &[BDDFunction],
    manager: &BDDManagerRef,
) -> BDDFunction {
    bdd_number_gt(lhs, rhs, manager).not().unwrap()
}

/// Greater Than or Equal (A >= B): Equivalent to NOT(A < B)
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
    use oxidd::{
        BooleanFunction, BooleanFunctionQuant, FunctionSubst, Manager, ManagerRef, Subst,
        Substitution,
    };

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
