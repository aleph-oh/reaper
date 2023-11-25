use bitvec::prelude as bv;
use core::num;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

use crate::types::*;

#[derive(Copy, Clone, Error, Debug)]
pub enum InvalidQueryError {
    #[error("too few predicates, expected 1, got 0")]
    TooFewPredicates,
    #[error("too many predicates, expected 1, got `{0}`")]
    TooManyPredicates(usize),
}

pub fn synthesize_pred<'a>(
    queries: impl Iterator<Item = &'a ASTNode>,
    examples: Examples,
) -> Result<Vec<PredNode>, Vec<(&'a ASTNode, InvalidQueryError)>> {
    // TODO: are we handling the zero-predicate case well?
    let (queries, errors): (Vec<_>, Vec<_>) = queries
        .map(|query| (query, AbstractQuery::try_from(query)))
        .partition(|(_q, r)| Result::is_ok(r));
    if errors.len() > 0 {
        return Err(errors
            .into_iter()
            .map(|(q, r)| (q, r.unwrap_err()))
            .collect());
    }

    let mut predicates: Vec<_> = queries
        .into_iter()
        .filter_map(|(_q, r)| synthesize(&r.unwrap(), &examples))
        .collect();
    predicates.sort_unstable_by_key(PredNode::height);
    Ok(predicates)
}

impl ExprNode {
    fn height(&self) -> usize {
        match self {
            ExprNode::FieldName { name: _ } => 1,
            ExprNode::Int { value: _ } => 1,
        }
    }
}

impl PredNode {
    fn height(&self) -> usize {
        match self {
            PredNode::Lt { left, right } | PredNode::Eq { left, right } => {
                left.height().max(right.height()) + 1
            }
            PredNode::And { left, right } => left.height().max(right.height()) + 1,
        }
    }
}

/// An [IntermediateTable] references a ConcreteTable and contains
/// a bit-vector of size [t.values.len()] where bit i is set if
/// row i is in the table.
struct IntermediateTable<'a> {
    base: &'a ConcTable,
    rows: bv::BitVec,
}

impl<'a> IntermediateTable<'a> {
    /// [new(t)] returns a new concrete table which represents the same abstract state as [t].
    fn new(base: &'a ConcTable) -> Self {
        let rows = bv::bitvec![1; base.values.len()];
        IntermediateTable { base, rows }
    }

    fn columns(&self) -> &[String] {
        // NOTE: I don't like this return type, but &str and String
        // have a different memory layout so there's no non-allocating
        // way to transform the type.
        self.base.columns.as_ref()
    }
}

// NOTE: if &= ends up being useful, figure out how to implement it. The overloads for
// BitVec don't seem that helpful.

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

impl ASTNode {
    fn num_holes(&self) -> usize {
        match self {
            ASTNode::Select { table, .. } => table.num_holes() + 1,
            ASTNode::Join { table1, table2, .. } => table1.num_holes() + table2.num_holes() + 1,
            ASTNode::Table { .. } => 0,
            ASTNode::Field { .. } => 0,
        }
    }
}

#[derive(Debug)]
struct AbstractQuery(ASTNode);

impl TryFrom<&ASTNode> for AbstractQuery {
    type Error = InvalidQueryError;

    fn try_from(value: &ASTNode) -> Result<Self, Self::Error> {
        match value.num_holes() {
            0 => Err(InvalidQueryError::TooFewPredicates),
            1 => Ok(AbstractQuery(value.clone())),
            n => Err(InvalidQueryError::TooManyPredicates(n)),
        }
    }
}

fn synthesize(query: &AbstractQuery, examples: Examples) -> Option<PredNode> {
    // The Scythe paper implements predicate search as a top-down search where we try to generate predicates (simplest-first)
    // such that they produce the expected output. It is essentially an exhaustive search. This leaves a few questions:
    //  - How do we group predicates? We probably want to map a given query result to all the predicates that produce
    //  that query result.
    //      - How do we do this pruning before concrete evaluation to reduce the number of concrete evaluations?
    //  - This predicate grouping also doesn't handle the case where a query returns too many entries.
    // TODO: how do we limit the amount of time the synthesizer spends?
    //  - limit the depth and the time together, limiting time is a little
    //  less invasive
    // TODO: how do we parallelize this nicely?
    //  - rayon, spawns / parallel iteration have a nice / reasonable API
    // TODO: should we be only returning one candidate predicate? Maybe we can
    // return many if time allows. Combining this with parallelism will require some kind of concurrency-safe vector
    // (or we can put a Mutex around the Vec).
    todo!()
}
