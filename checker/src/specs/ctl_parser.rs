//! Parser for CTL formulas.
//!
//! This module provides a parser for CTL formulas using the pest parser generator.
//!

use crate::specs::ctl_formula::{CtlFormula, CtlFormulaArena, FormulaID};

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "specs/ctl.pest"]
pub struct CtlParser;

/// Entry point to parse a string into a CtlFormula
pub fn parse_ctl_formula(input: &str) -> Result<(CtlFormulaArena, FormulaID), String> {
    let pairs = CtlParser::parse(Rule::main, input).map_err(|e| format!("Parse error: {}", e))?;

    let formula_pair = pairs
        .into_iter()
        .next()
        .unwrap()
        .into_inner()
        .find(|p| p.as_rule() == Rule::formula)
        .expect("Grammar error: formula not found inside main");

    let mut formula_arena = CtlFormulaArena::new();
    let id = parse_expr(formula_pair, &mut formula_arena);
    let result = (formula_arena, id);
    Ok(result)
}

fn parse_expr(pair: Pair<Rule>, formula_arena: &mut CtlFormulaArena) -> FormulaID {
    match pair.as_rule() {
        Rule::formula | Rule::implication | Rule::iff | Rule::or | Rule::and => {
            let mut inner = pair.into_inner();
            let mut left = parse_expr(inner.next().unwrap(), formula_arena);

            while let Some(op) = inner.next() {
                let right = parse_expr(inner.next().unwrap(), formula_arena);
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
                    let f1 = parse_expr(sub.next().unwrap(), formula_arena);
                    let f2 = parse_expr(sub.next().unwrap(), formula_arena);
                    formula_arena.insert(CtlFormula::EU(f1, f2))
                }
                Rule::au => {
                    let mut sub = inner.into_inner();
                    let f1 = parse_expr(sub.next().unwrap(), formula_arena);
                    let f2 = parse_expr(sub.next().unwrap(), formula_arena);
                    formula_arena.insert(CtlFormula::AU(f1, f2))
                }
                _ => parse_expr(inner, formula_arena),
            }
        }

        Rule::unary => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();

            match first.as_rule() {
                Rule::unary_op => {
                    let op_str = first.as_str();
                    let sub = parse_expr(inner.next().unwrap(), formula_arena);
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
                        parse_expr(first, formula_arena)
                    } else {
                        let sub = parse_expr(first, formula_arena);
                        formula_arena.insert(CtlFormula::Not(sub))
                    }
                }
            }
        }

        Rule::primary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::formula => parse_expr(inner, formula_arena),
                Rule::constant => match inner.as_str().to_lowercase().as_str() {
                    "true" => formula_arena.insert(CtlFormula::True),
                    "false" => formula_arena.insert(CtlFormula::False),
                    _ => unreachable!(),
                },
                Rule::proposition => formula_arena.insert_proposition(inner.as_str()),
                _ => unreachable!("Unexpected primary rule: {:?}", inner.as_rule()),
            }
        }

        _ => unreachable!("Unexpected rule: {:?}", pair.as_rule()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let (arena, id) = parse_ctl_formula("is_red").unwrap();
        match arena.get(id) {
            CtlFormula::Prop(p_id) => {
                assert_eq!(arena.format_formula(id), "is_red");
            }
            _ => panic!("Expected Proposition"),
        }
    }

    #[test]
    fn test_parse_unary() {
        let (arena, id) = parse_ctl_formula("!abc").unwrap();
        if let CtlFormula::Not(child_id) = arena.get(id) {
            assert_eq!(arena.format_formula(*child_id), "abc");
        } else {
            panic!("Expected Not operator");
        }
    }

    #[test]
    fn test_parse_precedence() {
        let (arena, id) = parse_ctl_formula("!p & q").unwrap();
        if let CtlFormula::And(left_id, _right_id) = arena.get(id) {
            assert!(matches!(arena.get(*left_id), CtlFormula::Not(_)));
        } else {
            panic!("Precedence failed: expected And at top level");
        }
    }

    #[test]
    fn test_parse_spaces() {
        let mut arena = CtlFormulaArena::new();

        let (arena1, id1) = parse_ctl_formula("AG(p)").unwrap();
        let (arena2, id2) = parse_ctl_formula("AG  (  p  )").unwrap();

        assert_eq!(arena1.format_formula(id1), arena2.format_formula(id2));
    }

    #[test]
    fn test_parse_binary_temporal_until() {
        let (arena, id) = parse_ctl_formula("E[p U q]").unwrap();
        assert!(matches!(arena.get(id), CtlFormula::EU(_, _)));

        let (arena2, id2) = parse_ctl_formula("A[p U q]").unwrap();
        assert!(matches!(arena2.get(id2), CtlFormula::AU(_, _)));
    }

    #[test]
    fn test_parse_constant_case_insensitive() {
        let (_, id1) = parse_ctl_formula("TRUE").unwrap();
        let (arena, id2) = parse_ctl_formula("true").unwrap();

        assert!(matches!(arena.get(id2), CtlFormula::True));
    }
}
