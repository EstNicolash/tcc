use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashSet;

pub struct KripkeStructure {
    pub graph: DiGraph<String, ()>,
    pub initial_labels: std::collections::HashMap<NodeIndex, HashSet<String>>,
}
