use crate::types::{ExprNode, Field, PredNode, AST};
use bitvec::prelude as bv;
use itertools::Itertools;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PredicateEnumerationError {
    #[error("database operation failed")]
    DatabaseError(#[from] rusqlite::Error),
}

fn enum_primitive_pred(constants: &[isize], fields: &[Field]) -> Vec<PredNode> {
    fn exprs<'a>(
        constants: &'a [isize],
        fields: &'a [Field],
    ) -> impl Iterator<Item = ExprNode> + 'a + Clone {
        constants
            .iter()
            .map(|n| ExprNode::Int { value: *n })
            .chain(fields.iter().map(|f| ExprNode::Field(f.clone())))
    }
    exprs(constants, fields)
        .cartesian_product(exprs(constants, fields))
        .flat_map(|(p1, p2)| {
            std::iter::once(PredNode::Eq {
                left: p1.clone(),
                right: p2.clone(),
            })
            .chain(std::iter::once(PredNode::Lt {
                left: p1,
                right: p2,
            }))
            .chain(std::iter::once(PredNode::True))
        })
        .collect()
}

fn enum_compound_pred(predicates: &[PredNode]) -> impl Iterator<Item = PredNode> + '_ {
    predicates
        .iter()
        .cartesian_product(predicates.iter())
        .map(|(p1, p2)| PredNode::And {
            left: Box::new(p1.clone()),
            right: Box::new(p2.clone()),
        })
}

pub fn enum_and_group_predicates(
    q: &AST<()>,
    constants: &[isize],
    max_depth: usize,
    conn: &rusqlite::Connection,
) -> Result<HashMap<bv::BitVec, Vec<PredNode>>, PredicateEnumerationError> {
    let t = crate::sql::eval_abstract(q, conn)?;
    let fields = crate::bottomup::get_fields(q);
    let primitives = enum_primitive_pred(constants, &fields);
    let mut rep: HashMap<_, Vec<PredNode>> = HashMap::new();
    primitives.into_iter().for_each(|p| {
        let predicate_vector = crate::bvdfs::predicate_vector(&t, &p);
        rep.entry(predicate_vector).or_insert_with(Vec::new).push(p);
    });

    for _ in 1..max_depth {
        let representatives = rep
            .values()
            .map(|v| {
                v.first()
                    .expect("all vectors in rep must be non-empty")
                    .clone()
            })
            .collect::<Vec<_>>();
        enum_compound_pred(&representatives).for_each(|p| {
            let predicate_vector = crate::bvdfs::predicate_vector(&t, &p);
            rep.entry(predicate_vector).or_insert_with(Vec::new).push(p);
        });
    }

    for preds in rep.values() {
        println!("{}", preds.first().unwrap());
    }

    // TODO: sort the Vec by simplicity?
    Ok(rep)
}
