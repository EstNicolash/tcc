use crate::specs::ctl_formula::CtlFormula;
use std::fmt;

#[derive(Debug, Clone)]
pub struct SsmvModel {
    pub name: String,
    pub variables: Vec<SsmvVariable>,
    pub definitions: Vec<SsmvDefine>,
    pub assignments: Vec<SsmvAssignment>,
    pub specifications: Vec<CtlFormula>,
}

#[derive(Debug, Clone)]
pub struct SsmvVariable {
    pub name: String,
    pub data_type: SsmvType,
}

#[derive(Debug, Clone)]
pub enum SsmvType {
    Boolean,
    Enum(Vec<String>),
    Range(i32, i32),
}

#[derive(Debug, Clone)]
pub struct SsmvDefine {
    pub name: String,
    pub expression: SsmvExpr,
}

#[derive(Debug, Clone)]
pub enum SsmvAssignment {
    Init(String, SsmvExpr),
    Next(String, SsmvExpr),
}

#[derive(Debug, Clone)]
pub enum SsmvExpr {
    Identifier(String),
    Number(i32),
    Bool(bool),
    Unary(String, Box<SsmvExpr>),
    Binary(Box<SsmvExpr>, String, Box<SsmvExpr>),
    Case(Vec<(SsmvExpr, SsmvExpr)>),
    Set(Vec<SsmvExpr>),
}

impl fmt::Display for SsmvModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "MODULE {}", self.name)?;

        if !self.variables.is_empty() {
            writeln!(f, "VAR")?;
            for var in &self.variables {
                writeln!(f, "  {}: {};", var.name, var.data_type)?;
            }
        }

        if !self.definitions.is_empty() {
            writeln!(f, "DEFINE")?;
            for def in &self.definitions {
                writeln!(f, "  {} := {};", def.name, def.expression)?;
            }
        }

        if !self.assignments.is_empty() {
            writeln!(f, "ASSIGN")?;
            for assign in &self.assignments {
                match assign {
                    SsmvAssignment::Init(name, expr) => {
                        writeln!(f, "  init({}) := {};", name, expr)?
                    }
                    SsmvAssignment::Next(name, expr) => {
                        writeln!(f, "  next({}) := {};", name, expr)?
                    }
                }
            }
        }

        for spec in &self.specifications {
            writeln!(f, "CTLSPEC {};", spec)?;
        }

        Ok(())
    }
}

impl fmt::Display for SsmvType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SsmvType::Boolean => write!(f, "boolean"),
            SsmvType::Enum(values) => write!(f, "{{{}}}", values.join(", ")),
            SsmvType::Range(lo, hi) => write!(f, "{}..{}", lo, hi),
        }
    }
}

impl fmt::Display for SsmvExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SsmvExpr::Identifier(s) => write!(f, "{}", s),
            SsmvExpr::Number(n) => write!(f, "{}", n),
            SsmvExpr::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            SsmvExpr::Unary(op, expr) => write!(f, "({}{})", op, expr),
            SsmvExpr::Binary(lhs, op, rhs) => write!(f, "({} {} {})", lhs, op, rhs),
            SsmvExpr::Set(vals) => {
                let s: Vec<String> = vals.iter().map(|v| v.to_string()).collect();
                write!(f, "{{{}}}", s.join(", "))
            }
            SsmvExpr::Case(arms) => {
                writeln!(f, "case")?;
                for (cond, val) in arms {
                    writeln!(f, "    {} : {};", cond, val)?;
                }
                write!(f, "  esac")
            }
        }
    }
}
