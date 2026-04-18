// tests/verification.rs
use checker::{
    BinaryOp, CtlFormula, CtlFormulaArena, Domain, KripkeBuilder, KripkeStructure, Model,
    SsmvArena, SymbolicArena, SymbolicExpr, SymbolicExprID, Value, Variable, verify,
};

/// Helper to create a basic symbolic traffic light setup:
/// Green (0) -> Yellow (1) -> Red (2) -> Green (0)
fn setup_traffic_light() -> (KripkeStructure, Model) {
    let mut ast_names = SsmvArena::new();
    let mut sym_arena = SymbolicArena::new();
    let mut ctl_arena = CtlFormulaArena::new();

    let light_name = ast_names.intern_identifier("light");
    let light_var = Variable {
        name: light_name,
        domain: Domain::Range { min: 0, max: 2 },
    };

    // 1. Defina a Proposição Simbólica (light == 2)
    let var_expr = {
        let id = SymbolicExprID(sym_arena.expressions.len() as u32);
        sym_arena.expressions.push(SymbolicExpr::Reference(0)); // Variável 0
        id
    };
    let val_2 = {
        let id = SymbolicExprID(sym_arena.expressions.len() as u32);
        sym_arena
            .expressions
            .push(SymbolicExpr::Literal(Value::Int(2)));
        id
    };
    let is_red_expr = {
        let id = SymbolicExprID(sym_arena.expressions.len() as u32);
        sym_arena
            .expressions
            .push(SymbolicExpr::Binary(BinaryOp::Eq, var_expr, val_2));
        id
    };

    // 2. Construa a Kripke Structure
    let mut builder = KripkeBuilder::new(1);
    // IMPORTANTE: Insira os estados e guarde os IDs
    let s0 = builder.states.get_or_insert(&vec![0]); // Green
    let s1 = builder.states.get_or_insert(&vec![1]); // Yellow
    let s2 = builder.states.get_or_insert(&vec![2]); // Red (Valor 2!)

    builder.add_initial_state(s0);
    builder.add_transition(s0, s1);
    builder.add_transition(s1, s2);
    builder.add_transition(s2, s0);

    let ks = KripkeStructure::from_builder(builder);

    // 3. Monte o Modelo
    let prop_red = ctl_arena.insert(CtlFormula::Prop(is_red_expr));
    let spec0 = ctl_arena.insert(CtlFormula::EF(prop_red));

    let mut model = Model {
        variables: vec![light_var],
        init_assignments: vec![],
        next_assignments: vec![],
        specs: vec![spec0],
        arena: sym_arena,
        ast_names,
        ctl_arena,
    };

    (ks, model)
}

#[test]
fn test_traffic_light_full_verification() {
    let (ks, model) = setup_traffic_light();

    // The verify function handles CTL purification (EF -> EU)
    let results = verify(&ks, model);

    assert!(results.len() > 0);
    assert!(
        results[0],
        "EF(is_red) should be TRUE. Path: Green(0) -> Yellow(1) -> Red(2)"
    );
}

#[test]
fn test_simple_boolean_logic() {
    let mut ast_names = SsmvArena::new();
    let mut sym_arena = SymbolicArena::new();
    let mut ctl_arena = CtlFormulaArena::new();

    let var = Variable {
        name: ast_names.intern_identifier("x"),
        domain: Domain::Boolean,
    };

    let mut builder = KripkeBuilder::new(1);
    let s0 = builder.states.get_or_insert(&vec![0]);
    builder.add_initial_state(s0);
    let ks = KripkeStructure::from_builder(builder);

    // Spec: True AND (NOT False)
    let t = ctl_arena.insert(CtlFormula::True);
    let f = ctl_arena.insert(CtlFormula::False);
    let not_f = ctl_arena.insert(CtlFormula::Not(f));
    let spec = ctl_arena.insert(CtlFormula::And(t, not_f));

    let model = Model {
        variables: vec![var],
        init_assignments: vec![],
        next_assignments: vec![],
        specs: vec![spec],
        arena: sym_arena,
        ast_names,
        ctl_arena,
    };

    let results = verify(&ks, model);
    assert!(
        results[0],
        "Basic boolean logic (True AND NOT False) failed."
    );
}
