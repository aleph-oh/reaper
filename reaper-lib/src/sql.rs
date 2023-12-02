use crate::types::*;
use rusqlite::{params, Connection, Result, Error};

pub fn create_table() -> Result<Connection, Error> {
    todo!()
}

pub fn eval(query: ASTNode, connection: &Connection) -> Result<Vec<ConcTable>, Error> {
    todo!()
}