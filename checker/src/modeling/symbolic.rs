use crate::modeling::ssmv_ast::{
    ExprID, IdentifierID, SsmvArena, SsmvAssignment, SsmvExpr, SsmvModel, SsmvType,
};
use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolicExprID(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Enum(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolicExpr {
    Literal(Value),
    Reference(usize),
    Binary(BinaryOp, SymbolicExprID, SymbolicExprID),
    Unary(UnaryOp, SymbolicExprID),
    Case { start: u32, len: u32 },
    Set { start: u32, len: u32 },
}

pub struct SymbolicArena {
    pub expressions: Vec<SymbolicExpr>,
    pub case_buffer: Vec<(SymbolicExprID, SymbolicExprID)>,
    pub set_buffer: Vec<SymbolicExprID>,
    expr_lookup: HashMap<SymbolicExpr, SymbolicExprID>,
}

impl SymbolicArena {
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
            case_buffer: Vec::new(),
            set_buffer: Vec::new(),
            expr_lookup: HashMap::new(),
        }
    }

    pub fn alloc_expr(&mut self, expr: SymbolicExpr) -> SymbolicExprID {
        if let Some(&id) = self.expr_lookup.get(&expr) {
            return id;
        }
        let id = SymbolicExprID(self.expressions.len() as u32);
        self.expr_lookup.insert(expr.clone(), id);
        self.expressions.push(expr);
        id
    }
}

pub struct Model {
    pub variables: Vec<Variable>,
    pub init_assignments: Vec<(usize, SymbolicExprID)>,
    pub next_assignments: Vec<(usize, SymbolicExprID)>,
    pub specs: Vec<FormulaID>,
    pub arena: SymbolicArena,
    pub ast_names: SsmvArena,
    pub ctl_arena: CtlFormulaArena<SymbolicExprID>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    And,
    Or,
    Imply,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Not,
    Neg,
}

pub struct Variable {
    pub name: IdentifierID,
    pub domain: Domain,
}

#[derive(Debug, Clone)]
pub enum Domain {
    Boolean,
    Range { min: i32, max: i32 },
    Enum(Vec<IdentifierID>),
}

fn map_bin_op(op: &str) -> BinaryOp {
    match op {
        "&" | "and" => BinaryOp::And,
        "|" | "or" => BinaryOp::Or,
        "->" | "imply" => BinaryOp::Imply,
        "=" => BinaryOp::Eq,
        "!=" => BinaryOp::Neq,
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Sub,
        "*" => BinaryOp::Mul,
        "/" => BinaryOp::Div,
        "<" => BinaryOp::Lt,
        "<=" => BinaryOp::Lte,
        ">" => BinaryOp::Gt,
        ">=" => BinaryOp::Gte,
        _ => panic!("Unknown binary operator: {}", op),
    }
}

fn map_un_op(op: &str) -> UnaryOp {
    match op {
        "!" | "not" => UnaryOp::Not,
        "-" | "neg" => UnaryOp::Neg,
        _ => panic!("Unknown unary operator: {}", op),
    }
}

fn translate_and_purify_ctl(
    ast_f_id: FormulaID,
    old_arena: &CtlFormulaArena<ExprID>,
    new_arena: &mut CtlFormulaArena<SymbolicExprID>,
    memo: &mut HashMap<FormulaID, FormulaID>,
    ast_arena: &SsmvArena,
    sym_arena: &mut SymbolicArena,
    var_map: &HashMap<IdentifierID, usize>,
    def_map: &HashMap<IdentifierID, ExprID>,
    enum_map: &HashMap<IdentifierID, (usize, usize)>,
) -> FormulaID {
    if let Some(&new_id) = memo.get(&ast_f_id) {
        return new_id;
    }

    let formula = old_arena.get(ast_f_id);

    let mut conv = |f| {
        translate_and_purify_ctl(
            f, old_arena, new_arena, memo, ast_arena, sym_arena, var_map, def_map, enum_map,
        )
    };

    let new_id = match formula {
        CtlFormula::Prop(ast_expr_id) => {
            let mut stack = Vec::new();
            let sym_expr_id = translate_expr(
                *ast_expr_id,
                ast_arena,
                sym_arena,
                var_map,
                def_map,
                enum_map,
                &mut stack,
            );
            new_arena.insert(CtlFormula::Prop(sym_expr_id))
        }

        CtlFormula::EG(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let af_not_f = new_arena.insert(CtlFormula::AF(not_f));
            new_arena.insert(CtlFormula::Not(af_not_f))
        }
        CtlFormula::AG(f) => {
            let f_c = conv(*f);
            let not_f = new_arena.insert(CtlFormula::Not(f_c));
            let t_id = new_arena.insert(CtlFormula::True);
            let eu = new_arena.insert(CtlFormula::EU(t_id, not_f));
            new_arena.insert(CtlFormula::Not(eu))
        }
        CtlFormula::EF(f) => {
            let f_c = conv(*f);
            let t_id = new_arena.insert(CtlFormula::True);
            new_arena.insert(CtlFormula::EU(t_id, f_c))
        }
        CtlFormula::AU(f1, f2) => {
            let f1_c = conv(*f1);
            let f2_c = conv(*f2);
            let not_f1 = new_arena.insert(CtlFormula::Not(f1_c));
            let not_f2 = new_arena.insert(CtlFormula::Not(f2_c));
            let and_n1_n2 = new_arena.insert(CtlFormula::And(not_f1, not_f2));
            let eu = new_arena.insert(CtlFormula::EU(not_f2, and_n1_n2));
            let af_f2 = new_arena.insert(CtlFormula::AF(f2_c));
            let not_af = new_arena.insert(CtlFormula::Not(af_f2));
            let or_f = new_arena.insert(CtlFormula::Or(eu, not_af));
            new_arena.insert(CtlFormula::Not(or_f))
        }

        CtlFormula::Not(f) => {
            let c = conv(*f);
            new_arena.insert(CtlFormula::Not(c))
        }
        CtlFormula::And(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::And(c1, c2))
        }
        CtlFormula::Or(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::Or(c1, c2))
        }
        CtlFormula::Imply(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::Imply(c1, c2))
        }
        CtlFormula::EX(f) => {
            let c = conv(*f);
            new_arena.insert(CtlFormula::EX(c))
        }
        CtlFormula::AX(f) => {
            let c = conv(*f);
            new_arena.insert(CtlFormula::AX(c))
        }
        CtlFormula::AF(f) => {
            let c = conv(*f);
            new_arena.insert(CtlFormula::AF(c))
        }
        CtlFormula::EU(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::EU(c1, c2))
        }
        CtlFormula::Iff(f1, f2) => {
            let c1 = conv(*f1);
            let c2 = conv(*f2);
            new_arena.insert(CtlFormula::Iff(c1, c2))
        }
        CtlFormula::True => new_arena.insert(CtlFormula::True),
        CtlFormula::False => new_arena.insert(CtlFormula::False),
        _ => panic!("Unknown operator {:?}", formula),
    };

    memo.insert(ast_f_id, new_id);
    new_id
}

pub fn build_model(ast: SsmvModel) -> Model {
    let mut sym_arena = SymbolicArena::new();

    let (var_map, def_map, enum_map) = build_indices(&ast);

    let variables = ast
        .variables
        .iter()
        .map(|v| Variable {
            name: v.name,
            domain: match &v.data_type {
                SsmvType::Boolean => Domain::Boolean,
                SsmvType::Range(lo, hi) => Domain::Range { min: *lo, max: *hi },
                SsmvType::Enum(ids) => Domain::Enum(ids.clone()),
            },
        })
        .collect();

    let mut translate = |expr_id: ExprID| {
        translate_expr(
            expr_id,
            &ast.arena,
            &mut sym_arena,
            &var_map,
            &def_map,
            &enum_map,
            &mut Vec::new(),
        )
    };

    let init_assignments = ast
        .assignments
        .iter()
        .filter_map(|a| match a {
            SsmvAssignment::Init(vid, eid) => Some((*var_map.get(vid)?, translate(*eid))),
            _ => None,
        })
        .collect();

    let next_assignments = ast
        .assignments
        .iter()
        .filter_map(|a| match a {
            SsmvAssignment::Next(vid, eid) => Some((*var_map.get(vid)?, translate(*eid))),
            _ => None,
        })
        .collect();

    let mut sym_ctl_arena = CtlFormulaArena::new();
    let mut memo = HashMap::new();
    let mut sym_specs = Vec::new();

    for &ast_spec_id in &ast.specifications {
        let sym_spec_id = translate_and_purify_ctl(
            ast_spec_id,
            &ast.ctl_arena,
            &mut sym_ctl_arena,
            &mut memo,
            &ast.arena,
            &mut sym_arena,
            &var_map,
            &def_map,
            &enum_map,
        );
        sym_specs.push(sym_spec_id);
    }

    Model {
        variables,
        init_assignments,
        next_assignments,
        specs: sym_specs,
        arena: sym_arena,
        ast_names: ast.arena,
        ctl_arena: sym_ctl_arena,
    }
}

fn build_indices(
    ast: &SsmvModel,
) -> (
    HashMap<IdentifierID, usize>,
    HashMap<IdentifierID, ExprID>,
    HashMap<IdentifierID, (usize, usize)>,
) {
    let mut var_map = HashMap::new();
    let mut def_map = HashMap::new();
    let mut enum_map = HashMap::new();

    for (idx, var) in ast.variables.iter().enumerate() {
        var_map.insert(var.name, idx);
        if let SsmvType::Enum(ids) = &var.data_type {
            for (v_idx, &val_id) in ids.iter().enumerate() {
                if let Some(&(_existing_var_idx, existing_v_idx)) = enum_map.get(&val_id) {
                    if existing_v_idx != v_idx {
                        panic!("Enum value {:?} is used with conflicting indices!", val_id);
                    }
                } else {
                    enum_map.insert(val_id, (idx, v_idx));
                }
            }
        }
    }

    for def in &ast.definitions {
        def_map.insert(def.name, def.expression);
    }

    (var_map, def_map, enum_map)
}

fn translate_expr(
    ast_eid: ExprID,
    ast_arena: &SsmvArena,
    sym_arena: &mut SymbolicArena,
    var_map: &HashMap<IdentifierID, usize>,
    def_map: &HashMap<IdentifierID, ExprID>,
    enum_map: &HashMap<IdentifierID, (usize, usize)>,
    stack: &mut Vec<IdentifierID>,
) -> SymbolicExprID {
    let expr = match &ast_arena.expressions[ast_eid.0 as usize] {
        SsmvExpr::Number(n) => SymbolicExpr::Literal(Value::Int(*n)),
        SsmvExpr::Bool(b) => SymbolicExpr::Literal(Value::Bool(*b)),

        SsmvExpr::Identifier(id) => {
            if let Some(&def_eid) = def_map.get(id) {
                if stack.contains(id) {
                    panic!("Circular define detected!");
                }
                stack.push(*id);
                let res_id = translate_expr(
                    def_eid, ast_arena, sym_arena, var_map, def_map, enum_map, stack,
                );
                stack.pop();
                return res_id;
            }

            if let Some(&idx) = var_map.get(id) {
                SymbolicExpr::Reference(idx)
            } else if let Some(&(_, v_idx)) = enum_map.get(id) {
                SymbolicExpr::Literal(Value::Enum(v_idx))
            } else {
                panic!("Unknown ID: {:?}", id);
            }
        }

        SsmvExpr::Binary(l, op, r) => {
            let sl = translate_expr(*l, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
            let sr = translate_expr(*r, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
            SymbolicExpr::Binary(map_bin_op(ast_arena.get_ident(*op)), sl, sr)
        }

        SsmvExpr::Unary(op, sub) => {
            let s_sub = translate_expr(
                *sub, ast_arena, sym_arena, var_map, def_map, enum_map, stack,
            );
            SymbolicExpr::Unary(map_un_op(ast_arena.get_ident(*op)), s_sub)
        }

        SsmvExpr::Case(start, len) => {
            let mut tmp = Vec::with_capacity(*len as usize);
            for i in 0..(*len as usize) {
                let (c, t) = ast_arena.case_arms[*start as usize + i];
                let sc = translate_expr(c, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
                let st = translate_expr(t, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
                tmp.push((sc, st));
            }
            let sym_start = sym_arena.case_buffer.len() as u32;
            sym_arena.case_buffer.extend(tmp);
            SymbolicExpr::Case {
                start: sym_start,
                len: *len,
            }
        }

        SsmvExpr::Set(start, len) => {
            let mut tmp = Vec::with_capacity(*len as usize);
            for i in 0..(*len as usize) {
                let e = ast_arena.set_elements[*start as usize + i];
                let se = translate_expr(e, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
                tmp.push(se);
            }
            let sym_start = sym_arena.set_buffer.len() as u32;
            sym_arena.set_buffer.extend(tmp);
            SymbolicExpr::Set {
                start: sym_start,
                len: *len,
            }
        }
    };

    sym_arena.alloc_expr(expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::ssmv_ast::{SsmvDefine, SsmvVariable};

    #[test]
    fn test_enum_translation() {
        let mut arena = SsmvArena::new();
        let light_id = arena.intern_identifier("light");
        let red_id = arena.intern_identifier("red");
        let green_id = arena.intern_identifier("green");

        let var_light = SsmvVariable {
            name: light_id,
            data_type: SsmvType::Enum(vec![red_id, green_id]),
        };

        let red_expr = arena.insert_expr(SsmvExpr::Identifier(red_id));

        let ast = SsmvModel {
            name: "Traffic".into(),
            variables: vec![var_light],
            definitions: vec![],
            assignments: vec![SsmvAssignment::Init(light_id, red_expr)],
            specifications: vec![],
            arena,
            ctl_arena: CtlFormulaArena::new(),
        };

        let model = build_model(ast);

        let (_, expr_id) = model.init_assignments[0];

        let expr = &model.arena.expressions[expr_id.0 as usize];

        if let SymbolicExpr::Literal(Value::Enum(v_idx)) = expr {
            assert_eq!(*v_idx, 0);
        } else {
            panic!("Enum literal translation failed. Found: {:?}", expr);
        }
    }

    #[test]
    #[should_panic(expected = "Circular define detected!")]
    fn test_circular_define() {
        let mut arena = SsmvArena::new();
        let a_id = arena.intern_identifier("A");
        let b_id = arena.intern_identifier("B");
        let x_id = arena.intern_identifier("x");

        let def_a = SsmvDefine {
            name: a_id,
            expression: arena.insert_expr(SsmvExpr::Identifier(b_id)),
        };
        let def_b = SsmvDefine {
            name: b_id,
            expression: arena.insert_expr(SsmvExpr::Identifier(a_id)),
        };

        let ast = SsmvModel {
            name: "Loop".into(),
            variables: vec![SsmvVariable {
                name: x_id,
                data_type: SsmvType::Boolean,
            }],
            definitions: vec![def_a, def_b],
            assignments: vec![SsmvAssignment::Init(
                x_id,
                arena.insert_expr(SsmvExpr::Identifier(a_id)),
            )],
            specifications: vec![],
            arena,
            ctl_arena: CtlFormulaArena::new(),
        };

        build_model(ast);
    }
}
