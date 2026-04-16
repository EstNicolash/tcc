use fixedbitset::FixedBitSet;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateID(pub u32);

pub struct KripkeStructure {
    successors: Vec<StateID>,
    succ_offset: Vec<usize>,

    predecessors: Vec<StateID>,
    pred_offset: Vec<usize>,

    initial_states: FixedBitSet,
    atomic_propositions: Vec<FixedBitSet>,

    states_map: HashMap<Vec<i32>, StateID>,
}

/// Stores the state data and lookup table for the Kripke structure.
///
/// # Example:
/// * If the Model have three variables, then vars_per_state = 3.
/// * The value of each variable is stored sequentially in the `state_data` vector.
/// * The StateID(0) corresponds to state_data\[0 ... vars_per_state - 1].
/// * The StateID(1) corresponds to state_data\[vars_per_state ... 2 * vars_per_state - 1], and so on.
pub struct StateStore {
    state_data: Vec<i32>,
    lookup: HashMap<u64, StateID>,
    vars_per_state: usize,
}

struct RawEdge {
    to: StateID,       //where this edge points to
    next: Option<u32>, //index of the next edge in the same state
}

pub struct EdgeArena {
    heads: Vec<Option<u32>>, //index of the first edge in each state
    edges: Vec<RawEdge>,     //All edges
}
pub struct KripkeBuilder {
    pub states: StateStore,
    pub successors: EdgeArena,
    pub predecessors: EdgeArena,

    pub labels: Vec<FixedBitSet>,
    pub initial_states: FixedBitSet,

    vars_count: usize,
}

impl KripkeBuilder {
    pub fn add_transition(&mut self, from: StateID, to: StateID) {
        self.successors.add_raw_edge(from, to);
        self.predecessors.add_raw_edge(to, from);
    }
}
impl StateStore {
    pub fn get_or_insert(&mut self, values: &[i32]) -> StateID {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        values.hash(&mut hasher);
        let h = hasher.finish();

        if let Some(&id) = self.lookup.get(&h) {
            return id;
        }

        let new_id = StateID(self.lookup.len() as u32);
        self.state_data.extend_from_slice(values);
        self.lookup.insert(h, new_id);

        new_id
    }

    pub fn get_values(&self, id: StateID) -> &[i32] {
        let start = id.0 as usize * self.vars_per_state;
        let end = start + self.vars_per_state;
        &self.state_data[start..end]
    }
}
impl EdgeArena {
    /// Flattens the edge arena into a single vector of edges and a vector of offsets.
    /// Each state's edges are concatenated together, and the offsets indicate where each state's edges start.
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
        offsets.push(current_offset);

        (flat_edges, offsets)
    }

    fn add_raw_edge(&mut self, from: StateID, to: StateID) {
        let new_edge_idx = self.edges.len() as u32;
        self.edges.push(RawEdge {
            to,
            next: self.heads[from.0 as usize],
        });
        self.heads[from.0 as usize] = Some(new_edge_idx);
    }
}

impl KripkeStructure {
    pub fn from_builder(mut builder: KripkeBuilder) -> Self {
        let num_states = builder.states.lookup.len();

        for i in 0..num_states {
            if builder.successors.heads[i].is_none() {
                let sid = StateID(i as u32);
                builder.successors.add_raw_edge(sid, sid);
                builder.predecessors.add_raw_edge(sid, sid);
            }
        }

        let (successors, succ_offset) = builder.successors.flatten(num_states);
        let (predecessors, pred_offset) = builder.predecessors.flatten(num_states);

        Self {
            successors,
            succ_offset,
            predecessors,
            pred_offset,
            initial_states: builder.initial_states,
            atomic_propositions: builder.labels,
            states_map: HashMap::new(),
        }
    }
}
