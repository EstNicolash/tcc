//! # Module kripke_structure
//! This module provides utilities for working with Kripke structures.
//!
//! # Public Types
//! - `StateID`: A unique identifier for a state in the Kripke structure.
//! - `KripkeStructure`: The final, read-only Kripke structure used by the labelling algorithm.
//!

use fixedbitset::FixedBitSet;
use std::collections::HashMap;

/// A unique identifier for a state in the Kripke structure.
///
/// # INVARIANT
/// `StateID(n)` always refers to the n-th state inserted into `StateStore`,
/// with n starting at 0 and incrementing by 1 for each new unique state.
/// `FixedBitSet` indices and `state_data` slice offsets both rely on this
/// property — do NOT construct `StateID` from arbitrary values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateID(pub u32);

/// The final, read-only Kripke structure used by the labelling algorithm.
///
/// Edges are stored as two flat CSR (Compressed Sparse Row) arrays:
/// - `successors`  + `succ_offset`  for forward  traversal
/// - `predecessors` + `pred_offset` for backward traversal
///
/// For a given `StateID(i)`:
/// - successors   are at `successors  [succ_offset[i]  .. succ_offset[i+1]]`
/// - predecessors are at `predecessors[pred_offset[i]  .. pred_offset[i+1]]`
pub struct KripkeStructure {
    successors: Vec<StateID>,
    succ_offset: Vec<usize>,

    predecessors: Vec<StateID>,
    pred_offset: Vec<usize>,

    /// Flat state store — accessible by the labelling algorithm to evaluate
    /// `Prop(SymbolicExprID)` against concrete state values.
    pub states: StateStore,

    /// Bit i is set iff StateID(i) is an initial state.
    /// Capacity is always exactly `num_states` after `from_builder`.
    initial_states: FixedBitSet,
}

/// Stores the flat state data and a lookup table for deduplication.
///
/// # Layout
/// All variable values for all states are packed sequentially in `state_data`:
/// - `StateID(0)` → `state_data[0 .. vars_per_state]`
/// - `StateID(1)` → `state_data[vars_per_state .. 2 * vars_per_state]`
/// - etc.
///
/// The `lookup` maps the full value vector (`Vec<i32>`) to its `StateID`,
/// avoiding hash collision bugs from a raw `u64` key.
pub struct StateStore {
    state_data: Vec<i32>,
    /// * `Key:` full state value vector. Value: assigned StateID. Using Vec<i32> directly avoids silent hash collisions.
    lookup: HashMap<Vec<i32>, StateID>,
    vars_per_state: usize,
}

/// A single directed edge in an `EdgeArena`.
///
/// Edges for the same source state form an intrusive singly-linked list
/// via the `next` field (index into `EdgeArena::edges`).
/// This is a builder-only structure; it is flattened into a CSR array
/// in `KripkeStructure::from_builder`.
struct RawEdge {
    to: StateID,       // destination state
    next: Option<u32>, // index of the next edge for the same source, or None
}

/// Builder-time edge storage using an intrusive linked list per state.
///
/// After construction is complete, call `flatten` to convert to the
/// cache-friendly CSR layout used by `KripkeStructure`.
pub struct EdgeArena {
    /// `heads[i]` is the index in `edges` of the first edge from state `i`,
    /// or `None` if state `i` has no edges yet.
    heads: Vec<Option<u32>>,
    /// All edges, in insertion order.
    edges: Vec<RawEdge>,
}

/// Mutable builder for a `KripkeStructure`.
///
/// Call `add_transition` and `add_initial_state` during BFS expansion,
/// then call `KripkeStructure::from_builder` to produce the final structure.
pub struct KripkeBuilder {
    pub states: StateStore,
    pub successors: EdgeArena,
    pub predecessors: EdgeArena,
    pub initial_states: FixedBitSet,
    vars_count: usize,
}

// ─── KripkeBuilder ────────────────────────────────────────────────────────────

impl KripkeBuilder {
    pub fn new(vars_count: usize) -> Self {
        Self {
            states: StateStore::new(vars_count),
            successors: EdgeArena::new(),
            predecessors: EdgeArena::new(),
            initial_states: FixedBitSet::new(),
            vars_count,
        }
    }

    /// Grows all internal structures to accommodate at least `id`.
    fn ensure_capacity(&mut self, id: StateID) {
        let size = (id.0 + 1) as usize;
        if self.initial_states.len() < size {
            self.initial_states.grow(size);
        }
        self.successors.ensure_capacity(size);
        self.predecessors.ensure_capacity(size);
    }

    /// Marks `id` as an initial state, growing capacity if needed.
    pub fn add_initial_state(&mut self, id: StateID) {
        self.ensure_capacity(id);
        self.initial_states.insert(id.0 as usize);
    }

    /// Adds a directed edge from `from` to `to` in both the successor
    /// and predecessor arenas.
    pub fn add_transition(&mut self, from: StateID, to: StateID) {
        self.successors.add_raw_edge(from, to);
        self.predecessors.add_raw_edge(to, from);
    }
}

// ─── StateStore ───────────────────────────────────────────────────────────────

impl StateStore {
    pub fn new(vars_count: usize) -> Self {
        Self {
            state_data: Vec::new(),
            lookup: HashMap::new(),
            vars_per_state: vars_count,
        }
    }

    /// Returns the `StateID` for `values`, inserting a new state if not present.
    ///
    /// Uses `Vec<i32>` as the hash map key to guarantee absence of collisions —
    /// a raw `u64` hash key would risk silently aliasing distinct states.
    pub fn get_or_insert(&mut self, values: &[i32]) -> StateID {
        // state already known
        if let Some(&id) = self.lookup.get(values) {
            return id;
        }
        // new state
        let new_id = StateID(self.lookup.len() as u32);
        self.state_data.extend_from_slice(values);
        self.lookup.insert(values.to_vec(), new_id);
        new_id
    }

    /// Returns the number of distinct states stored.
    pub fn len(&self) -> usize {
        self.lookup.len()
    }

    /// Returns the flat value slice for `id`.
    ///
    /// # Panics
    /// Panics if `id` is out of range (i.e. was not returned by `get_or_insert`).
    pub fn get_values(&self, id: StateID) -> &[i32] {
        let start = id.0 as usize * self.vars_per_state;
        let end = start + self.vars_per_state;
        &self.state_data[start..end]
    }

    pub fn contains(&self, values: &[i32]) -> bool {
        self.lookup.contains_key(values)
    }
}

// ─── EdgeArena ────────────────────────────────────────────────────────────────

impl EdgeArena {
    pub fn new() -> Self {
        Self {
            heads: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Grows `heads` to hold at least `num_states` entries.
    pub fn ensure_capacity(&mut self, num_states: usize) {
        if self.heads.len() < num_states {
            self.heads.resize(num_states, None);
        }
    }

    /// Prepends an edge `from → to` to the linked list for `from`.
    pub fn add_raw_edge(&mut self, from: StateID, to: StateID) {
        let from_idx = from.0 as usize;
        if from_idx >= self.heads.len() {
            self.heads.resize(from_idx + 1, None);
        }
        let new_edge_idx = self.edges.len() as u32;
        self.edges.push(RawEdge {
            to,
            next: self.heads[from_idx],
        });
        self.heads[from_idx] = Some(new_edge_idx);
    }

    /// Converts the linked-list structure into a cache-friendly CSR layout.
    ///
    /// Returns `(flat_edges, offsets)` where:
    /// - `flat_edges[offsets[i] .. offsets[i+1]]` are the targets of state `i`.
    /// - `offsets` has length `num_states + 1` (sentinel at the end).
    fn flatten(&self, num_states: usize) -> (Vec<StateID>, Vec<usize>) {
        let mut flat_edges = Vec::with_capacity(self.edges.len());
        let mut offsets = Vec::with_capacity(num_states + 1);
        let mut current_offset = 0;

        for i in 0..num_states {
            offsets.push(current_offset);
            let mut curr = self.heads[i];
            while let Some(edge_idx) = curr {
                let edge = &self.edges[edge_idx as usize];
                flat_edges.push(edge.to);
                curr = edge.next;
                current_offset += 1;
            }
        }
        offsets.push(current_offset); // sentinel

        (flat_edges, offsets)
    }
}

// ─── KripkeStructure ──────────────────────────────────────────────────────────

impl KripkeStructure {
    /// Returns the total number of states.
    pub fn num_states(&self) -> usize {
        self.states.len()
    }

    /// Returns the initial-states bitset.
    /// Bit `i` is set iff `StateID(i)` is an initial state.
    pub fn get_initial_states(&self) -> &FixedBitSet {
        &self.initial_states
    }

    /// Returns the successor states of `state` as a slice.
    pub fn get_successors(&self, state: StateID) -> &[StateID] {
        let start = self.succ_offset[state.0 as usize];
        let end = self.succ_offset[state.0 as usize + 1];
        &self.successors[start..end]
    }

    /// Returns the predecessor states of `state` as a slice.
    pub fn get_predecessors(&self, state: StateID) -> &[StateID] {
        let start = self.pred_offset[state.0 as usize];
        let end = self.pred_offset[state.0 as usize + 1];
        &self.predecessors[start..end]
    }

    /// Returns the raw successor offset for `state` (start index in the flat array).
    pub fn get_succ_offset(&self, state: StateID) -> usize {
        self.succ_offset[state.0 as usize]
    }

    /// Returns the raw predecessor offset for `state` (start index in the flat array).
    pub fn get_pred_offset(&self, state: StateID) -> usize {
        self.pred_offset[state.0 as usize]
    }

    /// Consumes a `KripkeBuilder` and produces a finalised `KripkeStructure`.
    ///
    /// # What this does
    /// 1. Ensures all edge arenas are large enough for every known state.
    /// 2. Adds a self-loop to every deadlock state (no successors) to make
    ///    the transition relation total — required for CTL semantics.
    /// 3. Aligns `initial_states` capacity to exactly `num_states`.
    /// 4. Flattens both edge arenas into CSR arrays.
    pub fn from_builder(mut builder: KripkeBuilder) -> Self {
        let num_states = builder.states.len();

        // 1. Guarantee arenas cover every state before the deadlock loop.
        builder.successors.ensure_capacity(num_states);
        builder.predecessors.ensure_capacity(num_states);

        // 2. Add self-loops to deadlock states (makes relation total).
        for i in 0..num_states {
            if builder.successors.heads[i].is_none() {
                let sid = StateID(i as u32);
                builder.successors.add_raw_edge(sid, sid);
                builder.predecessors.add_raw_edge(sid, sid);
            }
        }

        // 3. Align initial_states capacity to exactly num_states.
        //    builder.initial_states may be larger (grown lazily during BFS)
        //    or smaller (if initial states were never explicitly grown).
        //    We create a fresh FixedBitSet of the correct size and copy bits.
        let mut aligned_initial = FixedBitSet::with_capacity(num_states);
        for bit in builder.initial_states.ones() {
            if bit < num_states {
                aligned_initial.insert(bit);
            }
        }

        // 4. Flatten edge arenas into CSR layout.
        let (successors, succ_offset) = builder.successors.flatten(num_states);
        let (predecessors, pred_offset) = builder.predecessors.flatten(num_states);

        Self {
            successors,
            succ_offset,
            predecessors,
            pred_offset,
            states: builder.states,
            initial_states: aligned_initial,
        }
    }
}
