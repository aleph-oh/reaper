use bitvec::prelude as bv;
use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use thiserror::Error;

use crate::types::*;

#[derive(Copy, Clone, Error, Debug)]
pub enum InvalidQueryError {
    #[error("too few predicates, expected 1, got 0")]
    TooFewPredicates,
    #[error("too many predicates, expected 1, got `{0}`")]
    TooManyPredicates(usize),
}

/// [synthesize_pred(query, examples)] synthesizes all predicates found that,
/// with all examples (i, o), substituting the predicate into the query yields
/// a query q such that q(i) = o.
///
/// If the query is not a valid abstract query, an error is returned.
/// If no predicate is found, the returned Vec is empty.
pub fn synthesize_pred<'a>(
    query: &ASTNode,
    examples: Examples,
) -> Result<Vec<PredNode>, InvalidQueryError> {
    let query = match AbstractQuery::try_from(query) {
        Ok(query) => query,
        Err(InvalidQueryError::TooFewPredicates) => return Ok(vec![PredNode::True]),
        Err(e) => return Err(e),
    };
    let mut predicates = synthesize(&query, &examples);
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
    // TODO: test that this works
}

impl PredNode {
    fn height(&self) -> usize {
        match self {
            PredNode::True => 1,
            PredNode::Lt { left, right } | PredNode::Eq { left, right } => {
                left.height().max(right.height()) + 1
            }
            PredNode::And { left, right } => left.height().max(right.height()) + 1,
        }
    }

    // TODO: test that this works
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
    /// [t.num_holes()] returns the number of holes in the AST.
    fn num_holes(&self) -> usize {
        match self {
            ASTNode::Select { table, .. } => table.num_holes() + 1,
            ASTNode::Join { table1, table2, .. } => table1.num_holes() + table2.num_holes() + 1,
            ASTNode::Concat { table1, table2 } => table1.num_holes() + table2.num_holes(),
            ASTNode::Table { .. } => 0,
        }
    }

    // TODO: check that we test hole count properly here.
}

#[derive(Debug)]
/// AbstractQuery represents a query with *exactly one* hole.
struct AbstractQuery(ASTNode);

impl AbstractQuery {
    /// [q.with_predicate(p)] returns a new query where p is substituted for the hole in q.
    fn with_predicate(&self, pred: PredNode) -> ASTNode {
        debug_assert!(self.0.num_holes() == 1);
        match &self.0 {
            ASTNode::Select { fields, table, .. } => ASTNode::Select {
                fields: fields.clone(),
                table: Rc::clone(table),
                pred,
            },
            ASTNode::Join { table1, table2, .. } => ASTNode::Join {
                table1: Rc::clone(table1),
                table2: Rc::clone(table2),
                pred,
            },
            ASTNode::Concat { table1, table2 } => ASTNode::Concat {
                table1: Rc::clone(table1),
                table2: Rc::clone(table2),
            },
            // NOTE: I don't like catch-all patterns for types that might change in the near-future,
            // so this match explicitly checks the remaining cases. We could do better by also
            // expanding all the fields, but that feels verbose.
            n @ ASTNode::Table { .. } => n.clone(),
        }
    }
}

impl TryFrom<&ASTNode> for AbstractQuery {
    type Error = InvalidQueryError;

    fn try_from(value: &ASTNode) -> Result<Self, Self::Error> {
        match value.num_holes() {
            0 => Err(InvalidQueryError::TooFewPredicates),
            1 => Ok(AbstractQuery(value.clone())),
            n => Err(InvalidQueryError::TooManyPredicates(n)),
        }
    }

    // TODO: test that we correctly error for cases w/ many holes.
}

fn synthesize(query: &AbstractQuery, examples: Examples) -> Vec<PredNode> {
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
    todo!()
}
