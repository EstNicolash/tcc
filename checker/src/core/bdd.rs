// bdd.rs
use crate::modeling::symbolic::{Domain, Model};
use oxidd::bdd::BDDFunction;
use oxidd::bdd::BDDManagerRef;
use oxidd::{Manager, ManagerRef};

pub struct SymbolicContext {
    manager: BDDManagerRef,
    var_map: Vec<VarBits>,
}

struct VarBits {
    curr: Vec<u32>,
    next: Vec<u32>,
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

#[cfg(test)]
mod tests {
    use oxidd::bdd::BDDFunction;
    use oxidd::{
        BooleanFunction, BooleanFunctionQuant, FunctionSubst, Manager, ManagerRef, Subst,
        Substitution,
    };

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
}
