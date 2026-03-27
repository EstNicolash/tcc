mod formula;
mod kripke_structure;
mod labelling;
use formula::CtlFormula;

use petgraph::graph::DiGraph;

fn main() {
    let mut kripke = DiGraph::<&str, ()>::new();
    let s0 = kripke.add_node("s0");
    let s1 = kripke.add_node("s1");
    kripke.add_edge(s0, s1, ());

    println!("{} states.", kripke.node_count());
}
