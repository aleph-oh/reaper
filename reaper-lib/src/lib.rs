use bottomup::{generate_abstract_queries, get_fields};
use thiserror::Error;

pub mod bottomup;
pub mod sql;
pub mod stun;
pub mod types;

fn query_rank(q: &types::ASTNode) -> isize {
    // TODO: implement this!
    todo!()
}

#[derive(Error, Debug)]
pub enum SynthesisError {
    #[error("failed to synthesize predicate")]
    Predicate(#[from] stun::PredicateSynthesisError),
    #[error("failed to find a satisfying query")]
    NoQueriesFound,
}

pub fn synthesize(
    input: Vec<types::ConcTable>,
    output: types::ConcTable,
    constants: Vec<isize>,
) -> Result<types::ASTNode, SynthesisError> {
    let conn = sql::create_table(&input).expect("Failed to create table");
    let queries = generate_abstract_queries((input, output.clone()), 3, &conn);
    let concrete_queries: Vec<types::ASTNode> = queries
        .iter()
        .flat_map(|q| {
            let fields = get_fields(q);
            let predicates = stun::synthesize_pred(q, &output, &conn, &fields, &constants, 5);
            match predicates {
                Err(e) => itertools::Either::Left(std::iter::once(Err(e))),
                Ok(predicates) => {
                    let q = stun::AbstractQuery::try_from(q).unwrap();
                    itertools::Either::Right(
                        predicates.into_iter().map(move |p| Ok(q.with_predicate(p))),
                    )
                }
            }
        })
        .collect::<Result<_, _>>()?;
    match concrete_queries.into_iter().max_by_key(query_rank) {
        None => Err(SynthesisError::NoQueriesFound),
        Some(q) => Ok(q),
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
