use bitvec::prelude as bv;
use std::{
    collections::HashMap,
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

#[derive(Error, Debug)]
pub enum PredicateSynthesisError {
    #[error("query execution failed")]
    DbExecFailed(#[from] rusqlite::Error),
    #[error("invalid query")]
    InvalidQuery(#[from] InvalidQueryError),
}

/// [synthesize_pred(query, example)] synthesizes all predicates found that,
/// with all examples (i, o), substituting the predicate into the query yields
/// a query q such that q(i) = o.
///
/// If the query is not a valid abstract query, an error is returned.
/// If no predicate is found, the returned Vec is empty.
pub fn synthesize_pred<'a>(
    query: &AST<()>,
    target: &ConcTable,
    conn: &rusqlite::Connection,
    fields: &[Field],
    constants: &[isize],
    max_depth: usize,
) -> Result<Vec<PredNode>, PredicateSynthesisError> {
    let mut predicates = synthesize(&query, target, conn, fields, constants, max_depth)?;
    predicates.sort_unstable_by_key(PredNode::height);
    Ok(predicates)
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

impl AST<()> {
    /// [t.num_holes()] returns the number of holes in the AST.
    pub(crate) fn num_holes(&self) -> usize {
        match self {
            AST::Select { table, .. } => table.num_holes() + 1,
            AST::Join { table1, table2, .. } => table1.num_holes() + table2.num_holes() + 1,
            AST::Concat { table1, table2 } => table1.num_holes() + table2.num_holes(),
            AST::Table { .. } => 0,
        }
    }

    fn with_predicates_aux<'pred>(
        &self,
        predicates: &'pred [PredNode],
    ) -> Result<(AST<PredNode>, &'pred [PredNode]), ()> {
        match self {
            AST::Select {
                fields,
                table,
                pred: _,
            } => {
                let (pred, predicates) = predicates.split_first().ok_or(())?;
                let (table, predicates) = table.with_predicates_aux(predicates)?;
                Ok((
                    AST::Select {
                        fields: fields.as_ref().map(Rc::clone),
                        table: Box::new(table),
                        pred: pred.clone(),
                    },
                    predicates,
                ))
            }
            AST::Join {
                fields,
                table1,
                table2,
                pred: _,
            } => {
                let (pred, predicates) = predicates.split_first().ok_or(())?;
                let (table1, predicates) = table1.with_predicates_aux(predicates)?;
                let (table2, predicates) = table2.with_predicates_aux(predicates)?;
                Ok((
                    AST::Join {
                        fields: fields.as_ref().map(Rc::clone),
                        table1: Box::new(table1),
                        table2: Box::new(table2),
                        pred: pred.clone(),
                    },
                    predicates,
                ))
            }
            AST::Table { name, columns } => Ok((
                AST::Table {
                    name: name.clone(),
                    columns: columns.clone(),
                },
                predicates,
            )),
            AST::Concat { table1, table2 } => {
                let (table1, predicates) = table1.with_predicates_aux(predicates)?;
                let (table2, predicates) = table2.with_predicates_aux(predicates)?;
                Ok((
                    AST::Concat {
                        table1: Box::new(table1),
                        table2: Box::new(table2),
                    },
                    predicates,
                ))
            }
        }
    }

    pub(crate) fn with_predicates(&self, predicates: &[PredNode]) -> Result<AST<PredNode>, ()> {
        Ok(self.with_predicates_aux(predicates)?.0)
    }

    // TODO: check that we test hole count properly here.
}

impl ConcTable {
    fn to_intermediate<'a>(&self, base: &'a ConcTable) -> IntermediateTable<'a> {
        IntermediateTable {
            base,
            rows: bv::bitvec![1; base.values.len()],
        }
    }
}

// TODO: maybe not strings here?
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
    fn eval(&self, env: &Environment) -> isize {
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
    fn eval(&self, env: &Environment) -> bool {
        match self {
            PredNode::True => true,
            PredNode::Lt { left, right } => left.eval(env) < right.eval(env),
            PredNode::Eq { left, right } => left.eval(env) == right.eval(env),
            PredNode::And { left, right } => left.eval(env) && right.eval(env),
        }
    }
}

fn base_exprs(fields: &[Field], constants: &[isize]) -> Vec<ExprNode> {
    fields
        .iter()
        .map(|field| ExprNode::Field(field.clone()))
        .chain(constants.iter().map(|n| ExprNode::Int { value: *n }))
        .collect()
}

fn base_preds(fields: &[Field], constants: &[isize]) -> Vec<PredNode> {
    let exprs = base_exprs(fields, constants);
    let mut new = Vec::with_capacity(exprs.len() * 2);
    new.push(PredNode::True);
    for (i, left) in exprs.iter().enumerate() {
        for right in exprs[i..].iter() {
            new.push(PredNode::Eq {
                left: left.clone(),
                right: right.clone(),
            });
            new.push(PredNode::Lt {
                left: left.clone(),
                right: right.clone(),
            });
        }
    }
    new
}

fn grow(with: &[PredNode], base_predicates: &[PredNode]) -> Vec<PredNode> {
    let mut new = Vec::with_capacity(with.len());
    for p1 in with.iter().chain(base_predicates.iter()) {
        for p2 in with.iter().chain(base_predicates.iter()).cloned() {
            new.push(PredNode::And {
                left: Box::new(p1.clone()),
                right: Box::new(p2),
            })
        }
        new.push(PredNode::And {
            left: Box::new(p1.clone()),
            right: Box::new(PredNode::True),
        });
    }
    new
}

fn synthesize(
    query: &AST<()>,
    target: &ConcTable,
    conn: &rusqlite::Connection,
    fields: &[Field],
    constants: &[isize],
    max_depth: usize,
) -> Result<Vec<PredNode>, PredicateSynthesisError> {
    // The Scythe paper implements predicate search as a top-down search where we try to generate predicates (simplest-first)
    // such that they produce the expected output. It is essentially an exhaustive search. This leaves a few questions:
    //  - How do we group predicates? We probably want to map a given query result to all the predicates that produce
    //  that query result.
    //      - How do we do this pruning before concrete evaluation to reduce the number of concrete evaluations?
    //  - This predicate grouping also doesn't handle the case where a query returns too many entries.
    // TODO: how do we limit the amount of time the synthesizer spends?
    //  - limit the depth and the time together, limiting time is a little
    //   less invasive
    // TODO: how do we parallelize this nicely?
    //  - rayon, spawns / parallel iteration have a nice / reasonable API
    //
    // Idea: represent the expected table as its own bitvector relative to the examples.
    // This is done by evaluating the abstract query to a table, comparing the rows present in it
    // to the rows present in the expected table, and setting the corresponding bits.
    //
    // We then group predicates by what bits in the intermediate table they set, mapping bit-vectors to
    // the predicates that yield them, and then we search for a predicate that gives the same bit-vector
    // as the output once we're done generating this large table. Optimization: don't store bit-vectors that
    // are subsets of the target intermediate table T_out: we only have conjunction so we can never use those to
    // produce the bitvector for T_out.
    //
    // We also want to pick a representative of each predicate class so that way we can evaluate queries
    // for a given bitvector if we have to (when do we have to?). To pick good representatives, we should
    // probably rank by some heuristic that captures complexity so we get the fastest possible evaluation.

    // First, evaluate the abstract query.
    let rows = crate::sql::eval_abstract(&query, conn)?;
    // Now, phrase the concrete table as a bitvector.
    let target_intermediate = target.to_intermediate(&rows);

    // TODO: how do we get the space of all fields? I'm assuming it can be passed in as a parameter here, but
    // I think it might depend on our abstract query?
    let base_predicates = base_preds(fields, constants);
    let mut prior_depth_predicates = vec![PredNode::True];
    let mut vec_to_preds = HashMap::new();
    vec_to_preds.insert(bv::bitvec![1; rows.values.len()], vec![PredNode::True]);
    for _depth in 1..max_depth {
        use itertools::Itertools;
        // NOTE: this form of predicate generation is maybe a little silly. What if we tried to find
        // predicates that, when AND-ed, make the right thing (for example)?

        // We don't do elimination here because it'll happen when we construct the new predicates anyways.
        let mut predicates: HashMap<_, _> = grow(&prior_depth_predicates, &base_predicates)
            .into_iter()
            .map(|pred| {
                let mut v = bv::bitvec![0; rows.values.len()];
                // TODO: here, make it possible to compute the new bitvectors w/o doing this computation
                // by doing bitwise ops. Right now we have to re-evaluate each time by testing the predicate,
                // which is pretty slow.
                v.iter_mut().enumerate().for_each(|(i, mut x)| {
                    let env = Environment::from_row(&rows, i);
                    *x = pred.eval(&env)
                });
                (pred, v)
            })
            // v must be a superset of the target rows since we don't have disjunction.
            .filter(|(_pred, v)| target_intermediate.rows.clone() & v == target_intermediate.rows)
            .group_by(|(_pred, v)| v.clone())
            .into_iter()
            .map(|(v, pairs)| {
                let mut pairs: Vec<_> = pairs.map(|(pred, _v)| pred).collect();
                // TODO: sort pairs by some metric for complexity before popping last.
                let rep = pairs
                    .pop()
                    .expect("to have a pair, group must be non-empty");
                (v, (rep, pairs))
            })
            .collect();

        // When we find a predicate that has the right rows, stop and return it.
        if let Some((rep, mut rest)) = predicates.remove(&target_intermediate.rows) {
            rest.push(rep);
            return Ok(rest);
        }

        // Lastly, if we haven't found matches at this depth, go to the next depth,
        // which can build on these predicates.
        let mut new_prior_depth_predicates = Vec::with_capacity(predicates.iter().len());
        for (v, (rep, rest)) in predicates.into_iter() {
            new_prior_depth_predicates.push(rep.clone());
            let e = vec_to_preds
                .entry(v)
                .or_insert_with(|| Vec::with_capacity(rest.len() + 1));
            e.extend(rest);
            e.push(rep);
        }

        prior_depth_predicates = new_prior_depth_predicates;
    }

    // Finding nothing doesn't indicate an error, but it does indicate that there might be no
    // solutions, or the user will have to try another depth.
    Ok(vec![])
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::stun::Environment;
    use crate::types::{ExprNode, Field, PredNode};
    use proptest::strategy::Strategy;
    use std::rc::Rc;

    use super::grow;

    fn field_name() -> impl Strategy<Value = ExprNode> {
        proptest::string::string_regex(".*").unwrap().prop_map(|s| {
            ExprNode::Field(Field {
                name: s,
                table: String::from("t"),
            })
        })
    }

    fn int() -> impl Strategy<Value = ExprNode> {
        proptest::num::isize::ANY.prop_map(|n| ExprNode::Int { value: n })
    }

    fn expr_node() -> impl Strategy<Value = ExprNode> {
        proptest::prop_oneof!(field_name(), int())
    }

    fn pred_node() -> impl Strategy<Value = PredNode> {
        use proptest::prelude::*;
        let leaf = prop_oneof!(
            Just(PredNode::True),
            (expr_node(), expr_node()).prop_map(|(left, right)| PredNode::Lt { left, right }),
            (expr_node(), expr_node()).prop_map(|(left, right)| PredNode::Eq { left, right }),
        );
        leaf.prop_recursive(8, 256, 10, |inner| {
            inner.clone().prop_flat_map(move |left| {
                inner.clone().prop_map(move |right| PredNode::And {
                    left: Box::new(left.clone()),
                    right: Box::new(right.clone()),
                })
            })
        })
    }
    proptest::proptest! {
        #[test]
        fn expr_heights_are_1(node in expr_node()) {
            proptest::prop_assert_eq!(node.height(), 1)
        }

        #[test]
        fn pred_heights_are_strictly_larger_than_children(pred in pred_node()) {
            match &pred {
                PredNode::Lt { left, right  } | PredNode::Eq { left, right }  => proptest::prop_assert!(left.height() < pred.height() && right.height() < pred.height() ),
                PredNode::And { left, right } => proptest::prop_assert!(left.height() < pred.height() && right.height() < pred.height() ),
                PredNode::True => proptest::prop_assert_eq!(pred.height(), 1),
            }
        }
    }

    #[test]
    fn base_preds_match() {
        // TODO: it's kinda weird here that we don't include the base predicates.
        let base_preds = super::base_preds(
            &[
                Field {
                    name: String::from("hello"),
                    table: String::from("t1"),
                },
                Field {
                    name: String::from("world"),
                    table: String::from("t2"),
                },
            ],
            &[1, -1],
        );
        insta::assert_debug_snapshot!(base_preds);
    }

    #[test]
    fn growing_preds_match() {
        let base_preds = [PredNode::Lt {
            left: ExprNode::Field(Field {
                table: String::from("t"),
                name: String::from("hello"),
            }),
            right: ExprNode::Field(Field {
                table: String::from("t"),
                name: String::from("world"),
            }),
        }];
        let grow_with = [PredNode::Lt {
            left: ExprNode::Int { value: 1 },
            right: ExprNode::Int { value: 2 },
        }];

        let grown = grow(&grow_with, &base_preds);
        insta::assert_debug_snapshot!(grown);
    }

    #[test]
    fn predicate_equality() {
        let environment = Environment(HashMap::from([
            (String::from("a"), 1),
            (String::from("b"), 2),
        ]));
        let node = PredNode::Eq {
            left: ExprNode::Field(Field {
                table: String::from("t"),
                name: String::from("a"),
            }),
            right: ExprNode::Int { value: 1 },
        };
        assert!(node.eval(&environment))
    }

    #[test]
    fn predicate_inequality() {
        let environment = Environment(HashMap::from([
            (String::from("a"), 1),
            (String::from("b"), 2),
        ]));
        let node = PredNode::Eq {
            left: ExprNode::Field(Field {
                table: String::from("t"),
                name: String::from("a"),
            }),
            right: ExprNode::Field(Field {
                table: String::from("t"),
                name: String::from("b"),
            }),
        };
        assert!(!node.eval(&environment))
    }

    #[test]
    fn predicate_comparison() {
        let environment = Environment(HashMap::from([
            (String::from("a"), 1),
            (String::from("b"), 2),
        ]));
        let node = PredNode::Lt {
            left: ExprNode::Field(Field {
                table: String::from("t"),
                name: String::from("a"),
            }),
            right: ExprNode::Field(Field {
                table: String::from("t"),
                name: String::from("b"),
            }),
        };
        assert!(node.eval(&environment))
    }

    // TODO: add an insta test that we find the right predicates for a pretty simple example
}
