use crate::types::*;
use std::rc::Rc;

fn grow(queries: Vec<ASTNode>) -> Vec<ASTNode> {
    let mut new_queries = Vec::new();
    for (i, query) in queries.iter().enumerate() {
        // Identity
        new_queries.push(query.clone());

        // Select
        let select = ASTNode::Select {
            fields: todo!(),
            table: Rc::new(query.clone()),
            pred: PredNode::True,
        };
        new_queries.push(select);

        for (j, query2) in queries.iter().enumerate() {
            // Join
            let join = ASTNode::Join {
                table1: Rc::new(query.clone()),
                table2: Rc::new(query2.clone()),
                pred: PredNode::True,
            };
            new_queries.push(join);

            // Concat
            let concat = ASTNode::Concat {
                table1: Rc::new(query.clone()),
                table2: Rc::new(query2.clone()),
            };
            new_queries.push(concat);
        }
    }

    new_queries
}

fn elim(queries: Vec<ASTNode>) -> Vec<ASTNode> {
    todo!()
}

fn initial_set(examples: Examples) -> Vec<ASTNode> {
    // Just return the set of all tables
    let mut queries = Vec::new();
    for example in examples.iter() {
        for (input, _) in example.iter() {
            queries.push(ASTNode::Table {
                name: input.name.clone(),
            });
        }
    }
    queries
}

pub fn generate_abstract_queries(examples: Examples, depth: i32) -> Vec<ASTNode> {
    let mut queries = initial_set(examples);

    // Remove equivalent queries and incorrect queries
    queries = elim(queries);

    for _ in 0..depth {
        queries = elim(queries);
        queries = grow(queries);
    }

    queries
}
