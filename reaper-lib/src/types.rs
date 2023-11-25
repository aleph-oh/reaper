#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Field {
    name: String,
    table: String,
}
type Fields = Vec<Field>;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ASTNode {
    Select {
        // NOTE: we should think about making this Rc. Cloning fields
        // whenever we substitute in a predicate will duplicate all the Vector
        // elements which we don't actually need.
        fields: Option<Fields>,
        table: Rc<ASTNode>,
        pred: PredNode,
    },
    Join {
        table1: Rc<ASTNode>,
        table2: Rc<ASTNode>,
        pred: PredNode,
    }, // Note: Rel, Rel can be expressed as select _ from _, _ where True
    Table {
        name: String,
    },
    Field {
        name: String,
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

pub struct ConcTable {
    pub columns: Vec<String>,
    pub values: Vec<Vec<isize>>,
}

pub type Examples<'a, 'b> = &'a [&'b [(ConcTable, ConcTable)]];
