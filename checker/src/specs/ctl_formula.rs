use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CtlFormula {
    True,
    False,
    Prop(String),
    Not(Box<CtlFormula>),
    And(Box<CtlFormula>, Box<CtlFormula>),
    Or(Box<CtlFormula>, Box<CtlFormula>),
    Imply(Box<CtlFormula>, Box<CtlFormula>),
    EX(Box<CtlFormula>),
    AX(Box<CtlFormula>),
    EU(Box<CtlFormula>, Box<CtlFormula>),
    AU(Box<CtlFormula>, Box<CtlFormula>),
    AF(Box<CtlFormula>),
    AG(Box<CtlFormula>),
    EF(Box<CtlFormula>),
    EG(Box<CtlFormula>),
}

impl fmt::Display for CtlFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CtlFormula::True => write!(f, "true"),
            CtlFormula::False => write!(f, "false"),
            CtlFormula::Prop(s) => write!(f, "{}", s),
            CtlFormula::Not(phi) => write!(f, "!{}", phi),
            CtlFormula::And(phi, psi) => write!(f, "({} & {})", phi, psi),
            CtlFormula::Or(phi, psi) => write!(f, "({} | {})", phi, psi),
            CtlFormula::Imply(phi, psi) => write!(f, "({} -> {})", phi, psi),
            CtlFormula::EX(phi) => write!(f, "EX {}", phi),
            CtlFormula::AX(phi) => write!(f, "AX {}", phi),
            CtlFormula::EF(phi) => write!(f, "EF {}", phi),
            CtlFormula::AF(phi) => write!(f, "AF {}", phi),
            CtlFormula::EG(phi) => write!(f, "EG {}", phi),
            CtlFormula::AG(phi) => write!(f, "AG {}", phi),
            CtlFormula::EU(phi, psi) => write!(f, "E[{} U {}]", phi, psi),
            CtlFormula::AU(phi, psi) => write!(f, "A[{} U {}]", phi, psi),
        }
    }
}
