use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::modeling::ast::{
    SsmvAssignment, SsmvDefine, SsmvExpr, SsmvModel, SsmvType, SsmvVariable,
};
use crate::specs::ctl_formula::CtlFormula;

#[derive(Parser)]
#[grammar = "modeling/ssmv.pest"]
pub struct SsmvParser;

pub fn parse_ssmv(input: &str) -> Result<SsmvModel, String> {
    let mut pairs =
        SsmvParser::parse(Rule::ssmv_main, input).map_err(|e| format!("Parse error: {}", e))?;

    let module_main = pairs
        .next()
        .unwrap()
        .into_inner()
        .find(|p| p.as_rule() == Rule::ssmv_module_main)
        .expect("Grammar error: ssmv_module_main not found");

    Ok(parse_module(module_main))
}

fn parse_module(pair: Pair<Rule>) -> SsmvModel {
    let mut variables = vec![];
    let mut definitions = vec![];
    let mut assignments = vec![];
    let mut specifications = vec![];

    for section in pair.into_inner() {
        match section.as_rule() {
            Rule::ssmv_var_section => {
                for inner in section.into_inner() {
                    if inner.as_rule() == Rule::ssmv_var_decl {
                        variables.push(parse_var_decl(inner));
                    }
                }
            }
            Rule::ssmv_define_section => {
                for inner in section.into_inner() {
                    if inner.as_rule() == Rule::ssmv_define_decl {
                        definitions.push(parse_define_decl(inner));
                    }
                }
            }
            Rule::ssmv_assign_section => {
                for inner in section.into_inner() {
                    if inner.as_rule() == Rule::ssmv_assignment {
                        assignments.push(parse_assignment(inner));
                    }
                }
            }
            Rule::ssmv_spec_section => {
                let formula_pair = section
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::formula)
                    .expect("Grammar error: formula not found inside CTLSPEC");
                specifications.push(parse_ctl(formula_pair));
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
    }
}

fn parse_var_decl(pair: Pair<Rule>) -> SsmvVariable {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let data_type = parse_var_type(inner.next().unwrap());
    SsmvVariable { name, data_type }
}

fn parse_var_type(pair: Pair<Rule>) -> SsmvType {
    // boolean is a literal — no child. ssmv_enum_type and ssmv_range_type generate.
    match pair.into_inner().next() {
        None => SsmvType::Boolean,
        Some(inner) => match inner.as_rule() {
            Rule::ssmv_enum_type => {
                let values = inner.into_inner().map(|p| p.as_str().to_string()).collect();
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

fn parse_define_decl(pair: Pair<Rule>) -> SsmvDefine {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let expression = parse_expr(inner.next().unwrap());
    SsmvDefine { name, expression }
}

fn parse_assignment(pair: Pair<Rule>) -> SsmvAssignment {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::ssmv_init_assign => {
            let mut sub = inner.into_inner();
            let name = sub.next().unwrap().as_str().to_string();
            let expr = parse_expr(sub.next().unwrap());
            SsmvAssignment::Init(name, expr)
        }
        Rule::ssmv_next_assign => {
            let mut sub = inner.into_inner();
            let name = sub.next().unwrap().as_str().to_string();
            let expr = parse_expr(sub.next().unwrap());
            SsmvAssignment::Next(name, expr)
        }
        _ => unreachable!("Unexpected assignment rule: {:?}", inner.as_rule()),
    }
}

fn parse_expr(pair: Pair<Rule>) -> SsmvExpr {
    match pair.as_rule() {
        Rule::ssmv_expression => parse_expr(pair.into_inner().next().unwrap()),

        Rule::ssmv_implication => parse_left_binary(pair, "->"),
        Rule::ssmv_logical_or => parse_binary_with_op(pair),
        Rule::ssmv_logical_and => parse_binary_with_op(pair),
        Rule::ssmv_comparison => parse_binary_with_op(pair),
        Rule::ssmv_term => parse_binary_with_op(pair),
        Rule::ssmv_factor => parse_binary_with_op(pair),

        Rule::ssmv_unary => parse_unary(pair),
        Rule::ssmv_primary => parse_primary(pair),

        _ => unreachable!("Unexpected expression rule: {:?}", pair.as_rule()),
    }
}

/// Para ssmv_implication: operador é sempre "->" (literal, não aparece como Pair)
/// For ssmv_implication: operator is always "->" (literal, not a Pair)
fn parse_left_binary(pair: Pair<Rule>, op: &str) -> SsmvExpr {
    let mut inner = pair.into_inner();
    let mut left = parse_expr(inner.next().unwrap());
    for right_pair in inner {
        let right = parse_expr(right_pair);
        left = SsmvExpr::Binary(Box::new(left), op.to_string(), Box::new(right));
    }
    left
}

/// For rules where the operator is a named Pair (ssmv_or_op, ssmv_and_op, etc.)
/// inner: [expr, op, expr, op, expr, ...]
fn parse_binary_with_op(pair: Pair<Rule>) -> SsmvExpr {
    let mut inner = pair.into_inner();
    let mut left = parse_expr(inner.next().unwrap());
    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str().to_string();
        let right = parse_expr(inner.next().unwrap());
        left = SsmvExpr::Binary(Box::new(left), op, Box::new(right));
    }
    left
}

fn parse_unary(pair: Pair<Rule>) -> SsmvExpr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();

    match first.as_rule() {
        Rule::ssmv_unary_op => {
            let op = first.as_str().to_string();
            let operand = parse_expr(inner.next().unwrap());
            SsmvExpr::Unary(op, Box::new(operand))
        }
        // Without operator: the first child is already the primary/expr
        _ => parse_primary(first),
    }
}

fn parse_primary(pair: Pair<Rule>) -> SsmvExpr {
    let inner = match pair.as_rule() {
        Rule::ssmv_primary => pair.into_inner().next().unwrap(),
        other => panic!("Expected ssmv_primary, got {:?}", other),
    };

    match inner.as_rule() {
        Rule::ssmv_number => SsmvExpr::Number(inner.as_str().parse::<i32>().unwrap()),
        Rule::ssmv_boolean_const => SsmvExpr::Bool(inner.as_str() == "TRUE"),
        Rule::ssmv_ident => SsmvExpr::Identifier(inner.as_str().to_string()),
        Rule::ssmv_set_const => {
            let elements = inner.into_inner().map(|p| parse_primary_from(p)).collect();
            SsmvExpr::Set(elements)
        }
        Rule::ssmv_case_expression => {
            let arms = inner
                .into_inner()
                .filter(|p| p.as_rule() == Rule::ssmv_case_arm)
                .map(|arm| {
                    let mut sub = arm.into_inner();
                    let cond = parse_expr(sub.next().unwrap());
                    let val = parse_expr(sub.next().unwrap());
                    (cond, val)
                })
                .collect();
            SsmvExpr::Case(arms)
        }
        // "(" ~ ssmv_expression ~ ")" — the parenthesis is literal, child is ssmv_expression
        Rule::ssmv_expression => parse_expr(inner),
        _ => unreachable!("Unexpected primary child: {:?}", inner.as_rule()),
    }
}

/// Versão de parse_primary que aceita qualquer regra de expressão (usada em Set)
fn parse_primary_from(pair: Pair<Rule>) -> SsmvExpr {
    match pair.as_rule() {
        Rule::ssmv_primary => parse_primary(pair),
        _ => parse_expr(pair),
    }
}

fn parse_ctl(pair: Pair<Rule>) -> CtlFormula {
    match pair.as_rule() {
        Rule::formula | Rule::implication | Rule::iff | Rule::or | Rule::and => {
            let mut inner = pair.into_inner();
            let mut left = parse_ctl(inner.next().unwrap());
            while let Some(op) = inner.next() {
                let right = parse_ctl(inner.next().unwrap());
                left = match op.as_rule() {
                    Rule::op_imply => CtlFormula::Imply(Box::new(left), Box::new(right)),
                    Rule::op_iff => CtlFormula::Iff(Box::new(left), Box::new(right)),
                    Rule::op_or => CtlFormula::Or(Box::new(left), Box::new(right)),
                    Rule::op_and => CtlFormula::And(Box::new(left), Box::new(right)),
                    _ => unreachable!("Unexpected CTL operator: {:?}", op.as_rule()),
                };
            }
            left
        }
        Rule::temporal_binary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::eu => {
                    let mut sub = inner.into_inner();
                    let f1 = parse_ctl(sub.next().unwrap());
                    let f2 = parse_ctl(sub.next().unwrap());
                    CtlFormula::EU(Box::new(f1), Box::new(f2))
                }
                Rule::au => {
                    let mut sub = inner.into_inner();
                    let f1 = parse_ctl(sub.next().unwrap());
                    let f2 = parse_ctl(sub.next().unwrap());
                    CtlFormula::AU(Box::new(f1), Box::new(f2))
                }
                _ => parse_ctl(inner),
            }
        }
        Rule::unary => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();
            match first.as_rule() {
                Rule::unary_op => {
                    let sub = Box::new(parse_ctl(inner.next().unwrap()));
                    match first.as_str() {
                        "EX" => CtlFormula::EX(sub),
                        "AX" => CtlFormula::AX(sub),
                        "EF" => CtlFormula::EF(sub),
                        "AF" => CtlFormula::AF(sub),
                        "EG" => CtlFormula::EG(sub),
                        "AG" => CtlFormula::AG(sub),
                        op => unreachable!("Unknown CTL op: {}", op),
                    }
                }
                Rule::primary => parse_ctl(first),
                _ => CtlFormula::Not(Box::new(parse_ctl(first))),
            }
        }
        Rule::primary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::formula => parse_ctl(inner),
                Rule::constant => match inner.as_str().to_lowercase().as_str() {
                    "true" => CtlFormula::True,
                    "false" => CtlFormula::False,
                    _ => unreachable!(),
                },
                Rule::proposition => CtlFormula::Prop(inner.as_str().to_string()),
                _ => unreachable!("Unexpected primary: {:?}", inner.as_rule()),
            }
        }
        _ => unreachable!("Unexpected CTL rule: {:?}", pair.as_rule()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boolean_variable() {
        let model = parse_ssmv("MODULE main VAR flag : boolean;").unwrap();
        assert_eq!(model.variables.len(), 1);
        assert_eq!(model.variables[0].name, "flag");
        assert!(matches!(model.variables[0].data_type, SsmvType::Boolean));
    }

    #[test]
    fn test_enum_variable() {
        let model = parse_ssmv("MODULE main VAR state : {s0, s1, s2};").unwrap();
        assert_eq!(model.variables.len(), 1);
        if let SsmvType::Enum(vals) = &model.variables[0].data_type {
            assert_eq!(vals, &["s0", "s1", "s2"]);
        } else {
            panic!("Expected Enum type");
        }
    }

    #[test]
    fn test_range_variable() {
        let model = parse_ssmv("MODULE main VAR counter : 0..9;").unwrap();
        assert!(matches!(
            model.variables[0].data_type,
            SsmvType::Range(0, 9)
        ));
    }

    #[test]
    fn test_negative_range_variable() {
        let model = parse_ssmv("MODULE main VAR temp : -10..10;").unwrap();
        assert!(matches!(
            model.variables[0].data_type,
            SsmvType::Range(-10, 10)
        ));
    }

    #[test]
    fn test_init_assignment() {
        let src = "MODULE main VAR s : {a, b}; ASSIGN init(s) := a;";
        let model = parse_ssmv(src).unwrap();
        assert_eq!(model.assignments.len(), 1);
        assert!(matches!(&model.assignments[0], SsmvAssignment::Init(name, _) if name == "s"));
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
        assert_eq!(model.assignments.len(), 1);
        if let SsmvAssignment::Next(_, SsmvExpr::Case(arms)) = &model.assignments[0] {
            assert_eq!(arms.len(), 2);
        } else {
            panic!("Expected Next with Case expression");
        }
    }

    #[test]
    fn test_next_assignment_set() {
        let src = "MODULE main VAR s : {a, b}; ASSIGN next(s) := {a, b};";
        let model = parse_ssmv(src).unwrap();
        if let SsmvAssignment::Next(_, SsmvExpr::Set(elems)) = &model.assignments[0] {
            assert_eq!(elems.len(), 2);
        } else {
            panic!("Expected Next with Set expression");
        }
    }

    #[test]
    fn test_define_section() {
        let src = "MODULE main VAR x : boolean; DEFINE is_active := x & TRUE;";
        let model = parse_ssmv(src).unwrap();
        assert_eq!(model.definitions.len(), 1);
        assert_eq!(model.definitions[0].name, "is_active");
        assert!(
            matches!(&model.definitions[0].expression, SsmvExpr::Binary(_, op, _) if op == "&")
        );
    }

    #[test]
    fn test_ctlspec_ag() {
        let src = "MODULE main VAR s : boolean; CTLSPEC AG s;";
        let model = parse_ssmv(src).unwrap();
        assert_eq!(model.specifications.len(), 1);
        assert!(matches!(&model.specifications[0], CtlFormula::AG(_)));
    }

    #[test]
    fn test_ctlspec_implication() {
        let src = "MODULE main VAR s : boolean; CTLSPEC AG(s -> EF !s);";
        let model = parse_ssmv(src).unwrap();
        assert!(matches!(&model.specifications[0], CtlFormula::AG(_)));
    }

    #[test]
    fn test_ctlspec_eu() {
        let src = "MODULE main VAR s : boolean; CTLSPEC E[s U !s];";
        let model = parse_ssmv(src).unwrap();
        assert!(matches!(&model.specifications[0], CtlFormula::EU(_, _)));
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
                next(timer) := case
                    timer < 9 : timer + 1;
                    TRUE      : 0;
                esac;
            CTLSPEC AG(state = red -> AF state = green)
            CTLSPEC AG EF state = green
        "#;
        let model = parse_ssmv(src).unwrap();
        assert_eq!(model.name, "main");
        assert_eq!(model.variables.len(), 2);
        assert_eq!(model.assignments.len(), 4);
        assert_eq!(model.specifications.len(), 2);
    }

    #[test]
    fn test_display_roundtrip() {
        let src = r#"
            MODULE main
            VAR s : {a, b};
            ASSIGN
                init(s) := a;
                next(s) := b;
            CTLSPEC AG s = a;
        "#;
        let model = parse_ssmv(src).unwrap();
        let output = model.to_string();
        assert!(output.contains("MODULE main"));
        assert!(output.contains("VAR"));
        assert!(output.contains("ASSIGN"));
        assert!(output.contains("CTLSPEC"));
    }
}
