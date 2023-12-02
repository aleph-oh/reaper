use crate::types::*;
use std::collections::HashMap;
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

fn interpret(query: &ASTNode, example: &Example) -> Vec<ConcTable> {
    todo!()
}

fn elim(queries: Vec<ASTNode>, example: &Example) -> Vec<ASTNode> {
    todo!()
}

fn initial_set(example: &Example) -> Vec<ASTNode> {
    // Just return the set of all tables
    let mut queries = Vec::new();
    for table in example.0.iter() {
        queries.push(ASTNode::Table {
            name: table.name.clone(),
        });
    }
    queries
}

pub fn generate_abstract_queries(example: Example, depth: i32) -> Vec<ASTNode> {
    let mut queries = initial_set(&example);

    // Remove equivalent queries and incorrect queries
    queries = elim(queries, &example);

    for _ in 0..depth {
        queries = elim(queries, &example);
        queries = grow(queries);
    }

    queries
}
