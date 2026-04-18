use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::modeling::ssmv_ast::{
    ExprID, IdentifierID, SsmvArena, SsmvAssignment, SsmvDefine, SsmvExpr, SsmvModel, SsmvType,
    SsmvVariable,
};
use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};

#[derive(Parser)]
#[grammar = "modeling/ssmv.pest"]
pub struct SsmvParser;

/// Convert an SSMV model string into an AST representation.
/// # Arguments:
/// - `input`: the SSMV model string to parse
///
/// # Returns:
/// - `Ok(model)`: the parsed SSMV model
/// - `Err(msg)`: a parse error message
pub fn parse_ssmv(input: &str) -> Result<SsmvModel, String> {
    let mut pairs =
        SsmvParser::parse(Rule::ssmv_main, input).map_err(|e| format!("Parse error: {}", e))?;

    let module_main = pairs
        .next() //Gives the first pair in the pairs iterator: Some(ssmv_main) or None
        .unwrap() //Catch the first pair (ssmv_main), panicking if the iterator is empty
        .into_inner() //Extract the inner pairs of the first pair, example: ssmv_main to [SOI, ssvm_module_main, EOI]
        .find(|p| p.as_rule() == Rule::ssmv_module_main) //Find the ssmv_module_main pair, example: [SOI, here> ssvm_module_main <, EOI]
        .expect("Grammar error: ssmv_module_main not found"); //Panic if not found

    Ok(parse_module(module_main))
}

/// Parses an SSMV module and returns an `SsmvModel`.
/// # Arguments
/// * `pair` - The `ssmv_module_main` pair to parse
/// # Returns
/// An `SsmvModel` containing the parsed module.
fn parse_module(pair: Pair<Rule>) -> SsmvModel {
    let mut variables = vec![];
    let mut definitions = vec![];
    let mut assignments = vec![];
    let mut specifications = vec![];

    let mut ssmv_arena = SsmvArena::new();
    let mut ctl_arena = CtlFormulaArena::new();

    // Iterate over the sections of the ssmv_module_main block and populate the model.
    for section in pair.into_inner() {
        match section.as_rule() {
            Rule::ssmv_var_section => {
                for inner in section.into_inner() {
                    if inner.as_rule() == Rule::ssmv_var_decl {
                        variables.push(parse_var_decl(inner, &mut ssmv_arena));
                    }
                }
            }
            Rule::ssmv_define_section => {
                for inner in section.into_inner() {
                    if inner.as_rule() == Rule::ssmv_define_decl {
                        definitions.push(parse_define_decl(inner, &mut ssmv_arena));
                    }
                }
            }
            Rule::ssmv_assign_section => {
                for inner in section.into_inner() {
                    if inner.as_rule() == Rule::ssmv_assignment {
                        assignments.push(parse_assignment(inner, &mut ssmv_arena));
                    }
                }
            }
            Rule::ssmv_spec_section => {
                let formula_pair = section
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::formula)
                    .expect("Grammar error: formula not found inside CTLSPEC");
                specifications.push(parse_ctl(formula_pair, &mut ctl_arena, &mut ssmv_arena));
            }
            _ => {}
        }
    }

    SsmvModel {
        name: "main".to_string(),
        variables,
        definitions,
        assignments,
        specifications,
        arena: ssmv_arena,
        ctl_arena: ctl_arena,
    }
}
/// Parses an SSMV variable declaration and returns an `SsmvVariable`.
///
/// A variable declaration is something like `x: int;`, `locked: boolean;`, `hp: 0..3;` and `inventory: {key, treasure};`.
///
/// # Arguments
/// * `pair` - The `ssmv_var_decl` pair to parse
/// # Returns
/// An `SsmvVariable` containing the parsed variable declaration.
fn parse_var_decl(pair: Pair<Rule>, ssmv_arena: &mut SsmvArena) -> SsmvVariable {
    let mut inner = pair.into_inner(); // ssmv_var_decl to [ssmv_ident, ssmv_var_type]
    let name_str = inner.next().unwrap().as_str();
    let name: IdentifierID = ssmv_arena.intern_identifier(name_str);
    let data_type = parse_var_type(inner.next().unwrap(), ssmv_arena);
    SsmvVariable { name, data_type }
}

/// Parses an SSMV variable type and returns an `SsmvType`.
/// # Arguments
/// * `pair` - The `ssmv_var_type` pair to parse
/// # Returns
/// An `SsmvType` containing the parsed variable type.
fn parse_var_type(pair: Pair<Rule>, ssmv_arena: &mut SsmvArena) -> SsmvType {
    // boolean is a literal — no child. ssmv_enum_type and ssmv_range_type generate.
    match pair.into_inner().next() {
        None => SsmvType::Boolean,
        Some(inner) => match inner.as_rule() {
            Rule::ssmv_enum_type => {
                let values = inner
                    .into_inner()
                    .map(|p| ssmv_arena.intern_identifier(p.as_str()))
                    .collect(); // Create a vector of the defined enum values as strings
                SsmvType::Enum(values)
            }

            Rule::ssmv_range_type => {
                let mut nums = inner.into_inner();
                let lo = nums.next().unwrap().as_str().parse::<i32>().unwrap();
                let hi = nums.next().unwrap().as_str().parse::<i32>().unwrap();
                SsmvType::Range(lo, hi)
            }
            _ => unreachable!("Unexpected var type: {:?}", inner.as_rule()),
        },
    }
}

fn parse_define_decl(pair: Pair<Rule>, ssmv_arena: &mut SsmvArena) -> SsmvDefine {
    let mut inner = pair.into_inner();
    let name = ssmv_arena.intern_identifier(inner.next().unwrap().as_str());
    let expression = parse_expr(inner.next().unwrap(), ssmv_arena);
    SsmvDefine { name, expression }
}

/// Parses an SSMV assignment and returns an `SsmvAssignment`.
///
/// An assignment can be either an `Init` or a `Next` assignment like `init(gamestate) := running;` or `next(gamestate) := running;`.
///
/// # Arguments
/// * `pair` - The `ssmv_assign` pair to parse
/// * `ssmv_arena` - The arena to store the parsed expression in
///
/// # Returns
/// An `SsmvAssignment` containing the parsed assignment.
fn parse_assignment(pair: Pair<Rule>, ssmv_arena: &mut SsmvArena) -> SsmvAssignment {
    let inner = pair.into_inner().next().unwrap();

    match inner.as_rule() {
        Rule::ssmv_init_assign => {
            let mut sub = inner.into_inner();
            let name = ssmv_arena.intern_identifier(sub.next().unwrap().as_str());
            let expr = parse_expr(sub.next().unwrap(), ssmv_arena);
            SsmvAssignment::Init(name, expr)
        }
        Rule::ssmv_next_assign => {
            let mut sub = inner.into_inner();
            let name = ssmv_arena.intern_identifier(sub.next().unwrap().as_str());
            let expr = parse_expr(sub.next().unwrap(), ssmv_arena);
            SsmvAssignment::Next(name, expr)
        }
        _ => unreachable!("Unexpected assignment rule: {:?}", inner.as_rule()),
    }
}
/// Parses an SSMV expression and returns an `SsmvExpr`.
///
/// # Arguments
/// * `pair` - The `ssmv_expression` or other expression pair to parse
/// * `ssmv_arena` - The arena to store the parsed expression in
/// # Returns
/// An `SsmvExpr` containing the parsed expression.
fn parse_expr(pair: Pair<Rule>, ssmv_arena: &mut SsmvArena) -> ExprID {
    match pair.as_rule() {
        Rule::ssmv_expression => parse_expr(pair.into_inner().next().unwrap(), ssmv_arena),
        Rule::ssmv_implication => {
            let mut inner = pair.into_inner();
            let mut left = parse_expr(inner.next().unwrap(), ssmv_arena);
            let op_id = ssmv_arena.intern_identifier("->");
            for right_pair in inner {
                let right = parse_expr(right_pair, ssmv_arena);
                left = ssmv_arena.insert_expr(SsmvExpr::Binary(left, op_id, right));
            }
            left
        }
        Rule::ssmv_logical_or
        | Rule::ssmv_logical_and
        | Rule::ssmv_comparison
        | Rule::ssmv_term
        | Rule::ssmv_factor => {
            let mut inner = pair.into_inner();
            let mut left = parse_expr(inner.next().unwrap(), ssmv_arena);
            while let Some(op_pair) = inner.next() {
                let op_id = ssmv_arena.intern_identifier(op_pair.as_str());
                let right = parse_expr(inner.next().unwrap(), ssmv_arena);
                left = ssmv_arena.insert_expr(SsmvExpr::Binary(left, op_id, right));
            }
            left
        }
        Rule::ssmv_unary => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();
            match first.as_rule() {
                Rule::ssmv_unary_op => {
                    let op_id = ssmv_arena.intern_identifier(first.as_str());
                    let operand = parse_expr(inner.next().unwrap(), ssmv_arena);
                    ssmv_arena.insert_expr(SsmvExpr::Unary(op_id, operand))
                }
                _ => parse_primary(first, ssmv_arena),
            }
        }
        Rule::ssmv_primary => parse_primary(pair, ssmv_arena),
        _ => unreachable!(),
    }
}

/// For primary expressions, parses the inner expression and returns an `SsmvExpr`.
///
/// # Arguments
/// * `pair` - The `ssmv_primary` or other primary expression pair to parse
/// * `ssmv_arena` - The arena to store the parsed expression in
///
/// A primary expression is either a number, boolean, identifier, set constant, a case expression, or a parenthesized expression.
fn parse_primary(pair: Pair<Rule>, arena: &mut SsmvArena) -> ExprID {
    let inner = match pair.as_rule() {
        Rule::ssmv_primary => pair.into_inner().next().unwrap(),
        _ => pair,
    };

    match inner.as_rule() {
        Rule::ssmv_number => arena.insert_expr(SsmvExpr::Number(inner.as_str().parse().unwrap())),
        Rule::ssmv_boolean_const => arena.insert_expr(SsmvExpr::Bool(inner.as_str() == "TRUE")),
        Rule::ssmv_ident => {
            let id = arena.intern_identifier(inner.as_str());
            arena.insert_expr(SsmvExpr::Identifier(id))
        }
        Rule::ssmv_set_const => {
            let elements: Vec<ExprID> = inner.into_inner().map(|p| parse_expr(p, arena)).collect();
            arena.alloc_set(elements)
        }
        Rule::ssmv_case_expression => {
            let arms: Vec<(ExprID, ExprID)> = inner
                .into_inner()
                .filter(|p| p.as_rule() == Rule::ssmv_case_arm)
                .map(|arm| {
                    let mut sub = arm.into_inner();
                    let cond = parse_expr(sub.next().unwrap(), arena);
                    let val = parse_expr(sub.next().unwrap(), arena);
                    (cond, val)
                })
                .collect();
            arena.alloc_case(arms)
        }
        Rule::ssmv_expression => parse_expr(inner, arena),
        _ => unreachable!(),
    }
}

/// For CTL expressions, parses the operator and operands and returns a `CtlFormula`.
fn parse_ctl(
    pair: Pair<Rule>,
    formula_arena: &mut CtlFormulaArena<ExprID>,
    ssmv_arena: &mut SsmvArena,
) -> FormulaID {
    match pair.as_rule() {
        Rule::formula | Rule::implication | Rule::iff | Rule::or | Rule::and => {
            let mut inner = pair.into_inner();
            let mut left = parse_ctl(inner.next().unwrap(), formula_arena, ssmv_arena);

            while let Some(op) = inner.next() {
                let right = parse_ctl(inner.next().unwrap(), formula_arena, ssmv_arena);
                left = match op.as_rule() {
                    Rule::op_imply => formula_arena.insert(CtlFormula::Imply(left, right)),
                    Rule::op_iff => formula_arena.insert(CtlFormula::Iff(left, right)),
                    Rule::op_or => formula_arena.insert(CtlFormula::Or(left, right)),
                    Rule::op_and => formula_arena.insert(CtlFormula::And(left, right)),
                    _ => unreachable!("Unexpected operator rule: {:?}", op.as_rule()),
                };
            }
            left
        }

        Rule::temporal_binary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::eu => {
                    let mut sub = inner.into_inner();
                    let f1 = parse_ctl(sub.next().unwrap(), formula_arena, ssmv_arena);
                    let f2 = parse_ctl(sub.next().unwrap(), formula_arena, ssmv_arena);
                    formula_arena.insert(CtlFormula::EU(f1, f2))
                }
                Rule::au => {
                    let mut sub = inner.into_inner();
                    let f1 = parse_ctl(sub.next().unwrap(), formula_arena, ssmv_arena);
                    let f2 = parse_ctl(sub.next().unwrap(), formula_arena, ssmv_arena);
                    formula_arena.insert(CtlFormula::AU(f1, f2))
                }
                _ => parse_ctl(inner, formula_arena, ssmv_arena),
            }
        }

        Rule::unary => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();

            match first.as_rule() {
                Rule::unary_op => {
                    let op_str = first.as_str();
                    let sub = parse_ctl(inner.next().unwrap(), formula_arena, ssmv_arena);
                    match op_str {
                        "EX" => formula_arena.insert(CtlFormula::EX(sub)),
                        "AX" => formula_arena.insert(CtlFormula::AX(sub)),
                        "EF" => formula_arena.insert(CtlFormula::EF(sub)),
                        "AF" => formula_arena.insert(CtlFormula::AF(sub)),
                        "EG" => formula_arena.insert(CtlFormula::EG(sub)),
                        "AG" => formula_arena.insert(CtlFormula::AG(sub)),
                        _ => unreachable!("Unknown CTL op: {}", op_str),
                    }
                }
                _ => {
                    if first.as_rule() == Rule::primary {
                        parse_ctl(first, formula_arena, ssmv_arena)
                    } else {
                        let sub = parse_ctl(first, formula_arena, ssmv_arena);
                        formula_arena.insert(CtlFormula::Not(sub))
                    }
                }
            }
        }

        Rule::primary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::formula => parse_ctl(inner, formula_arena, ssmv_arena),
                Rule::constant => match inner.as_str().to_lowercase().as_str() {
                    "true" => formula_arena.insert(CtlFormula::True),
                    "false" => formula_arena.insert(CtlFormula::False),
                    _ => unreachable!(),
                },
                Rule::proposition => {
                    let expr_pair = inner.into_inner().next().unwrap();

                    let expr_id = parse_expr(expr_pair, ssmv_arena);

                    formula_arena.insert(CtlFormula::Prop(expr_id))
                }
                _ => unreachable!("Unexpected primary rule: {:?}", inner.as_rule()),
            }
        }

        _ => unreachable!("Unexpected rule: {:?}", pair.as_rule()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::ctl_formula::CtlFormula;

    #[test]
    fn test_boolean_variable() {
        let model = parse_ssmv("MODULE main VAR flag : boolean;").unwrap();
        assert_eq!(model.variables.len(), 1);

        assert_eq!(model.arena.get_ident(model.variables[0].name), "flag");
        assert!(matches!(model.variables[0].data_type, SsmvType::Boolean));
    }

    #[test]
    fn test_enum_variable() {
        let model = parse_ssmv("MODULE main VAR state : {s0, s1, s2};").unwrap();
        assert_eq!(model.variables.len(), 1);

        if let SsmvType::Enum(vals) = &model.variables[0].data_type {
            let names: Vec<&str> = vals.iter().map(|&id| model.arena.get_ident(id)).collect();
            assert_eq!(names, vec!["s0", "s1", "s2"]);
        } else {
            panic!("Expected Enum type");
        }
    }

    #[test]
    fn test_range_variable() {
        let model = parse_ssmv("MODULE main VAR counter : 0..9;").unwrap();
        assert_eq!(model.arena.get_ident(model.variables[0].name), "counter");
        assert!(matches!(
            model.variables[0].data_type,
            SsmvType::Range(0, 9)
        ));
    }

    #[test]
    fn test_init_assignment() {
        let src = "MODULE main VAR s : {a, b}; ASSIGN init(s) := a;";
        let model = parse_ssmv(src).unwrap();
        assert_eq!(model.assignments.len(), 1);

        match &model.assignments[0] {
            SsmvAssignment::Init(name_id, _) => {
                assert_eq!(model.arena.get_ident(*name_id), "s");
            }
            _ => panic!("Expected Init assignment"),
        }
    }

    #[test]
    fn test_next_assignment_case() {
        let src = r#"
            MODULE main
            VAR s : {s0, s1};
            ASSIGN
                next(s) := case
                    s = s0 : s1;
                    s = s1 : s0;
                esac;
        "#;
        let model = parse_ssmv(src).unwrap();

        if let SsmvAssignment::Next(_, expr_id) = &model.assignments[0] {
            if let SsmvExpr::Case(_start, len) = model.arena.expressions[expr_id.0 as usize] {
                assert_eq!(len, 2);
            } else {
                panic!("Expected Case expression");
            }
        }
    }

    #[test]
    fn test_next_assignment_set() {
        let src = "MODULE main VAR s : {a, b}; ASSIGN next(s) := {a, b};";
        let model = parse_ssmv(src).unwrap();

        if let SsmvAssignment::Next(_, expr_id) = &model.assignments[0] {
            if let SsmvExpr::Set(_start, len) = model.arena.expressions[expr_id.0 as usize] {
                assert_eq!(len, 2);
            } else {
                panic!("Expected Set expression");
            }
        }
    }

    #[test]
    fn test_define_section() {
        let src = "MODULE main VAR x : boolean; DEFINE is_active := x & TRUE;";
        let model = parse_ssmv(src).unwrap();

        let def = &model.definitions[0];
        assert_eq!(model.arena.get_ident(def.name), "is_active");

        if let SsmvExpr::Binary(_lhs, op_id, _rhs) =
            model.arena.expressions[def.expression.0 as usize]
        {
            assert_eq!(model.arena.get_ident(op_id), "&");
        } else {
            panic!("Expected Binary expression");
        }
    }

    #[test]
    fn test_ctlspec_ag() {
        let src = "MODULE main VAR s : boolean; CTLSPEC AG s;";
        let model = parse_ssmv(src).unwrap();

        let formula_id = model.specifications[0];
        assert!(matches!(model.ctl_arena.get(formula_id), CtlFormula::AG(_)));
    }

    #[test]
    fn test_multiple_specs() {
        let src = r#"
            MODULE main
            VAR state : {idle, running, done};
            CTLSPEC AG(state = idle -> EF state = done)
            CTLSPEC EG !(state = running)
        "#;
        let model = parse_ssmv(src).unwrap();
        assert_eq!(model.specifications.len(), 2);
    }

    #[test]
    fn test_complete_model() {
        let src = r#"
            MODULE main
            VAR
                state : {green, yellow, red};
                timer : 0..9;
            ASSIGN
                init(state) := green;
                init(timer) := 0;
                next(state) := case
                    state = green  : yellow;
                    state = yellow : red;
                    state = red    : green;
                esac;
            CTLSPEC AG(state = red -> AF state = green)
        "#;
        let model = parse_ssmv(src).unwrap();
        assert_eq!(model.variables.len(), 2);
        assert_eq!(model.assignments.len(), 3);
        assert_eq!(model.specifications.len(), 1);
    }
}
