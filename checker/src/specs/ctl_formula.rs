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
/// A unique identifier for a proposition.
pub struct PropositionID(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
/// An enum representing the various CTL formula types as a POD (Plain Old Data)
pub enum CtlFormula {
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
    Prop(PropositionID),
}

/// The main data structure that stores CTL formulas using an arena allocator and a lookup table.
///
/// # Fields
///
/// - `formula_arena`: A vector of CTL formulas stored in an arena allocator.
/// - `formula_lookup`: A hash map used for fast lookup of `FormulaID` by `CtlFormula`.
/// - `proposition_arena`: A vector of proposition names stored in an arena allocator.
/// - `proposition_lookup`: A hash map used for fast lookup of `PropositionID` by `String`.
pub struct CtlFormulaArena {
    formula_arena: Vec<CtlFormula>,
    proposition_arena: Vec<String>,
    formula_lookup: HashMap<CtlFormula, FormulaID>,
    proposition_lookup: HashMap<String, PropositionID>,
}

impl CtlFormulaArena {
    /// Returns a new `CtlFormulasArena` with default capacity.
    pub fn new() -> Self {
        Self {
            formula_arena: Vec::new(),
            formula_lookup: HashMap::new(),
            proposition_lookup: HashMap::new(),
            proposition_arena: Vec::new(),
        }
    }

    /// Returns a new `CtlFormulasArena` with the specified capacity.
    ///
    /// # Arguments
    ///
    /// - `capacity`: The initial capacity of the arena and the lookup table.
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            formula_arena: Vec::with_capacity(capacity),
            formula_lookup: HashMap::with_capacity(capacity),
            proposition_lookup: HashMap::new(),
            proposition_arena: Vec::new(),
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
    pub fn insert(&mut self, formula: CtlFormula) -> FormulaID {
        if let Some(&cached) = self.formula_lookup.get(&formula) {
            return cached;
        }

        let id = FormulaID(self.formula_arena.len() as u32);
        self.formula_lookup.insert(formula.clone(), id);
        self.formula_arena.push(formula);
        id
    }

    /// Inserts a proposition into the arena and returns its `FormulaID`.
    ///
    /// # Arguments
    ///
    /// - `name`: The name of the proposition.
    ///
    /// # Returns
    ///
    /// The `FormulaID` of the inserted proposition. If the proposition is already present in the arena,
    /// its cached `FormulaID` is returned.
    pub fn insert_proposition(&mut self, name: &str) -> FormulaID {
        let prop_id = self.intern_proposition(name);
        self.insert(CtlFormula::Prop(prop_id))
    }

    /// Interns a proposition into the arena and returns its `PropositionID`.
    ///
    /// # Arguments
    ///
    /// - `name`: The name of the proposition.
    ///
    /// # Returns
    ///
    /// The `PropositionID` of the inserted proposition. If the proposition is already present in the arena,
    /// its cached `PropositionID` is returned.
    fn intern_proposition(&mut self, name: &str) -> PropositionID {
        if let Some(&cached) = self.proposition_lookup.get(name) {
            return cached;
        }

        let id = PropositionID(self.proposition_arena.len() as u32);
        let name_string = name.to_string();
        self.proposition_lookup.insert(name_string.clone(), id);
        self.proposition_arena.push(name_string);
        id
    }

    /// Returns a reference to the `CtlFormula` stored at the given `FormulaID`.
    pub fn get(&self, id: FormulaID) -> &CtlFormula {
        &self.formula_arena[id.0 as usize]
    }

    /// Returns a string representation of the `CtlFormula` stored at the given `FormulaID`.
    pub fn format_formula(&self, id: FormulaID) -> String {
        match self.get(id) {
            CtlFormula::True => "TRUE".to_string(),
            CtlFormula::False => "FALSE".to_string(),
            CtlFormula::Prop(p_id) => self.proposition_arena[p_id.0 as usize].clone(),
            CtlFormula::And(f1_id, f2_id) => format!(
                "({} & {})",
                self.format_formula(*f1_id),
                self.format_formula(*f2_id)
            ),
            CtlFormula::Or(f1_id, f2_id) => format!(
                "({} | {})",
                self.format_formula(*f1_id),
                self.format_formula(*f2_id)
            ),
            CtlFormula::Not(f_id) => format!("!{}", self.format_formula(*f_id)),
            CtlFormula::Imply(f1_id, f2_id) => format!(
                "({} => {})",
                self.format_formula(*f1_id),
                self.format_formula(*f2_id)
            ),
            CtlFormula::Iff(f1_id, f2_id) => format!(
                "({} <=> {})",
                self.format_formula(*f1_id),
                self.format_formula(*f2_id)
            ),
            CtlFormula::EX(f_id) => format!("EX {}", self.format_formula(*f_id)),
            CtlFormula::AX(f_id) => format!("AX {}", self.format_formula(*f_id)),
            CtlFormula::EU(f1_id, f2_id) => format!(
                "E[{} U {}]",
                self.format_formula(*f1_id),
                self.format_formula(*f2_id)
            ),
            CtlFormula::AU(f1_id, f2_id) => format!(
                "A[{} U {}]",
                self.format_formula(*f1_id),
                self.format_formula(*f2_id)
            ),
            CtlFormula::AF(f_id) => format!("AF {}", self.format_formula(*f_id)),
            CtlFormula::AG(f_id) => format!("AG {}", self.format_formula(*f_id)),
            CtlFormula::EF(f_id) => format!("EF {}", self.format_formula(*f_id)),
            CtlFormula::EG(f_id) => format!("EG {}", self.format_formula(*f_id)),
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for FormulaID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for PropositionID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for CtlFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CtlFormula::True => write!(f, "TRUE"),
            CtlFormula::False => write!(f, "FALSE"),
            CtlFormula::Prop(s) => write!(f, "{}", s),
            CtlFormula::Not(phi) => write!(f, "!{}", phi),
            CtlFormula::And(phi, psi) => write!(f, "({} & {})", phi, psi),
            CtlFormula::Or(phi, psi) => write!(f, "({} | {})", phi, psi),
            CtlFormula::Imply(phi, psi) => write!(f, "({} -> {})", phi, psi),
            CtlFormula::Iff(phi, psi) => write!(f, "({} <-> {})", phi, psi),
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
