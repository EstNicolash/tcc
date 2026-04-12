use crate::specs::ctl_formula::CtlFormula;
use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "specs/ctl.pest"]
pub struct CtlParser;

/// Entry point to parse a string into a CtlFormula
pub fn parse_ctl_formula(input: &str) -> Result<CtlFormula, String> {
    let pairs = CtlParser::parse(Rule::main, input).map_err(|e| format!("Parse error: {}", e))?;

    let formula_pair = pairs
        .into_iter()
        .next()
        .unwrap()
        .into_inner()
        .find(|p| p.as_rule() == Rule::formula)
        .expect("Grammar error: formula not found inside main");

    Ok(parse_expr(formula_pair))
}

fn parse_expr(pair: Pair<Rule>) -> CtlFormula {
    match pair.as_rule() {
        Rule::formula | Rule::implication | Rule::iff | Rule::or | Rule::and => {
            let mut inner = pair.into_inner();
            let mut left = parse_expr(inner.next().unwrap());

            while let Some(op) = inner.next() {
                let right = parse_expr(inner.next().unwrap());
                left = match op.as_rule() {
                    Rule::op_imply => CtlFormula::Imply(Box::new(left), Box::new(right)),
                    Rule::op_iff => CtlFormula::Iff(Box::new(left), Box::new(right)),
                    Rule::op_or => CtlFormula::Or(Box::new(left), Box::new(right)),
                    Rule::op_and => CtlFormula::And(Box::new(left), Box::new(right)),
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
                    let f1 = parse_expr(sub.next().unwrap());
                    let f2 = parse_expr(sub.next().unwrap());
                    CtlFormula::EU(Box::new(f1), Box::new(f2))
                }
                Rule::au => {
                    let mut sub = inner.into_inner();
                    let f1 = parse_expr(sub.next().unwrap());
                    let f2 = parse_expr(sub.next().unwrap());
                    CtlFormula::AU(Box::new(f1), Box::new(f2))
                }
                _ => parse_expr(inner),
            }
        }

        Rule::unary => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();

            match first.as_rule() {
                Rule::unary_op => {
                    let op_str = first.as_str();
                    let sub = Box::new(parse_expr(inner.next().unwrap()));
                    match op_str {
                        "EX" => CtlFormula::EX(sub),
                        "AX" => CtlFormula::AX(sub),
                        "EF" => CtlFormula::EF(sub),
                        "AF" => CtlFormula::AF(sub),
                        "EG" => CtlFormula::EG(sub),
                        "AG" => CtlFormula::AG(sub),
                        _ => unreachable!("Unknown CTL op: {}", op_str),
                    }
                }
                _ => {
                    if first.as_rule() == Rule::primary {
                        parse_expr(first)
                    } else {
                        CtlFormula::Not(Box::new(parse_expr(first)))
                    }
                }
            }
        }

        Rule::primary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::formula => parse_expr(inner),
                Rule::constant => match inner.as_str().to_lowercase().as_str() {
                    "true" => CtlFormula::True,
                    "false" => CtlFormula::False,
                    _ => unreachable!(),
                },
                Rule::proposition => CtlFormula::Prop(inner.as_str().to_string()),
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
        let f = parse_ctl_formula("is_red").unwrap();
        assert_eq!(f, CtlFormula::Prop("is_red".to_string()));
    }

    #[test]
    fn test_parse_unary() {
        let f = parse_ctl_formula("!abc").unwrap();
        assert_eq!(
            f,
            CtlFormula::Not(Box::new(CtlFormula::Prop("abc".to_string())))
        );
    }

    #[test]
    fn test_parse_precedence() {
        // !p & q should be (!p) & q, not !(p & q)
        let f = parse_ctl_formula("!p & q").unwrap();
        if let CtlFormula::And(left, _) = f {
            assert!(matches!(*left, CtlFormula::Not(_)));
        } else {
            panic!("Precedence failed: expected And at top level");
        }
    }

    #[test]
    fn test_parse_complex_temporal() {
        let input = "AG(is_green -> AF is_red)";
        let f = parse_ctl_formula(input);
        assert!(f.is_ok(), "Failed to parse complex formula: {:?}", f.err());
    }

    #[test]
    fn test_parse_spaces() {
        let f1 = parse_ctl_formula("AG(p)").unwrap();
        let f2 = parse_ctl_formula("AG  (  p  )").unwrap();
        assert_eq!(f1, f2);
    }

    #[test]
    fn test_parse_large_conjunction_with_special_chars() {
        let input = "EF (cId[0]=1 & p.state=WAIT & req[1]=0 & guard_1=true & M1=3)";
        let f = parse_ctl_formula(input);
        assert!(
            f.is_ok(),
            "Failed to process formula with special characters: {:?}",
            f.err()
        );
        let formula = f.unwrap();
        if let CtlFormula::EF(inner) = formula {
            assert!(matches!(*inner, CtlFormula::And(_, _)));
        } else {
            panic!("Expected operator EF at the top level");
        }
    }

    #[test]
    fn test_parse_binary_temporal_until() {
        let input = "E[p U q]";
        let f = parse_ctl_formula(input).unwrap();
        assert!(matches!(f, CtlFormula::EU(_, _)));

        let input2 = "A[p U q]";
        let f2 = parse_ctl_formula(input2).unwrap();
        assert!(matches!(f2, CtlFormula::AU(_, _)));
    }

    #[test]
    fn test_parse_nested_parentheses() {
        let input = "AG(p -> (EX(q & (r | !s))))";
        let f = parse_ctl_formula(input);
        assert!(f.is_ok());
    }

    #[test]
    fn test_parse_iff() {
        let f = parse_ctl_formula("p <-> q").unwrap();
        assert!(matches!(f, CtlFormula::Iff(_, _)));
    }

    #[test]
    fn test_parse_implication_right_assoc() {
        let f = parse_ctl_formula("p -> q -> r").unwrap();
        if let CtlFormula::Imply(_, right) = f {
            assert!(matches!(*right, CtlFormula::Imply(_, _)));
        } else {
            panic!("Expected Imply at top level");
        }
    }

    #[test]
    fn test_parse_constant_case_insensitive() {
        let f1 = parse_ctl_formula("TRUE").unwrap();
        let f2 = parse_ctl_formula("true").unwrap();
        let f3 = parse_ctl_formula("True").unwrap();
        assert_eq!(f1, CtlFormula::True);
        assert_eq!(f2, CtlFormula::True);
        assert_eq!(f3, CtlFormula::True);
    }
}
