use crate::formula::CtlFormula;
use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "ctl.pest"]
pub struct CtlParser;

/// Entry point to parse a string into a CtlFormula
pub fn parse_ctl_formula(input: &str) -> Result<CtlFormula, String> {
    let pairs = CtlParser::parse(Rule::main, input).map_err(|e| format!("Parse error: {}", e))?;

    // Rule::main always contains Rule::formula followed by EOI
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
        Rule::formula | Rule::implication | Rule::or | Rule::and => {
            //Pair(Rule::algo)
            let mut inner = pair.into_inner();
            //[Pair(Rule::algo1),Pair(Rule::algo2)]
            let mut left = parse_expr(inner.next().unwrap());

            // Now 'op' will correctly be Rule::op_and, Rule::op_or, etc.
            while let Some(op) = inner.next() {
                let right = parse_expr(inner.next().unwrap());
                left = match op.as_rule() {
                    Rule::op_imply => CtlFormula::Imply(Box::new(left), Box::new(right)),
                    Rule::op_or => CtlFormula::Or(Box::new(left), Box::new(right)),
                    Rule::op_and => CtlFormula::And(Box::new(left), Box::new(right)),
                    _ => unreachable!("Unexpected operator rule"),
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
                _ => parse_expr(inner), // Fallthrough to unary
            }
        }
        Rule::unary => {
            let mut inner = pair.into_inner();
            let op_or_primary = inner.next().unwrap();

            match op_or_primary.as_rule() {
                Rule::unary_op => {
                    let op_str = op_or_primary.as_str();
                    let sub = Box::new(parse_expr(inner.next().unwrap()));
                    match op_str {
                        "!" => CtlFormula::Not(sub),
                        "EX" => CtlFormula::EX(sub),
                        "AX" => CtlFormula::AX(sub),
                        "EF" => CtlFormula::EF(sub),
                        "AF" => CtlFormula::AF(sub),
                        "EG" => CtlFormula::EG(sub),
                        "AG" => CtlFormula::AG(sub),
                        _ => unreachable!(),
                    }
                }
                _ => parse_expr(op_or_primary),
            }
        }
        Rule::primary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::formula => parse_expr(inner),
                Rule::constant => match inner.as_str() {
                    "true" => CtlFormula::True,
                    "false" => CtlFormula::False,
                    _ => unreachable!(),
                },
                Rule::proposition => CtlFormula::Prop(inner.as_str().to_string()),
                _ => unreachable!(),
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
            "Falha ao processar fórmula com caracteres especiais: {:?}",
            f.err()
        );

        let formula = f.unwrap();
        if let CtlFormula::EF(inner) = formula {
            assert!(matches!(*inner, CtlFormula::And(_, _)));
        } else {
            panic!("Esperado operador EF no topo");
        }
    }

    #[test]
    fn test_parse_binary_temporal_until() {
        // E [ f1 U f2 ] e A [ f1 U f2 ]
        let input = "E[p U q]";
        let f = parse_ctl_formula(input).unwrap();
        assert!(matches!(f, CtlFormula::EU(_, _)));

        let input2 = "A[p U q]";
        let f2 = parse_ctl_formula(input2).unwrap();
        assert!(matches!(f2, CtlFormula::AU(_, _)));
    }

    #[test]
    fn test_parse_nested_parentheses() {
        // Teste de aninhamento profundo para garantir que a recursão do parser está ok
        let input = "AG(p -> (EX(q & (r | !s))))";
        let f = parse_ctl_formula(input);
        assert!(f.is_ok());
    }
}
