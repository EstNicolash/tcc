//! # Module ctl_formula
//!
//! This module provides a data structure for storing CTL formulas.
//! The implementation uses an arena allocator for efficient storage and a lookup table for fast retrieval.
//!
//! # Data Structures
//!
//! - `FormulaID`: A unique identifier for a CTL formula.
//! - `PropositionID`: A unique identifier for a proposition.
//! - `CtlFormula`: An enum representing the various CTL formula types.
//! - `CtlFormulaArena`: The main data structure that stores CTL formulas using an arena allocator and a lookup table.

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
/// A unique identifier for a CTL formula defined as a "newtype".
pub struct FormulaID(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
/// An enum representing the various CTL formula types as a POD (Plain Old Data)
pub enum CtlFormula<P> {
    True,
    False,
    Not(FormulaID),
    And(FormulaID, FormulaID),
    Or(FormulaID, FormulaID),
    Imply(FormulaID, FormulaID),
    Iff(FormulaID, FormulaID),
    EX(FormulaID),
    AX(FormulaID),
    EU(FormulaID, FormulaID),
    AU(FormulaID, FormulaID),
    AF(FormulaID),
    AG(FormulaID),
    EF(FormulaID),
    EG(FormulaID),
    Prop(P),
}

/// The main data structure that stores CTL formulas using an arena allocator and a lookup table.
///
/// # Fields
///
/// - `formula_arena`: A vector of CTL formulas stored in an arena allocator.
/// - `formula_lookup`: A hash map used for fast lookup of `FormulaID` by `CtlFormula`.
/// - `proposition_arena`: A vector of proposition names stored in an arena allocator.
/// - `proposition_lookup`: A hash map used for fast lookup of `PropositionID` by `String`.
pub struct CtlFormulaArena<P> {
    formula_arena: Vec<CtlFormula<P>>,
    formula_lookup: HashMap<CtlFormula<P>, FormulaID>,
}

impl<P: Copy + Eq + std::hash::Hash> CtlFormulaArena<P> {
    /// Returns a new `CtlFormulasArena` with default capacity.
    pub fn new() -> Self {
        Self {
            formula_arena: Vec::new(),
            formula_lookup: HashMap::new(),
        }
    }

    /// Inserts a `CtlFormula` into the arena and returns its `FormulaID`.
    ///
    ///
    /// # Arguments
    ///
    /// - `formula`: The `CtlFormula` to insert.
    ///
    /// # Returns
    ///
    /// The `FormulaID` of the inserted formula. If the formula is already present in the arena,
    /// its cached `FormulaID` is returned.
    pub fn insert(&mut self, formula: CtlFormula<P>) -> FormulaID {
        if let Some(&cached) = self.formula_lookup.get(&formula) {
            return cached;
        }

        let id = FormulaID(self.formula_arena.len() as u32);
        self.formula_lookup.insert(formula.clone(), id);
        self.formula_arena.push(formula);
        id
    }

    /// Returns a reference to the `CtlFormula` stored at the given `FormulaID`.
    pub fn get(&self, id: FormulaID) -> &CtlFormula<P> {
        &self.formula_arena[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.formula_arena.len()
    }

    /// Returns a string representation of the `CtlFormula` stored at the given `FormulaID`.
    pub fn format_formula<F>(&self, id: FormulaID, format_prop: &F) -> String
    where
        F: Fn(P) -> String,
    {
        match self.get(id) {
            CtlFormula::True => "TRUE".to_string(),
            CtlFormula::False => "FALSE".to_string(),
            CtlFormula::Prop(p) => format_prop(*p),
            CtlFormula::And(f1, f2) => format!(
                "({} & {})",
                self.format_formula(*f1, format_prop),
                self.format_formula(*f2, format_prop)
            ),
            CtlFormula::Or(f1, f2) => format!(
                "({} | {})",
                self.format_formula(*f1, format_prop),
                self.format_formula(*f2, format_prop)
            ),
            CtlFormula::Not(f_id) => format!("!{}", self.format_formula(*f_id, format_prop)),
            CtlFormula::Imply(f1, f2) => format!(
                "({} -> {})",
                self.format_formula(*f1, format_prop),
                self.format_formula(*f2, format_prop)
            ),
            CtlFormula::Iff(f1, f2) => format!(
                "({} <=> {})",
                self.format_formula(*f1, format_prop),
                self.format_formula(*f2, format_prop)
            ),
            CtlFormula::EX(f_id) => format!("EX {}", self.format_formula(*f_id, format_prop)),
            CtlFormula::AX(f_id) => format!("AX {}", self.format_formula(*f_id, format_prop)),
            CtlFormula::EU(f1, f2) => format!(
                "E[{} U {}]",
                self.format_formula(*f1, format_prop),
                self.format_formula(*f2, format_prop)
            ),
            CtlFormula::AU(f1, f2) => format!(
                "A[{} U {}]",
                self.format_formula(*f1, format_prop),
                self.format_formula(*f2, format_prop)
            ),
            CtlFormula::AF(f_id) => format!("AF {}", self.format_formula(*f_id, format_prop)),
            CtlFormula::AG(f_id) => format!("AG {}", self.format_formula(*f_id, format_prop)),
            CtlFormula::EF(f_id) => format!("EF {}", self.format_formula(*f_id, format_prop)),
            CtlFormula::EG(f_id) => format!("EG {}", self.format_formula(*f_id, format_prop)),
        }
    }
}

impl fmt::Display for FormulaID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
