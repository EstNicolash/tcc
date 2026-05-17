//! # Module ssmv_ast
//!
//! This module provides the AST (Abstract Syntax Tree) representation of SSMV models.
//!
//! # Public Types and Structures
//!
//! - [`ExprID`]: Represents an expression ID.
//! - [`IdentifierID`]: Represents an identifier ID.
//! - [`SsmvExpr`]: Represents an SSMV expression.
//! - [`SsmvType`]: Represents an SSMV type.
//! - [`SsmvVariable`]: Represents an SSMV variable.
//! - [`SsmvDefine`]: Represents an SSMV define.
//! - [`SsmvAssignment`]: Represents an SSMV assignment.
//!

use crate::specs::ctl_formula::{CtlFormulaArena, FormulaID};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprID(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentifierID(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SsmvExpr {
    Identifier(IdentifierID),
    Number(i32),
    Bool(bool),
    Unary(IdentifierID, ExprID),
    Binary(ExprID, IdentifierID, ExprID),
    Case(u32, u32), // (start_index, length) in the arena's case_arms vector.
    Set(u32, u32),  // (start_index, length) in the arena's set_elements vector.
}

#[derive(Debug, Clone)]
pub enum SsmvType {
    Boolean,
    Enum(Vec<IdentifierID>),
    Range(i32, i32),
}

#[derive(Debug, Clone)]
pub struct SsmvVariable {
    pub name: IdentifierID,
    pub data_type: SsmvType,
}

#[derive(Debug, Clone)]
pub struct SsmvDefine {
    pub name: IdentifierID,
    pub expression: ExprID,
}

#[derive(Debug, Clone)]
pub enum SsmvAssignment {
    Init(IdentifierID, ExprID),
    Next(IdentifierID, ExprID),
}
/// Represents an SSMV model, including its variables, definitions, assignments, and specifications.
pub struct SsmvModel {
    pub name: String,
    pub variables: Vec<SsmvVariable>,
    pub definitions: Vec<SsmvDefine>,
    pub assignments: Vec<SsmvAssignment>,
    pub specifications: Vec<FormulaID>,
    pub arena: SsmvArena,
    pub ctl_arena: CtlFormulaArena<ExprID>,
}

/// Stores all expressions and identifiers for the SSMV model in an arena.
pub struct SsmvArena {
    pub expressions: Vec<SsmvExpr>,
    pub identifiers: Vec<String>,

    // (condition, result) pairs for case expressions.
    // The case arms are stored sequentially in the arena, then Case(u32, u32) references the interval for the respective case.
    pub case_arms: Vec<(ExprID, ExprID)>,

    // The set elements are stored sequentially in the arena, so works like the case arms.
    pub set_elements: Vec<ExprID>,
    expr_lookup: HashMap<SsmvExpr, ExprID>,
    id_lookup: HashMap<String, IdentifierID>,
}

impl SsmvArena {
    /// Constructs a new, empty arena.
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
            identifiers: Vec::new(),
            case_arms: Vec::new(),
            set_elements: Vec::new(),
            expr_lookup: HashMap::new(),
            id_lookup: HashMap::new(),
        }
    }

    /// Interns an identifier into the arena, returning its ID.
    /// If the identifier is already present, returns the existing ID.
    pub fn intern_identifier(&mut self, name: &str) -> IdentifierID {
        if let Some(&id) = self.id_lookup.get(name) {
            return id;
        }
        let id = IdentifierID(self.identifiers.len() as u32);
        let s = name.to_string();
        self.id_lookup.insert(s.clone(), id);
        self.identifiers.push(s);
        id
    }

    /// Inserts an expression into the arena, returning its ID.
    /// If the expression is already present, returns the existing ID.
    pub fn insert_expr(&mut self, expr: SsmvExpr) -> ExprID {
        if let Some(&id) = self.expr_lookup.get(&expr) {
            return id;
        }
        let id = ExprID(self.expressions.len() as u32);
        self.expr_lookup.insert(expr, id);
        self.expressions.push(expr);
        id
    }

    /// Special allocator for Case blocks
    pub fn alloc_case(&mut self, arms: Vec<(ExprID, ExprID)>) -> ExprID {
        let start = self.case_arms.len() as u32;
        let len = arms.len() as u32;
        self.case_arms.extend(arms);
        self.insert_expr(SsmvExpr::Case(start, len))
    }

    /// Special allocator for Sets
    pub fn alloc_set(&mut self, elements: Vec<ExprID>) -> ExprID {
        let start = self.set_elements.len() as u32;
        let len = elements.len() as u32;
        self.set_elements.extend(elements);
        self.insert_expr(SsmvExpr::Set(start, len))
    }

    /// Returns the string representation of an identifier by its ID.
    pub fn get_ident(&self, id: IdentifierID) -> &str {
        &self.identifiers[id.0 as usize]
    }
}

impl SsmvArena {
    pub fn format_expr(&self, id: ExprID) -> String {
        match self.expressions[id.0 as usize] {
            SsmvExpr::Identifier(i) => self.get_ident(i).to_string(),
            SsmvExpr::Number(n) => n.to_string(),
            SsmvExpr::Bool(b) => {
                if b {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                }
            }
            SsmvExpr::Unary(op, e) => format!("({}{})", self.get_ident(op), self.format_expr(e)),
            SsmvExpr::Binary(l, op, r) => format!(
                "({} {} {})",
                self.format_expr(l),
                self.get_ident(op),
                self.format_expr(r)
            ),
            SsmvExpr::Set(start, len) => {
                let parts: Vec<String> = self.set_elements[start as usize..(start + len) as usize]
                    .iter()
                    .map(|&e| self.format_expr(e))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            SsmvExpr::Case(start, len) => {
                let mut s = String::from("case\n");
                for i in 0..len {
                    let (cond, val) = self.case_arms[(start + i) as usize];
                    s.push_str(&format!(
                        "    {} : {};\n",
                        self.format_expr(cond),
                        self.format_expr(val)
                    ));
                }
                s.push_str("esac");
                s
            }
        }
    }
}

impl SsmvModel {
    pub fn format(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("MODULE {}\n\n", self.name));

        if !self.variables.is_empty() {
            s.push_str("VAR\n");
            for var in &self.variables {
                s.push_str(&format!(
                    "    {} : {};\n",
                    self.arena.get_ident(var.name),
                    self.format_type(&var.data_type)
                ));
            }
            s.push_str("\n");
        }

        if !self.definitions.is_empty() {
            s.push_str("DEFINE\n");
            for def in &self.definitions {
                s.push_str(&format!(
                    "    {} := {};\n",
                    self.arena.get_ident(def.name),
                    self.arena.format_expr(def.expression)
                ));
            }
            s.push_str("\n");
        }

        if !self.assignments.is_empty() {
            s.push_str("ASSIGN\n");
            for assign in &self.assignments {
                match assign {
                    SsmvAssignment::Init(name_id, expr_id) => {
                        s.push_str(&format!(
                            "    init({}) := {};\n",
                            self.arena.get_ident(*name_id),
                            self.arena.format_expr(*expr_id)
                        ));
                    }
                    SsmvAssignment::Next(name_id, expr_id) => {
                        s.push_str(&format!(
                            "    next({}) := {};\n",
                            self.arena.get_ident(*name_id),
                            self.arena.format_expr(*expr_id)
                        ));
                    }
                }
            }
            s.push_str("\n");
        }

        for &spec_id in &self.specifications {
            let formatted_formula = self
                .ctl_arena
                .format_formula(spec_id, &|expr_id| self.arena.format_expr(expr_id));
            s.push_str(&format!("CTLSPEC {};\n", formatted_formula));
        }

        s
    }
    fn format_type(&self, t: &SsmvType) -> String {
        match t {
            SsmvType::Boolean => "boolean".to_string(),
            SsmvType::Enum(vals) => {
                let names: Vec<_> = vals.iter().map(|&id| self.arena.get_ident(id)).collect();
                format!("{{{}}}", names.join(", "))
            }
            SsmvType::Range(lo, hi) => format!("{}..{}", lo, hi),
        }
    }
}
