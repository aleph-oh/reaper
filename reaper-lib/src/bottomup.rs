use rusqlite::Connection;

use crate::sql::*;
use crate::types::*;
use std::collections::HashMap;
use std::rc::Rc;

fn is_superset(result: &ConcTable, expected: &ConcTable) -> bool {
    // Check that the result contains all the columns of the expected
    for col in expected.columns.iter() {
        if !result.columns.contains(col) {
            return false;
        }
    }

    // Check that the result contains all the rows of the expected
    for row in expected.values.iter() {
        if !result.values.contains(row) {
            return false;
        }
    }

    true
}

fn field_combinations()

fn grow(queries: Vec<ASTNode>) -> Vec<ASTNode> {
    let mut new_queries = Vec::new();

    // Get every combination of fields


    for (i, query) in queries.iter().enumerate() {
        // Identity
        new_queries.push(query.clone());

        // Select
        let select = ASTNode::Select {
            fields: None, // TODO: this should be a subset of the fields of the table
            table: Rc::new(query.clone()),
            pred: PredNode::True,
        };
        new_queries.push(select);

        for (j, query2) in queries.iter().enumerate() {
            // Join
            let join = ASTNode::Join {
                fields: None, // TODO: this should be a subset of the fields of the table
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

fn elim(queries: Vec<ASTNode>, example: &Example, conn: &Connection) -> Vec<ASTNode> {
    // Map output to representative query
    let mut output_map = HashMap::new();

    for query in queries.iter() {
        let output = eval(query, conn);

        match output {
            Err(_) => continue,
            Ok(output) => {
                // TODO: equivalence occurs if the values are the same, regardless of ordering
                if !output_map.contains_key(&output) {
                  // Check that this is a superset of the expected output
                  if !is_superset(&output, &example.1) {
                      continue;
                  }
                  output_map.insert(output.clone(), query.clone());
                }
                // TODO: heuristic for which query to keep
            }
        } 
    }

    output_map.values().cloned().collect()
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

pub fn generate_abstract_queries(example: Example, depth: i32, conn: &Connection) -> Vec<ASTNode> {
    let mut queries = initial_set(&example);

    for _ in 0..3 {
        queries = grow(queries);
        println!("{}", "After grow");
        for query in queries.iter() {
            println!("{:?}", query);
        }
        queries = elim(queries, &example, conn);

        for query in queries.iter() {
            println!("{}", "After grow and elim");
            println!("{:?}", query);
        }
    }

    queries
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_queries_simple() {
        let input = vec![ConcTable {
            name: "t1".to_string(),
            columns: vec!["a".to_string(), "b".to_string()],
            values: vec![vec![1, 2], vec![3, 4]],
        }];
        let output = ConcTable {
            name: "".to_string(),
            columns: vec!["".to_string(), "".to_string()],
            values: vec![vec![1, 2], vec![3, 4]],
        };

        let conn = create_table(&input).unwrap();
        let queries = generate_abstract_queries((input, output), 2, &conn);
        for query in queries.iter() {
            println!("{:?}", query);
        }

        assert!(queries.len() > 0);
    }
}