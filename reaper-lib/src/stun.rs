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

fn synthesize(query: &ASTNode, examples: Examples) -> Option<PredNode> {
    // TODO: look at the Scythe paper to see their approach here.
    // TODO: how do we limit the amount of time the synthesizer spends?
    // TODO: how do we parallelize this nicely?
    // TODO: should we be only returning one candidate predicate? Maybe we can
    // return many if time allows.
    //  - Between an arbitrary stopping condition and parallelism it sounds like
    //  we need a stopper channel that the driver code sends to when it should be
    //  stopping. However, this makes rayon-style parallelism kind of difficult? Maybe
    //  we maintain an atomic flag that says whether to start the search in each
    //  iteration / recursive call.
    todo!()
}
