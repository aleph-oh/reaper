use rusqlite::Connection;

use crate::sql::*;
use crate::types::*;
use std::collections::HashMap;
use std::rc::Rc;

// Get fields from any ASTNode
pub fn get_fields(node: &AST<()>) -> Vec<Field> {
    match node {
        AST::Select { fields, table, .. } => {
            let mut fields = match fields {
                Some(fields) => fields.to_vec(),
                None => get_fields(table),
            };
            fields.sort_by(|a, b| a.name.cmp(&b.name));
            fields
        }
        AST::Join {
            fields,
            table1,
            table2,
            ..
        } => {
            let mut fields = match fields {
                Some(fields) => fields.to_vec(),
                None => {
                    let mut fields1 = get_fields(table1);
                    let mut fields2 = get_fields(table2);
                    fields1.append(&mut fields2);
                    fields1
                }
            };
            fields.sort_by(|a, b| a.name.cmp(&b.name));
            fields
        }
        AST::Table { name, columns } => columns
            .iter()
            .map(|col| Field {
                name: col.clone(),
                table: name.clone(),
            })
            .collect::<Vec<_>>()
            .into(),
        AST::Concat { table1, table2 } => {
            let mut fields1 = get_fields(table1);
            let mut fields2 = get_fields(table2);
            fields1.append(&mut fields2);
            fields1.sort_by(|a, b| a.name.cmp(&b.name));
            fields1
        }
    }
}

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

fn powerset<T>(s: &[T]) -> Vec<Vec<T>>
where
    T: Clone,
{
    (1..2usize.pow(s.len() as u32))
        .map(|i| {
            s.iter()
                .enumerate()
                .filter(|&(t, _)| (i >> t) % 2 == 1)
                .map(|(_, element)| element.clone())
                .collect()
        })
        .collect()
}

// Return every combination of fields possible
// TODO: I don't actually want to be copying the fields everywhere...
fn field_combinations(query: &AST<()>) -> Vec<Vec<Field>> {
    let fields = get_fields(query);

    powerset(&fields)
}

fn field_combinations_join(query1: &AST<()>, query2: &AST<()>) -> Vec<Vec<Field>> {
    let fields1 = get_fields(query1);
    let fields2 = get_fields(query2);

    // Union the fields
    let mut fields = fields1;
    for field in fields2.iter() {
        if !fields.contains(field) {
            fields.push(field.clone());
        }
    }

    powerset(&fields)
}

fn grow(queries: Vec<AST<()>>) -> Vec<AST<()>> {
    let mut new_queries = Vec::new();

    for (_i, query) in queries.iter().enumerate() {
        // Identity
        new_queries.push(query.clone());

        // Select
        let field_powerset = field_combinations(query);
        for fields in field_powerset.iter() {
            let select = AST::Select {
                fields: Some(Rc::from(&fields[..])),
                table: Box::new(query.clone()),
                pred: (),
            };
            new_queries.push(select);
        }

        for (_j, query2) in queries.iter().enumerate() {
            // Join
            let field_powerset = field_combinations_join(query, query2);
            for fields in field_powerset.iter() {
                let join = AST::Join {
                    fields: Some(Rc::from(&fields[..])),
                    table1: Box::new(query.clone()),
                    table2: Box::new(query2.clone()),
                    pred: (),
                };
                new_queries.push(join);
            }

            // Concat
            let concat = AST::Concat {
                table1: Box::new(query.clone()),
                table2: Box::new(query2.clone()),
            };
            new_queries.push(concat);
        }
    }

    new_queries
}

fn elim(
    queries: Vec<AST<()>>,
    _example: &Example,
    conn: &Connection,
    is_final: bool,
) -> Vec<AST<()>> {
    // Map output to representative query
    let mut output_map = HashMap::new();

    for query in queries.iter() {
        let output = eval_abstract(query, conn);

        match output {
            Err(_) => continue,
            Ok(output) => {
                // TODO: equivalence occurs if the values are the same, regardless of ordering
                if !output_map.contains_key(&output) {
                    if (is_final) {
                        // Check that this is both a superset and the right structure
                        todo!();
                    }
                    output_map.insert(output.clone(), query.clone());
                }
                // TODO: heuristic for which query to keep
            }
        }
    }

    output_map.values().cloned().collect()
}

fn initial_set(example: &Example) -> Vec<AST<()>> {
    // Just return the set of all tables
    let mut queries = Vec::new();
    for table in example.0.iter() {
        queries.push(AST::Table {
            name: table.name.clone(),
            columns: table.columns.clone(),
        });
    }
    queries
}

pub fn generate_abstract_queries(example: Example, depth: i32, conn: &Connection) -> Vec<AST<()>> {
    let mut queries = initial_set(&example);

    for d in 0..depth {
        queries = grow(queries);
        println!("After grow");
        for query in queries.iter() {
            println!("{:?}", query);
        }
        queries = elim(queries, &example, conn, d == depth - 1);

        for query in queries.iter() {
            println!("After grow and elim");
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
