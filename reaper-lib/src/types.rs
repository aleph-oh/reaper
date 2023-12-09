use std::rc::Rc;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub table: String,
}
type Fields = Vec<Field>;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ASTNode {
    Select {
        /// NOTE: we should think about making this Rc. We don't want to
        /// deeply clone the entire Vec<Field>.
        fields: Option<Fields>,
        table: Rc<ASTNode>,
        pred: PredNode,
    },
    Join {
        fields: Option<Fields>,
        table1: Rc<ASTNode>,
        table2: Rc<ASTNode>,
        pred: PredNode,
    }, // Note: Rel, Rel can be expressed as select _ from _, _ where True
    Table {
        name: String,
    },
    Concat {
        table1: Rc<ASTNode>,
        table2: Rc<ASTNode>,
    },
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ExprNode {
    FieldName { name: String },
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

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ConcTable {
    pub name: String,
    pub columns: Vec<String>,
    pub values: Vec<Vec<isize>>,
}

pub type Example = (Vec<ConcTable>, ConcTable);
