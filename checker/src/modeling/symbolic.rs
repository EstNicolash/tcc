use crate::modeling::ssmv_ast::{
    ExprID, IdentifierID, SsmvArena, SsmvAssignment, SsmvExpr, SsmvModel, SsmvType,
};
use crate::specs::ctl_formula::{CtlFormulaArena, FormulaID};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolicExprID(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Enum(usize),
}

#[derive(Debug, Clone)]
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
}

pub struct Model {
    pub variables: Vec<Variable>,
    pub init_assignments: Vec<(usize, SymbolicExprID)>,
    pub next_assignments: Vec<(usize, SymbolicExprID)>,
    pub specs: Vec<FormulaID>,
    pub arena: SymbolicArena,
    pub ast_names: SsmvArena,
    pub ctl_arena: CtlFormulaArena,
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
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

pub fn build_model(ast: SsmvModel) -> Model {
    let mut sym_arena = SymbolicArena {
        expressions: Vec::new(),
        case_buffer: Vec::new(),
        set_buffer: Vec::new(),
    };

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

    Model {
        variables,
        init_assignments,
        next_assignments,
        specs: ast.specifications,
        arena: sym_arena,
        ast_names: ast.arena,
        ctl_arena: ast.ctl_arena,
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
                if enum_map.contains_key(&val_id) {
                    panic!("Duplicate enum value ID: {:?}", val_id);
                }
                enum_map.insert(val_id, (idx, v_idx));
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
            let sym_start = sym_arena.case_buffer.len() as u32;
            for i in (*start as usize)..(*start as usize + *len as usize) {
                let (c, t) = ast_arena.case_arms[i];
                let sc = translate_expr(c, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
                let st = translate_expr(t, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
                sym_arena.case_buffer.push((sc, st));
            }
            SymbolicExpr::Case {
                start: sym_start,
                len: *len,
            }
        }

        SsmvExpr::Set(start, len) => {
            let sym_start = sym_arena.set_buffer.len() as u32;
            for i in (*start as usize)..(*start as usize + *len as usize) {
                let e = ast_arena.set_elements[i];
                let se = translate_expr(e, ast_arena, sym_arena, var_map, def_map, enum_map, stack);
                sym_arena.set_buffer.push(se);
            }
            SymbolicExpr::Set {
                start: sym_start,
                len: *len,
            }
        }
    };

    let id = SymbolicExprID(sym_arena.expressions.len() as u32);
    sym_arena.expressions.push(expr);
    id
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
