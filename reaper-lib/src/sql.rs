use crate::types::*;
use rusqlite::{params, params_from_iter, Connection, Error, Result};

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
pub fn eval(query: ASTNode, connection: &Connection) -> Result<ConcTable, Error> {
    todo!()
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
}
