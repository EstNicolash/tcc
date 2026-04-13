use crate::modeling::ast::{SsmvAssignment, SsmvExpr, SsmvModel, SsmvType, SsmvVariable};
use crate::specs::ctl_formula::CtlFormula;
use std::collections::{HashMap, HashSet};
use std::panic;

pub enum Domain {
    Boolean,
    Range { min: i32, max: i32 },
    Enum(Vec<String>),
}

impl Domain {
    pub fn size(&self) -> usize {
        match self {
            Domain::Boolean => 2,
            Domain::Range { min, max } => (max - min + 1) as usize,
            Domain::Enum(values) => values.len(),
        }
    }

    pub fn values(&self) -> Vec<Value> {
        match self {
            Domain::Boolean => vec![Value::Bool(false), Value::Bool(true)],
            Domain::Range { min, max } => (*min..=*max).map(Value::Int).collect(),
            Domain::Enum(vals) => (0..vals.len()).map(Value::Enum).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Enum(usize), // índice em Domain::Enum
}

pub struct Variable {
    pub name: String,
    pub domain: Domain,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    Reference(usize), // índex into Model::variables
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Case(Vec<(Expr, Expr)>),
    Set(Vec<Expr>),
}

pub struct Model {
    pub variables: Vec<Variable>,
    pub init_assignments: Vec<(usize, Expr)>, // (var_idx, expr)
    pub next_assignments: Vec<(usize, Expr)>, // (var_idx, expr)
    pub specs: Vec<CtlFormula>,
}

/// Build a Model from an SsmvModel AST.
///
/// # Arguments
///
/// * `ast` - The SsmvModel AST to build from.
///
/// # Returns
///
/// A `Model` struct representing the symbolic model.
///
/// # Panics
///
/// Panics if the AST contains invalid expressions or variable references.
pub fn build_model(ast: SsmvModel) -> Model {
    let (var_index_map, define_map, enum_value_map) = build_indices(&ast);

    let variables: Vec<Variable> = ast
        .variables
        .iter()
        .map(|var| translate_variable(&var))
        .collect::<Vec<_>>();

    let mut init_assignments = Vec::<(usize, Expr)>::new();
    let mut next_assignments = Vec::<(usize, Expr)>::new();

    for assignment in &ast.assignments {
        match assignment {
            SsmvAssignment::Init(var_name, expr) => {
                let var_idx = var_index_map[var_name];
                let expr = translate_expressions(
                    expr,
                    &var_index_map,
                    &define_map,
                    &enum_value_map,
                    &mut HashSet::new(),
                );
                if let Some(expr) = expr {
                    init_assignments.push((var_idx, expr));
                } else {
                    panic!(
                        "Failed to translate init assignment for variable: {}",
                        var_name
                    );
                }
            }
            SsmvAssignment::Next(var_name, expr) => {
                let var_idx = var_index_map[var_name];
                let expr = translate_expressions(
                    expr,
                    &var_index_map,
                    &define_map,
                    &enum_value_map,
                    &mut HashSet::new(),
                );

                if let Some(expr) = expr {
                    next_assignments.push((var_idx, expr));
                } else {
                    panic!(
                        "Failed to translate next assignment for variable: {}",
                        var_name
                    );
                }
            }
        }
    }

    // Post build validation
    let vars_com_init = init_assignments
        .iter()
        .map(|(idx, _)| idx)
        .collect::<HashSet<_>>();
    let vars_com_next = next_assignments
        .iter()
        .map(|(idx, _)| idx)
        .collect::<HashSet<_>>();

    for (idx, var) in variables.iter().enumerate() {
        if !vars_com_init.contains(&idx) {
            eprintln!(
                "Warning: variable '{}' has no init — will be non-deterministic",
                var.name
            );
        }
        if !vars_com_next.contains(&idx) {
            eprintln!(
                "Warning: variable '{}' has no next — will keep current value",
                var.name
            );
        }
    }

    Model {
        variables,
        init_assignments,
        next_assignments,
        specs: ast.specifications,
    }
}

/// Builds indices for variables, defines, and enum values in the AST.
///
/// # Arguments
///
/// * `ast` - The AST model to build indices for.
///
/// # Returns
///
/// * `var_index_map` - A map of variable names to their index in the `variables` vector. E.g. "state" → 0, "timer" → 1
/// * `define_map` - A map of define names to their expression. E.g. "is_active" → SsmvExpr::Binary(...)
/// * `enum_value_map` - A map of enum value names to their (enum_idx, value_idx) pair. E.g. "s0" → (var_idx=0, val_idx=0)
///
/// # Panics
///
/// * If a duplicate enum value is found.
fn build_indices(
    ast: &SsmvModel,
) -> (
    HashMap<String, usize>,
    HashMap<String, SsmvExpr>,
    HashMap<String, (usize, usize)>,
) {
    let mut var_index_map = HashMap::<String, usize>::new();
    let mut define_map = HashMap::<String, SsmvExpr>::new();
    let mut enum_value_map = HashMap::<String, (usize, usize)>::new();

    // Index variables declared in order
    for (idx, var) in ast.variables.iter().enumerate() {
        var_index_map.insert(var.name.clone(), idx);

        // Handle enum values
        if let SsmvType::Enum(vals) = &var.data_type {
            for (val_idx, val) in vals.iter().enumerate() {
                if enum_value_map.contains_key(val) {
                    panic!("Duplicate enum value: {}", val)
                }

                enum_value_map.insert(val.clone(), (idx, val_idx));
            }
        }
    }

    // Index defines
    for define in &ast.definitions {
        if define_map.contains_key(&define.name) {
            panic!("Duplicate define: {}", define.name)
        }
        define_map.insert(define.name.clone(), define.expression.clone());
    }

    return (var_index_map, define_map, enum_value_map);
}

/// Just translates a single variable from the SsmvVariable AST representation to the Variable struct.
///
/// # Arguments
///
/// * `var` - The variable to translate.
///
/// # Returns
///
/// * The translated variable.
fn translate_variable(var: &SsmvVariable) -> Variable {
    let domain = match &var.data_type {
        SsmvType::Enum(vals) => Domain::Enum(vals.clone()),
        SsmvType::Boolean => Domain::Boolean,
        SsmvType::Range(min, max) => Domain::Range {
            min: *min,
            max: *max,
        },
    };
    Variable {
        name: var.name.clone(),
        domain,
    }
}

/// Translates an SsmvExpr AST expression to an Expr struct.
///
/// # Arguments
///
/// * `expr` - The expression to translate.
/// * `var_index_map` - A map of variable names to their indices in the state vector.
/// * `define_map` - A map of define names to their expressions.
/// * `enum_value_map` - A map of enum names to their value ranges.
/// * `define_stack` - A set of define names that are currently being translated to detect circular references.
///
/// # Returns
///
/// * The translated expression, or `None` if the expression is invalid.
fn translate_expressions(
    expr: &SsmvExpr,
    var_index_map: &HashMap<String, usize>,
    define_map: &HashMap<String, SsmvExpr>,
    enum_value_map: &HashMap<String, (usize, usize)>,
    define_stack: &mut HashSet<String>,
) -> Option<Expr> {
    match expr {
        SsmvExpr::Number(n) => Some(Expr::Literal(Value::Int(*n))),
        SsmvExpr::Bool(b) => Some(Expr::Literal(Value::Bool(*b))),
        SsmvExpr::Identifier(name) => {
            if define_map.contains_key(name) {
                if define_stack.contains(name) {
                    panic!("Circular define: {}", name)
                }
                define_stack.insert(name.clone());
                let value = translate_expressions(
                    &define_map[name],
                    var_index_map,
                    define_map,
                    enum_value_map,
                    define_stack,
                );
                define_stack.remove(name);
                value
            } else if var_index_map.contains_key(name) {
                Some(Expr::Reference(var_index_map[name]))
            } else if enum_value_map.contains_key(name) {
                let (_, val_idx) = enum_value_map[name];
                Some(Expr::Literal(Value::Enum(val_idx)))
            } else {
                panic!("Undefined variable: {}", name)
            }
        }

        SsmvExpr::Unary(op_str, sub_expr) => {
            let op = match op_str.as_str() {
                "!" | "not" => UnaryOp::Not,
                "-" | "neg" => UnaryOp::Neg,
                _ => panic!("Unknown unary operator: {}", op_str),
            };
            let new_sub_expr = translate_expressions(
                sub_expr,
                var_index_map,
                define_map,
                enum_value_map,
                define_stack,
            )?;

            Some(Expr::Unary(op, Box::new(new_sub_expr)))
        }

        SsmvExpr::Binary(lhs, op_str, rhs) => {
            let op = match op_str.as_str() {
                "+" => BinaryOp::Add,
                "-" => BinaryOp::Sub,
                "*" => BinaryOp::Mul,
                "/" => BinaryOp::Div,
                "&" | "and" => BinaryOp::And,
                "|" | "or" => BinaryOp::Or,
                "->" | "imply" => BinaryOp::Imply,
                "=" => BinaryOp::Eq,
                "!=" => BinaryOp::Neq,
                "<" => BinaryOp::Lt,
                "<=" => BinaryOp::Lte,
                ">" => BinaryOp::Gt,
                ">=" => BinaryOp::Gte,
                _ => panic!("Unknown binary operator: {}", op_str),
            };
            let new_lhs = translate_expressions(
                lhs,
                var_index_map,
                define_map,
                enum_value_map,
                define_stack,
            )?;
            let new_rhs = translate_expressions(
                rhs,
                var_index_map,
                define_map,
                enum_value_map,
                define_stack,
            )?;

            Some(Expr::Binary(op, Box::new(new_lhs), Box::new(new_rhs)))
        }

        SsmvExpr::Case(branches) => {
            let new_branches = branches
                .into_iter()
                .map(|(cond, then_expr)| {
                    let new_cond = translate_expressions(
                        cond,
                        var_index_map,
                        define_map,
                        enum_value_map,
                        define_stack,
                    )?;
                    let new_then_expr = translate_expressions(
                        then_expr,
                        var_index_map,
                        define_map,
                        enum_value_map,
                        define_stack,
                    )?;
                    Some((new_cond, new_then_expr))
                })
                .collect::<Option<Vec<_>>>()?;

            Some(Expr::Case(new_branches))
        }

        SsmvExpr::Set(exprs) => {
            let new_exprs = exprs
                .into_iter()
                .map(|expr| {
                    translate_expressions(
                        expr,
                        var_index_map,
                        define_map,
                        enum_value_map,
                        define_stack,
                    )
                })
                .collect::<Option<Vec<_>>>()?;

            Some(Expr::Set(new_exprs))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::ast::{SsmvAssignment, SsmvDefine, SsmvExpr, SsmvType, SsmvVariable};

    fn mock_var(name: &str, data_type: SsmvType) -> SsmvVariable {
        SsmvVariable {
            name: name.to_string(),
            data_type,
        }
    }

    #[test]
    fn test_simple_boolean_model() {
        let ast = SsmvModel {
            name: "Main".into(),
            variables: vec![mock_var("bit", SsmvType::Boolean)],
            definitions: vec![],
            assignments: vec![
                SsmvAssignment::Init("bit".into(), SsmvExpr::Bool(false)),
                SsmvAssignment::Next(
                    "bit".into(),
                    SsmvExpr::Unary("!".into(), Box::new(SsmvExpr::Identifier("bit".into()))),
                ),
            ],
            specifications: vec![],
        };

        let model = build_model(ast);
        assert_eq!(model.variables.len(), 1);
        assert_eq!(model.init_assignments.len(), 1);
        assert_eq!(model.next_assignments.len(), 1);

        if let Expr::Unary(_, sub) = &model.next_assignments[0].1 {
            if let Expr::Reference(idx) = **sub {
                assert_eq!(idx, 0);
            } else {
                panic!("Expected reference");
            }
        }
    }

    #[test]
    fn test_enum_indexing() {
        let ast = SsmvModel {
            name: "Traffic".into(),
            variables: vec![mock_var(
                "light",
                SsmvType::Enum(vec!["red".into(), "green".into()]),
            )],
            definitions: vec![],
            assignments: vec![
                SsmvAssignment::Init("light".into(), SsmvExpr::Identifier("red".into())),
                SsmvAssignment::Next("light".into(), SsmvExpr::Identifier("green".into())),
            ],
            specifications: vec![],
        };

        let model = build_model(ast);
        if let (idx, Expr::Literal(Value::Enum(v_idx))) = &model.init_assignments[0] {
            assert_eq!(*idx, 0);
            assert_eq!(*v_idx, 0);
        } else {
            panic!("Enum value 'red' was not indexed correctly");
        }
    }

    #[test]
    #[should_panic(expected = "Circular define: A")]
    fn test_circular_define_detection() {
        let ast = SsmvModel {
            name: "Loop".into(),
            variables: vec![mock_var("x", SsmvType::Boolean)],
            definitions: vec![
                SsmvDefine {
                    name: "A".into(),
                    expression: SsmvExpr::Identifier("B".into()),
                },
                SsmvDefine {
                    name: "B".into(),
                    expression: SsmvExpr::Identifier("A".into()),
                },
            ],
            assignments: vec![
                SsmvAssignment::Init("x".into(), SsmvExpr::Identifier("A".into())),
                SsmvAssignment::Next("x".into(), SsmvExpr::Bool(true)),
            ],
            specifications: vec![],
        };

        build_model(ast);
    }
}
