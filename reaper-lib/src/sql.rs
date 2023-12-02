use crate::types::*;
use rusqlite::{params, Connection, Error, Result};

pub fn create_table() -> Result<Connection, Error> {
    todo!()
}

// NOTE: can we make query a reference? maybe there's a reason we can't?
pub fn eval(query: ASTNode, connection: &Connection) -> Result<ConcTable, Error> {
    todo!()
}
