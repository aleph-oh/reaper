use std::sync::atomic::{AtomicBool, Ordering};

use crate::types::*;

pub fn synthesize_pred<'a>(
    queries: impl Iterator<Item = &'a ASTNode>,
    examples: Examples,
) -> Vec<PredNode> {
    let mut predicates: Vec<_> = queries
        .filter_map(|query| synthesize(query, examples))
        .collect();
    predicates.sort_unstable_by_key(PredNode::depth);
    predicates
}

impl ExprNode {
    fn depth(&self) -> usize {
        match self {
            ExprNode::FieldName { name: _ } => 1,
            ExprNode::Int { value: _ } => 1,
        }
    }
}

impl PredNode {
    fn depth(&self) -> usize {
        match self {
            PredNode::Lt { left, right } | PredNode::Eq { left, right } => {
                left.depth().max(right.depth()) + 1
            }
            PredNode::And { left, right } => left.depth().max(right.depth()) + 1,
        }
    }
}

/// [run_unless_stopped] runs [f] unless [stopper] is set.
///
/// It's recommended that [f] itself uses [run_unless_stopped] during different phases of
/// expensive computation to ensure that the request to stop is respected as best as possible.
fn run_unless_stopped<T>(f: impl FnOnce() -> T, stopper: &AtomicBool) -> Option<T> {
    match stopper.load(Ordering::SeqCst) {
        true => None,
        false => Some(f()),
    }
}

fn synthesize(query: &ASTNode, examples: Examples) -> Option<PredNode> {
    // TODO: look at the Scythe paper to see their approach here.
    // TODO: how do we limit the amount of time the synthesizer spends?
    //  - limit the depth and the time together, limiting time is a little
    //  less invasive
    // TODO: how do we parallelize this nicely?
    //  - rayon, spawns / parallel iteration have a nice / reasonable API
    // TODO: should we be only returning one candidate predicate? Maybe we can
    // return many if time allows.
    todo!()
}
