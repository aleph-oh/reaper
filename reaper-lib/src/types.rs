use std::rc::Rc;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub table: String,
}
type Fields = Vec<Field>;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum AST<T> {
    // TODO: since this is generic maybe they should just be Box again
    Select {
        /// NOTE: we should think about making this Rc. We don't want to
        /// deeply clone the entire Vec<Field>.
        fields: Option<Fields>,
        table: Rc<AST<T>>,
        pred: T,
    },
    Join {
        fields: Option<Fields>,
        table1: Rc<AST<T>>,
        table2: Rc<AST<T>>,
        pred: T,
    }, // Note: Rel, Rel can be expressed as select _ from _, _ where True
    Table {
        name: String,
        columns: Vec<String>,
    },
    Concat {
        table1: Rc<AST<T>>,
        table2: Rc<AST<T>>,
    },
}

impl AST<PredNode> {
    pub(crate) fn height(&self) -> usize {
        match self {
            AST::Select {
                fields: _,
                table,
                pred,
            } => table.height().max(pred.height()),
            AST::Join {
                fields: _,
                table1,
                table2,
                pred,
            } => table1.height().max(table2.height()).max(pred.height()),
            AST::Table {
                name: _,
                columns: _,
            } => 1,
            AST::Concat { table1, table2 } => table1.height().max(table2.height()),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ExprNode {
    Field(Field),
    Int { value: isize },
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum PredNode {
    True,
    Lt {
        left: ExprNode,
        right: ExprNode,
    },
    Eq {
        left: ExprNode,
        right: ExprNode,
    },
    And {
        left: Box<PredNode>,
        right: Box<PredNode>,
    },
}

impl ExprNode {
    pub(crate) fn height(&self) -> usize {
        match self {
            ExprNode::Field(_) => 1,
            ExprNode::Int { value: _ } => 1,
        }
    }
}

impl PredNode {
    pub(crate) fn height(&self) -> usize {
        match self {
            PredNode::True => 1,
            PredNode::Lt { left, right } | PredNode::Eq { left, right } => {
                left.height().max(right.height()) + 1
            }
            PredNode::And { left, right } => left.height().max(right.height()) + 1,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ConcTable {
    pub name: String,
    pub columns: Vec<String>,
    pub values: Vec<Vec<isize>>,
}

pub type Example = (Vec<ConcTable>, ConcTable);
