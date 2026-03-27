use petgraph::graph::DiGraph;

#[derive(Debug, Clone, PartialEq)]
enum Formula {
    Prop(String),
    Not(Box<Formula>),
    And(Box<Formula>, Box<Formula>),
    EX(Box<Formula>),
    AF(Box<Formula>),
    EU(Box<Formula>, Box<Formula>),
}

fn main() {
    let mut kripke = DiGraph::<&str, ()>::new();
    let s0 = kripke.add_node("s0");
    let s1 = kripke.add_node("s1");
    kripke.add_edge(s0, s1, ());

    println!("{} states.", kripke.node_count());
}
