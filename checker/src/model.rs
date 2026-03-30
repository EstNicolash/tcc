use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashSet;

pub struct Model {
    pub graph: DiGraph<String, ()>,
    pub initial_labels: std::collections::HashMap<NodeIndex, HashSet<String>>,
    pub initial_states: HashSet<NodeIndex>,
}

impl Model {
    /// Initializes a new, empty Model.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            initial_labels: std::collections::HashMap::new(),
            initial_states: HashSet::new(),
        }
    }

    /// Adds a new state to the structure with its atomic propositions.
    ///
    /// # Arguments
    /// * `name` - A string identifier for the state (e.g., "s0").
    /// * `labels` - A list of atomic propositions true in this state.
    /// * `is_initial` - Whether this state belongs to the set of initial states S0.
    ///
    /// Returns the `NodeIndex` which is required to create transitions.
    pub fn add_state(&mut self, name: &str, labels: Vec<String>, is_initial: bool) -> NodeIndex {
        let index = self.graph.add_node(name.to_string());

        let prop_set: HashSet<String> = labels.into_iter().collect();
        self.initial_labels.insert(index, prop_set);

        if is_initial {
            self.initial_states.insert(index);
        }

        index
    }

    pub fn add_transition(&mut self, from: NodeIndex, to: NodeIndex) {
        self.graph.add_edge(from, to, ());
    }

    pub fn get_all_states(&self) -> petgraph::graph::NodeIndices {
        self.graph.node_indices()
    }
}
