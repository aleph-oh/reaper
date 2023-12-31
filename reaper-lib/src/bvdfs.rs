use std::collections::HashMap;

use crate::{
    enum_predicates,
    types::{ConcTable, ExprNode, Field, PredNode, AST},
};
use bitvec::{prelude as bv, vec::BitVec};
use thiserror::Error;

impl ConcTable {
    fn to_bv(&self) -> bv::BitVec {
        bv::bitvec![1; self.values.len()]
    }
}

#[derive(Error, Debug)]
pub enum BVDFSError {
    #[error("error evaluating query in SQLite")]
    SQLiteError(#[from] rusqlite::Error),
    #[error("error generating predicates")]
    PredicateEnumeration(#[from] enum_predicates::PredicateEnumerationError),
}

#[derive(Debug)]
struct Environment(HashMap<String, isize>);
impl Environment {
    fn from_row(table: &ConcTable, i: usize) -> Self {
        let names = &table.columns;
        let v = &table.values[i];
        let map: HashMap<_, _> = names.iter().cloned().zip(v.iter().copied()).collect();
        Self(map)
    }
}
impl ExprNode {
    fn eval2(&self, env: &Environment) -> Option<isize> {
        match self {
            ExprNode::Field(Field { table: _, name }) => env.0.get(name).copied(),
            ExprNode::Int { value } => Some(*value),
        }
    }
}

impl PredNode {
    fn eval2(&self, env: &Environment) -> Option<bool> {
        match self {
            PredNode::True => Some(true),
            PredNode::Lt { left, right } => left
                .eval2(env)
                .and_then(|left| right.eval2(env).map(|right| left < right)),
            PredNode::Eq { left, right } => left
                .eval2(env)
                .and_then(|left| right.eval2(env).map(|right| left == right)),
            PredNode::And { left, right } => left
                .eval2(env)
                .and_then(|left| right.eval2(env).map(|right| left && right)),
        }
    }
}

pub(crate) fn predicate_vector(rows: &ConcTable, p: &PredNode) -> BitVec {
    let mut v = bv::bitvec![0; rows.values.len()];
    for (i, mut x) in v.iter_mut().enumerate() {
        let env = Environment::from_row(rows, i);
        match p.eval2(&env) {
            Some(b) => *x = b,
            None => return bv::bitvec![0; rows.values.len()],
        }
    }
    v
}

fn cross(v1: &bv::BitSlice, v2: &bv::BitSlice) -> bv::BitVec {
    let mut v = bv::BitVec::new();
    for b in v1.iter() {
        let mut broadcasted = bv::bitvec![if *b {1} else {0}; v2.len()];
        broadcasted &= v2;
        v.append(&mut broadcasted);
    }
    v
}

/// [bvdfs(q, predicates, row_counts, conn)] returns all bitvectors and the corresponding predicate sequence that generated the bitvector,
/// where the predicate pool is drawn from predicates, for the given abstract query.
///
/// All bitvectors should be of the same arity. The ordering of the predicates is the parent node first, then all left children, then all right children,
/// applied recursively, so the same construction should be used when substituting predicate nodes back into the tree.
pub fn bvdfs(
    q: &AST<()>,
    constants: &[isize],
    max_predicate_depth: usize,
    row_counts: &mut HashMap<String, usize>,
    conn: &rusqlite::Connection,
) -> Result<Vec<(bv::BitVec, im::Vector<PredNode>)>, BVDFSError> {
    // TODO: we only look over the representatives
    let predicates =
        crate::enum_predicates::enum_and_group_predicates(q, constants, max_predicate_depth, conn)?;
    let representatives: Vec<_> = predicates
        .values()
        .map(|v| {
            v.first()
                .expect("each vec in predicates must not be empty!")
        })
        .cloned()
        .collect();
    match q {
        AST::Select {
            fields: _,
            table,
            pred: _,
        } => {
            let rows = crate::sql::eval_abstract(q, conn)?;
            let other_vectors = bvdfs(table, constants, max_predicate_depth - 1, row_counts, conn)?;
            let all = representatives
                .iter()
                .flat_map(|p| {
                    let v1 = predicate_vector(&rows, p);
                    other_vectors.iter().map(move |(v2, preds)| {
                        let mut preds = preds.clone();
                        preds.push_front(p.clone());
                        (v1.clone() & v2.clone(), preds)
                    })
                })
                .collect::<Vec<_>>();
            Ok(all)
        }
        AST::Join {
            fields: _,
            table1,
            table2,
            pred: _,
        } => {
            // TODO: use the cached lengths instead of doing an eval_abstract here
            let rows = crate::sql::eval_abstract(q, conn)?;
            let left = bvdfs(table1, constants, max_predicate_depth, row_counts, conn)?;
            let right = bvdfs(table2, constants, max_predicate_depth, row_counts, conn)?;
            let all = representatives
                .iter()
                .flat_map(|p| {
                    let v = predicate_vector(&rows, p);
                    let right = right.clone();
                    left.clone().into_iter().flat_map(move |(l, vl)| {
                        let v = v.clone();
                        let right = right.clone();
                        right.into_iter().map(move |(r, vr)| {
                            let v = cross(&l, &r) & v.clone();
                            let mut vector = vl.clone();
                            vector.append(vr);
                            vector.push_front(p.clone());
                            (v, vector)
                        })
                    })
                })
                .collect();
            Ok(all)
        }
        AST::Table { name, columns: _ } => {
            use std::collections::hash_map::Entry;
            let row_count = match row_counts.entry(name.clone()) {
                Entry::Occupied(e) => *e.get(),
                Entry::Vacant(e) => {
                    let query = AST::Select {
                        fields: None,
                        table: Box::new(q.clone()),
                        pred: (),
                    };
                    let rows = crate::sql::eval_abstract(&query, conn)?;
                    e.insert(rows.values.len());
                    rows.values.len()
                }
            };
            Ok(vec![(bv::bitvec![1; row_count], im::Vector::new())])
        }
        AST::Concat { table1, table2 } => {
            let left = bvdfs(table1, constants, max_predicate_depth, row_counts, conn)?;
            let right = bvdfs(table2, constants, max_predicate_depth, row_counts, conn)?;
            let all = left
                .iter()
                .flat_map(|(l, vl)| {
                    right.iter().cloned().map(|(mut r, vr)| {
                        let mut v = l.clone();
                        v.append(&mut r);
                        let mut preds = vl.clone();
                        preds.append(vr);
                        (v, preds)
                    })
                })
                .collect();
            Ok(all)
        }
    }
}
