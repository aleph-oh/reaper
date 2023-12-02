use crate::types::*;
use rusqlite::{params, params_from_iter, Connection, Error, Result};
use std::rc::Rc;

pub fn create_table(example: Example) -> Result<Connection, Error> {
    let conn = Connection::open_in_memory()?;

    for table in example.0.iter() {
        // Create table
        let mut create_table = String::from("CREATE TABLE ");
        create_table.push_str(&table.name);
        create_table.push_str(" (");
        for (i, field) in table.columns.iter().enumerate() {
            create_table.push_str(&field);
            create_table.push_str(" INTEGER");
            if i != table.columns.len() - 1 {
                create_table.push_str(", ");
            }
        }
        create_table.push_str(");");
        print!("{}", create_table);
        conn.execute(&create_table, params![])?;

        // Insert values
        let mut insert = String::from("INSERT INTO ");
        insert.push_str(&table.name);
        insert.push_str(" VALUES (");
        for i in 0..table.values.len() {
            if i == table.values.len() - 1 {
                insert.push_str(format!("?{}", i + 1).as_str());
            } else {
                insert.push_str(format!("?{}, ", i + 1).as_str());
            }
        }
        insert.push_str(");");

        print!("{}", insert);
        for row in table.values.iter() {
            conn.execute(&insert, params_from_iter(row.iter()))?;
        }
    }
    Ok(conn)
}

// NOTE: can we make query a reference? maybe there's a reason we can't?
pub fn eval(query: ASTNode, conn: &Connection) -> Result<ConcTable, Error> {
    // query_str = create_sql_query(query);
    // let mut stmt = conn.prepare(query_str)?;
    todo!();
}

fn create_fields_str(fields: Option<Vec<Field>>) -> String {
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

fn create_sql_query(query: ASTNode) -> String {
    let mut sql = String::from("(");
    match query {
        ASTNode::Select {
            fields: fields,
            table,
            pred,
        } => {
            sql.push_str("SELECT ");
            sql.push_str(&create_fields_str(fields));
            sql.push_str(" FROM ");
            sql.push_str(&create_sql_query((*table).clone()));
            sql.push_str(" WHERE ");
            sql.push_str(&create_sql_pred(pred));
        }
        ASTNode::Join {
            fields: fields,
            table1,
            table2,
            pred,
        } => {
            sql.push_str("SELECT ");
            sql.push_str(&create_fields_str(fields));
            sql.push_str(" FROM ");
            sql.push_str(&create_sql_query((*table1).clone()));
            sql.push_str(" JOIN ");
            sql.push_str(&create_sql_query((*table2).clone()));
            sql.push_str(" ON ");
            sql.push_str(&create_sql_pred(pred));
        }
        ASTNode::Table { name } => sql.push_str(&name),
        ASTNode::Concat { table1, table2 } => {
            sql.push_str(&create_sql_query((*table1).clone()));
            sql.push_str(", ");
            sql.push_str(&create_sql_query((*table2).clone()));
        }
    }
    sql.push_str(")");
    sql
}

fn create_sql_pred(pred: PredNode) -> String {
    match pred {
        PredNode::True => String::from("1"),
        PredNode::Lt { left, right } => {
            let mut sql = String::from("(");
            sql.push_str(&create_sql_expr(left));
            sql.push_str(" < ");
            sql.push_str(&create_sql_expr(right));
            sql.push_str(")");
            sql
        }
        PredNode::Eq { left, right } => {
            let mut sql = String::from("(");
            sql.push_str(&create_sql_expr(left));
            sql.push_str(" = ");
            sql.push_str(&create_sql_expr(right));
            sql.push_str(")");
            sql
        }
        PredNode::And { left, right } => {
            let mut sql = String::from("(");
            sql.push_str(&create_sql_pred(*left));
            sql.push_str(" AND ");
            sql.push_str(&create_sql_pred(*right));
            sql.push_str(")");
            sql
        }
    }
}

fn create_sql_expr(expr: ExprNode) -> String {
    match expr {
        ExprNode::FieldName { name } => format!("({})", name),
        ExprNode::Int { value } => format!("({})", value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table() {
        let example_input = vec![
            ConcTable {
                name: String::from("t1"),
                columns: vec![String::from("a"), String::from("b")],
                values: vec![vec![1, 2], vec![3, 4]],
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

        let conn = create_table((example_input, expected_output)).unwrap();
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
        let query = ASTNode::Select {
            fields: None,
            table: Rc::new(ASTNode::Table {
                name: String::from("t1"),
            }),
            pred: PredNode::True,
        };

        let expected = String::from("SELECT * FROM t1 WHERE true");
    }

    #[test]
    fn test_create_large_sql_query() {
        let query = ASTNode::Join {
            fields: None,
            table1: Rc::new(ASTNode::Select {
                fields: Some(vec![
                    Field {
                        name: String::from("id"),
                        table: String::from("users"),
                    },
                    Field {
                        name: String::from("role_id"),
                        table: String::from("users"),
                    },
                ]),
                table: Rc::new(ASTNode::Table {
                    name: String::from("users"),
                }),
                pred: PredNode::And {
                    left: Box::new(PredNode::Lt {
                        left: ExprNode::FieldName {
                            name: String::from("id"),
                        },
                        right: ExprNode::Int { value: 10 },
                    }),
                    right: Box::new(PredNode::Eq {
                        left: ExprNode::FieldName {
                            name: String::from("role_id"),
                        },
                        right: ExprNode::Int { value: 1 },
                    }),
                },
            }),
            table2: Rc::new(ASTNode::Select {
                fields: Some(vec![
                    Field {
                        name: String::from("id"),
                        table: String::from("users"),
                    },
                    Field {
                        name: String::from("role_id"),
                        table: String::from("users"),
                    },
                ]),
                table: Rc::new(ASTNode::Table {
                    name: String::from("users"),
                }),
                pred: PredNode::And {
                    left: Box::new(PredNode::Lt {
                        left: ExprNode::FieldName {
                            name: String::from("id"),
                        },
                        right: ExprNode::Int { value: 10 },
                    }),
                    right: Box::new(PredNode::Eq {
                        left: ExprNode::FieldName {
                            name: String::from("role_id"),
                        },
                        right: ExprNode::Int { value: 2 },
                    }),
                },
            }),
            pred: PredNode::Eq {
                left: ExprNode::FieldName {
                    name: String::from("users.id"),
                },
                right: ExprNode::FieldName {
                    name: String::from("users.id"),
                },
            },
        };

        let expected = String::from("(SELECT * FROM (SELECT id, role_id FROM (users) WHERE (((id) < (10)) AND ((role_id) = (1)))) JOIN (SELECT id, role_id FROM (users) WHERE (((id) < (10)) AND ((role_id) = (2)))) ON ((users.id) = (users.id)))");
        let actual = create_sql_query(query);
        assert_eq!(actual, expected);
    }
}
