use std::collections::{HashMap, HashSet};

use bitvec::prelude as bv;
use enum_predicates::enum_and_group_predicates;
use sql::eval_abstract;
use thiserror::Error;

pub mod bottomup;
pub mod bvdfs;
pub mod enum_predicates;
pub mod sql;
pub mod stun;
pub mod types;

fn query_rank(q: &types::AST<types::PredNode>) -> isize {
    // TODO: do something more sophisticated
    q.height() as isize
}

#[derive(Error, Debug)]
pub enum SynthesisError {
    #[error("failed to enumerate predicates")]
    Enumeration(#[from] enum_predicates::PredicateEnumerationError),
    #[error("failed during BVDFS to find target bitvector")]
    BVDFS(#[from] bvdfs::BVDFSError),
    #[error("failed during database query")]
    Database(#[from] rusqlite::Error),
    #[error("failed to find a satisfying query")]
    NoQueriesFound,
}

impl types::ConcTable {
    fn subset_bitvec(&self, other: &Self) -> bv::BitVec {
        let other_rows = other.values.iter().collect::<HashSet<_>>();
        let mut v = bv::bitvec![0; self.values.len()];
        for (mut b, row) in v.iter_mut().zip(self.values.iter()) {
            *b = other_rows.contains(row);
        }
        v
    }
}

pub fn synthesize(
    q: &types::AST<()>,
    target: &types::ConcTable,
    constants: &[isize],
    max_predicate_depth: usize,
    conn: &rusqlite::Connection,
) -> Result<Vec<types::AST<types::PredNode>>, SynthesisError> {
    let bitvectors = bvdfs::bvdfs(q, constants, max_predicate_depth, &mut HashMap::new(), conn)?;
    // TODO: make the return type of bvdfs less stupid. probably should be a hashmap from bitvecs to all predicate vectors that
    // produce that value. Without that change, it's not really better to make a HashMap out of these since we could just iterate.
    let t = eval_abstract(q, conn)?;
    let target_bv = t.subset_bitvec(target);
    let mut queries: Vec<_> = bitvectors
        .into_iter()
        .filter_map(|(bv, preds)| {
            if bv == target_bv {
                let preds = preds.into_iter().collect::<Vec<_>>();
                // TODO: with_predicates should probably accept an im::Vector instead.
                let q = q
                    .with_predicates(&preds)
                    .expect("query substitution failed!");
                Some(q)
            } else {
                None
            }
        })
        .collect();
    if queries.is_empty() {
        Err(SynthesisError::NoQueriesFound)
    } else {
        queries.sort_by_key(query_rank);
        Ok(queries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 4;
        assert_eq!(result, 4);
    }
}
