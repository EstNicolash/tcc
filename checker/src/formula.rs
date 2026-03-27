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
