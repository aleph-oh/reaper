use std::collections::HashMap;

use crate::types::{ConcTable, ExprNode, Field, PredNode, AST};
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
}
struct Environment(HashMap<String, isize>);
impl Environment {
    fn from_row(table: &ConcTable, i: usize) -> Self {
        let names = &table.columns;
        let v = &table.values[i];
        let map: HashMap<_, _> = names
            .iter()
            .map(|name| format!("{}.{}", table.name, name))
            .zip(v.iter().copied())
            .collect();
        Self(map)
    }
}
impl ExprNode {
    fn eval2(&self, env: &Environment) -> isize {
        match self {
            ExprNode::Field(Field { table: _, name }) => *env
                .0
                .get(name)
                .expect("well-formed predicate implies a value in env"),
            ExprNode::Int { value } => *value,
        }
    }
}

impl PredNode {
    fn eval2(&self, env: &Environment) -> bool {
        match self {
            PredNode::True => true,
            PredNode::Lt { left, right } => left.eval2(env) < right.eval2(env),
            PredNode::Eq { left, right } => left.eval2(env) == right.eval2(env),
            PredNode::And { left, right } => left.eval2(env) && right.eval2(env),
        }
    }
}

fn predicate_vectors(rows: &ConcTable, predicates: &[PredNode]) -> Vec<BitVec> {
    predicates
        .into_iter()
        .map(|p| {
            let mut v = bv::bitvec![0; rows.values.len()];
            v.iter_mut().enumerate().for_each(|(i, mut x)| {
                let env = Environment::from_row(&rows, i);
                *x = p.eval2(&env);
            });
            v
        })
        .collect()
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

pub fn bvdfs(
    q: &AST<()>,
    predicates: &[PredNode],
    row_counts: &mut HashMap<String, usize>,
    conn: &rusqlite::Connection,
) -> Result<Vec<bv::BitVec>, BVDFSError> {
    match q {
        AST::Select {
            fields: _,
            table,
            pred: _,
        } => {
            let rows = crate::sql::eval_abstract(&q, conn)?;
            let predicate_vectors: Vec<_> = predicates
                .into_iter()
                .map(|p| {
                    let mut v = bv::bitvec![0; rows.values.len()];
                    v.iter_mut().enumerate().for_each(|(i, mut x)| {
                        let env = Environment::from_row(&rows, i);
                        *x = p.eval2(&env);
                    });
                    v
                })
                .collect();
            let other_vectors = bvdfs(table, predicates, row_counts, conn)?;
            let all = predicate_vectors
                .into_iter()
                .flat_map(|v1| other_vectors.iter().map(move |v2| v1.clone() & v2.clone()))
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
            let rows = crate::sql::eval_abstract(&q, conn)?;
            let predicate_vectors = predicate_vectors(&rows, predicates);
            let left = bvdfs(table1, predicates, row_counts, conn)?;
            let right = bvdfs(table2, predicates, row_counts, conn)?;
            let all = predicate_vectors
                .iter()
                .flat_map(|v| {
                    left.iter()
                        .flat_map(|l| right.iter().map(|r| cross(l, r) & v.clone()))
                })
                .collect();
            Ok(all)
        }
        AST::Table { name, columns: _ } => {
            let row_count = row_counts.entry(name.clone()).or_insert_with(|| {
                let query = AST::Select {
                    fields: None,
                    table: Box::new(q.clone()),
                    pred: (),
                };
                let rows =
                    crate::sql::eval_abstract(&query, conn).expect("failed to eval abstract query");
                rows.values.len()
            });
            Ok(vec![bv::bitvec![1; *row_count]])
        }
        AST::Concat { table1, table2 } => {
            let left = bvdfs(table1, predicates, row_counts, conn)?;
            let right = bvdfs(table2, predicates, row_counts, conn)?;
            let all = left
                .iter()
                .flat_map(|l| {
                    right.iter().cloned().map(|mut r| {
                        let mut l = l.clone();
                        l.append(&mut r);
                        l
                    })
                })
                .collect();
            Ok(all)
        }
    }
}
