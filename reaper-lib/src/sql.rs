use crate::types::*;
use rusqlite::{params, params_from_iter, Connection, Error, Result};

pub fn create_table(input: &[ConcTable]) -> Result<Connection, Error> {
    let conn = Connection::open_in_memory()?;

    for table in input.iter() {
        // Create table
        let mut create_table = String::from("CREATE TABLE ");
        create_table.push_str(&table.name);
        create_table.push_str(" (");
        for (i, field) in table.columns.iter().enumerate() {
            create_table.push_str(field);
            create_table.push_str(" INTEGER");
            if i != table.columns.len() - 1 {
                create_table.push_str(", ");
            }
        }
        create_table.push_str(");");
        conn.execute(&create_table, params![])?;

        // Insert values
        let mut insert = String::from("INSERT INTO ");
        insert.push_str(&table.name);
        insert.push_str(" VALUES (");
        for i in 0..table.columns.len() {
            if i == table.columns.len() - 1 {
                insert.push_str(format!("?{}", i + 1).as_str());
            } else {
                insert.push_str(format!("?{}, ", i + 1).as_str());
            }
        }
        insert.push_str(");");

        for row in table.values.iter() {
            conn.execute(&insert, params_from_iter(row.iter()))?;
        }
    }
    Ok(conn)
}

fn make_dummy_table(query: &AST<PredNode>) -> ConcTable {
    match query {
        AST::Select { table, .. } => make_dummy_table(&table),
        AST::Join { table1, .. } => make_dummy_table(&table1),
        AST::Table { name, columns } => ConcTable {
            name: name.clone(),
            columns: columns.clone(),
            values: Vec::new(),
        },
        AST::Concat { table1, .. } => make_dummy_table(&table1),
    }
}

pub fn eval_abstract(query: &AST<()>, conn: &Connection) -> Result<ConcTable, Error> {
    let query = query
        .with_predicates(&vec![PredNode::True; query.num_holes()])
        .expect("expected hole count to match");

    let res = eval(&query, conn);

    match res {
        Ok(table) => Ok(table),
        Err(_) => Ok(make_dummy_table(&query)),
    }
}

// NOTE: can we make query a reference? maybe there's a reason we can't?
pub fn eval(query: &AST<PredNode>, conn: &Connection) -> Result<ConcTable, Error> {
    let mut table = ConcTable {
        name: String::from(""),
        columns: Vec::new(),
        values: Vec::new(),
    };

    let query_str = create_sql_query(query);

    // TODO: there should be a better way of doing this, but remove the paren
    // at the beginning and end of the query string
    let query_str = &query_str[1..query_str.len() - 1];
    let mut stmt = conn.prepare(query_str)?;

    for column in stmt.column_names().iter() {
        table.columns.push(String::from(*column));
    }

    let mut rows = stmt.query(params![])?;
    while let Some(row) = rows.next()? {
        let mut row_vec = Vec::new();
        for i in 0..table.columns.len() {
            row_vec.push(row.get::<_, isize>(i)?);
        }
        table.values.push(row_vec);
    }
    Ok(table)
}

fn create_fields_str(fields: Option<&[Field]>) -> String {
    match fields {
        Some(fields) => {
            let mut sql = String::from("");
            for (i, field) in fields.iter().enumerate() {
                sql.push_str(&field.name);
                if i != fields.len() - 1 {
                    sql.push_str(", ");
                }
            }
            sql
        }
        None => String::from("*"),
    }
}

pub fn create_sql_query(query: &AST<PredNode>) -> String {
    match query {
        AST::Select {
            fields,
            table,
            pred,
        } => {
            format!(
                "(SELECT {} FROM {} WHERE {})",
                create_fields_str(fields.as_ref().map(|t| &t[..])),
                create_sql_query(table),
                create_sql_pred(pred)
            )
        }
        AST::Join {
            fields,
            table1,
            table2,
            pred,
        } => {
            format!(
                "(SELECT {} FROM {} JOIN {} ON {})",
                create_fields_str(fields.as_ref().map(|t| &t[..])),
                create_sql_query(table1),
                create_sql_query(table2),
                create_sql_pred(pred)
            )
        }
        AST::Table { name, columns: _ } => format!("({})", name),
        AST::Concat { table1, table2 } => {
            format!(
                "({}, {})",
                create_sql_query(table1),
                create_sql_query(table2)
            )
        }
    }
}

fn create_sql_pred(pred: &PredNode) -> String {
    match pred {
        PredNode::True => String::from("1"),
        PredNode::Lt { left, right } => {
            let mut sql = String::from("(");
            sql.push_str(&create_sql_expr(left));
            sql.push_str(" < ");
            sql.push_str(&create_sql_expr(right));
            sql.push(')');
            sql
        }
        PredNode::Eq { left, right } => {
            let mut sql = String::from("(");
            sql.push_str(&create_sql_expr(left));
            sql.push_str(" = ");
            sql.push_str(&create_sql_expr(right));
            sql.push(')');
            sql
        }
        PredNode::And { left, right } => {
            let mut sql = String::from("(");
            sql.push_str(&create_sql_pred(left));
            sql.push_str(" AND ");
            sql.push_str(&create_sql_pred(right));
            sql.push(')');
            sql
        }
    }
}

fn create_sql_expr(expr: &ExprNode) -> String {
    match expr {
        ExprNode::Field(field) => format!("({}.{})", field.table, field.name),
        ExprNode::Int { value } => format!("({})", value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Field;
    use std::rc::Rc;

    #[test]
    fn test_create_table() {
        let example_input = vec![
            ConcTable {
                name: String::from("t1"),
                columns: vec![String::from("a"), String::from("b")],
                values: vec![vec![1, 2], vec![3, 4], vec![5, 6]],
            },
            ConcTable {
                name: String::from("t2"),
                columns: vec![String::from("a"), String::from("b")],
                values: vec![vec![1, 2], vec![3, 4]],
            },
        ];

        let expected_output = ConcTable {
            name: String::from("t1"),
            columns: vec![String::from("a"), String::from("b")],
            values: vec![vec![1, 2], vec![3, 4]],
        };

        let conn = create_table(&example_input).unwrap();
        let mut stmt = conn.prepare("SELECT * FROM t1;").unwrap();
        let mut rows = stmt.query(params![]).unwrap();
        let row = rows.next().unwrap().unwrap();
        assert_eq!(row.get::<_, isize>(0), Ok(1));
        assert_eq!(row.get::<_, isize>(1), Ok(2));
        let row = rows.next().unwrap().unwrap();
        assert_eq!(row.get::<_, isize>(0), Ok(3));
        assert_eq!(row.get::<_, isize>(1), Ok(4));

        let mut stmt = conn.prepare("SELECT * FROM t2;").unwrap();
        let mut rows = stmt.query(params![]).unwrap();
        let row = rows.next().unwrap().unwrap();
        assert_eq!(row.get::<_, isize>(0), Ok(1));
        assert_eq!(row.get::<_, isize>(1), Ok(2));
        let row = rows.next().unwrap().unwrap();
        assert_eq!(row.get::<_, isize>(0), Ok(3));
        assert_eq!(row.get::<_, isize>(1), Ok(4));
    }

    #[test]
    fn test_create_basic_sql_query() {
        let query = AST::Select {
            fields: None,
            table: Box::new(AST::Table {
                name: String::from("t1"),
                columns: vec![String::from("a"), String::from("b")],
            }),
            pred: PredNode::True,
        };

        let expected = String::from("SELECT * FROM t1 WHERE true");
    }

    #[test]
    fn test_create_large_sql_query() {
        let query = AST::Join {
            fields: None,
            table1: Box::new(AST::Select {
                fields: Some(Rc::from(vec![
                    Field {
                        name: String::from("id"),
                        table: String::from("users"),
                    },
                    Field {
                        name: String::from("role_id"),
                        table: String::from("users"),
                    },
                ])),
                table: Box::new(AST::Table {
                    name: String::from("users"),
                    columns: vec![String::from("id"), String::from("role_id")],
                }),
                pred: PredNode::And {
                    left: Box::new(PredNode::Lt {
                        left: ExprNode::Field(Field {
                            table: String::from("users"),
                            name: String::from("id"),
                        }),
                        right: ExprNode::Int { value: 10 },
                    }),
                    right: Box::new(PredNode::Eq {
                        left: ExprNode::Field(Field {
                            table: String::from("users"),
                            name: String::from("role_id"),
                        }),
                        right: ExprNode::Int { value: 1 },
                    }),
                },
            }),
            table2: Box::new(AST::Select {
                fields: Some(Rc::from(vec![
                    Field {
                        name: String::from("id"),
                        table: String::from("users"),
                    },
                    Field {
                        name: String::from("role_id"),
                        table: String::from("users"),
                    },
                ])),
                table: Box::new(AST::Table {
                    name: String::from("users"),
                    columns: vec![String::from("id"), String::from("role_id")],
                }),
                pred: PredNode::And {
                    left: Box::new(PredNode::Lt {
                        left: ExprNode::Field(Field {
                            table: String::from("users"),
                            name: String::from("id"),
                        }),
                        right: ExprNode::Int { value: 10 },
                    }),
                    right: Box::new(PredNode::Eq {
                        left: ExprNode::Field(Field {
                            table: String::from("users"),
                            name: String::from("role_id"),
                        }),
                        right: ExprNode::Int { value: 2 },
                    }),
                },
            }),
            pred: PredNode::Eq {
                left: ExprNode::Field(Field {
                    table: String::from("users"),
                    name: String::from("id"),
                }),
                right: ExprNode::Field(Field {
                    table: String::from("users"),
                    name: String::from("id"),
                }),
            },
        };

        insta::assert_debug_snapshot!(create_sql_query(&query), @r###""(SELECT * FROM (SELECT id, role_id FROM (users) WHERE (((users.id) < (10)) AND ((users.role_id) = (1)))) JOIN (SELECT id, role_id FROM (users) WHERE (((users.id) < (10)) AND ((users.role_id) = (2)))) ON ((users.id) = (users.id)))""###);
    }

    #[test]
    fn test_eval() {
        let example_input = vec![
            ConcTable {
                name: String::from("t1"),
                columns: vec![String::from("a"), String::from("b")],
                values: vec![vec![1, 2], vec![3, 4]],
            },
            ConcTable {
                name: String::from("t2"),
                columns: vec![String::from("a"), String::from("b")],
                values: vec![vec![1, 2], vec![5, 6]],
            },
        ];

        let expected_output = ConcTable {
            name: String::from(""),
            columns: vec![String::from("a"), String::from("b")],
            values: vec![vec![1, 2], vec![3, 4]],
        };

        let query = AST::Select {
            fields: None,
            table: Box::new(AST::Table {
                name: String::from("t1"),
                columns: vec![String::from("a"), String::from("b")],
            }),
            pred: PredNode::True,
        };

        let conn = create_table(&example_input).unwrap();
        let table = eval(&query, &conn).unwrap();
        assert_eq!(table, expected_output);
    }
}
