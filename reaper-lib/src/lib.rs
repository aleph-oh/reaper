use thiserror::Error;

pub mod bottomup;
pub mod bvdfs;
pub mod sql;
pub mod stun;
pub mod types;

fn query_rank(_q: &types::AST<types::PredNode>) -> isize {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 4;
        assert_eq!(result, 4);
    }
}
