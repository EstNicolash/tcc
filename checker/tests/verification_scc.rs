#[cfg(test)]
mod scc_integration_tests {
    // Assuming labelling_scc has a similar signature to the naive verify
    use checker::algorithms::labelling_scc::verify as verify_scc;
    use checker::{
        BinaryOp, CtlFormula, CtlFormulaArena, Domain, KripkeBuilder, KripkeStructure, Model,
        SsmvArena, SymbolicArena, SymbolicExpr, SymbolicExprID, Value, Variable,
    };

    /// Helper: Traffic light with a cycle: 0 -> 1 -> 2 -> 0
    fn setup_traffic_light_scc() -> (KripkeStructure, Model) {
        let mut ast_names = SsmvArena::new();
        let mut sym_arena = SymbolicArena::new();
        let mut ctl_arena = CtlFormulaArena::new();

        let light_var = Variable {
            _name: ast_names.intern_identifier("light"),
            domain: Domain::Range { min: 0, max: 2 },
        };

        // Create expressions for properties
        let var_ref = {
            let id = SymbolicExprID(sym_arena.expressions.len() as u32);
            sym_arena.expressions.push(SymbolicExpr::Reference(0));
            id
        };

        let val_0 = {
            let id = SymbolicExprID(sym_arena.expressions.len() as u32);
            sym_arena
                .expressions
                .push(SymbolicExpr::Literal(Value::Int(0)));
            id
        };

        // is_green: light == 0
        let is_green_expr = {
            let id = SymbolicExprID(sym_arena.expressions.len() as u32);
            sym_arena
                .expressions
                .push(SymbolicExpr::Binary(BinaryOp::Eq, var_ref, val_0));
            id
        };

        // Build Kripke Structure
        let mut builder = KripkeBuilder::new(1);
        let s0 = builder.states.get_or_insert(&vec![0]); // Green
        let s1 = builder.states.get_or_insert(&vec![1]); // Yellow
        let s2 = builder.states.get_or_insert(&vec![2]); // Red

        builder.add_initial_state(s0);
        builder.add_transition(s0, s1);
        builder.add_transition(s1, s2);
        builder.add_transition(s2, s0); // Create the cycle

        let ks = KripkeStructure::from_builder(builder);

        // Formulas to test
        let prop_green = ctl_arena.insert(CtlFormula::Prop(is_green_expr));

        // EG(is_green) should be FALSE (it must eventually leave Green)
        let spec_eg_green = ctl_arena.insert(CtlFormula::EG(prop_green));

        // AG(light < 3) => !EF(light >= 3) should be TRUE (normalized to !E[true U light >= 3])
        // For simplicity in this test, let's test EG(True) which is TRUE in a total system
        let t = ctl_arena.insert(CtlFormula::True);
        let spec_eg_true = ctl_arena.insert(CtlFormula::EG(t));

        let model = Model {
            variables: vec![light_var],
            init_assignments: vec![],
            next_assignments: vec![],
            specs: vec![spec_eg_green, spec_eg_true],
            arena: sym_arena,
            ast_names,
            ctl_arena,
        };

        (ks, model)
    }

    #[test]
    fn test_scc_eg_logic() {
        let (ks, model) = setup_traffic_light_scc();

        // Run the SCC-based verification
        let results = verify_scc(&ks, model);

        // Result 0: EG(light == 0)
        // Even though light=0 is in a cycle, it cannot stay in 0 forever.
        // The only SCC in the subgraph restricted to light=0 is a trivial one (no self-loop).
        assert_eq!(
            results[0], false,
            "EG(green) should be FALSE because the system must transition to yellow."
        );

        // Result 1: EG(True)
        // Since the whole system is a cycle, there is a path that stays in 'True' forever.
        assert_eq!(
            results[1], true,
            "EG(True) should be TRUE in a cyclic transition system."
        );
    }

    #[test]
    fn test_scc_deadlock_detection() {
        let mut ast_names = SsmvArena::new();
        let mut sym_arena = SymbolicArena::new();
        let mut ctl_arena = CtlFormulaArena::new();

        let var = Variable {
            _name: ast_names.intern_identifier("x"),
            domain: Domain::Boolean,
        };

        // Kripke: s0 (x=1) -> s1 (x=0) -> s1 (x=0)
        let mut builder = KripkeBuilder::new(1);
        let s0 = builder.states.get_or_insert(&vec![1]);
        let s1 = builder.states.get_or_insert(&vec![0]);
        builder.add_initial_state(s0);
        builder.add_transition(s0, s1);
        builder.add_transition(s1, s1);
        let ks = KripkeStructure::from_builder(builder);

        // Prop: x == 1
        let val_1 = {
            let id = SymbolicExprID(sym_arena.expressions.len() as u32);
            sym_arena
                .expressions
                .push(SymbolicExpr::Literal(Value::Int(1)));
            id
        };
        let var_ref = {
            let id = SymbolicExprID(sym_arena.expressions.len() as u32);
            sym_arena.expressions.push(SymbolicExpr::Reference(0));
            id
        };
        let is_one = {
            let id = SymbolicExprID(sym_arena.expressions.len() as u32);
            sym_arena
                .expressions
                .push(SymbolicExpr::Binary(BinaryOp::Eq, var_ref, val_1));
            id
        };

        let prop_one = ctl_arena.insert(CtlFormula::Prop(is_one));
        let spec_eg = ctl_arena.insert(CtlFormula::EG(prop_one));

        let model = Model {
            variables: vec![var],
            init_assignments: vec![],
            next_assignments: vec![],
            specs: vec![spec_eg],
            arena: sym_arena,
            ast_names,
            ctl_arena,
        };

        let results = verify_scc(&ks, model);

        assert_eq!(
            results[0], false,
            "EG(x=1) should be FALSE as it leads to a state where x=0."
        );
    }
}
