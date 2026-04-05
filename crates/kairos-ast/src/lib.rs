use std::fmt::{self, Display, Write as _};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub module: String,
    pub uses: Vec<String>,
    pub context: Option<ContextBlock>,
    pub schemas: Vec<SchemaDecl>,
    pub enums: Vec<EnumDecl>,
    pub type_aliases: Vec<TypeAliasDecl>,
    pub functions: Vec<FunctionDecl>,
    #[serde(skip_serializing, skip_deserializing, default)]
    pub source: String,
}

impl Program {
    #[must_use]
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = source.to_owned();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBlock {
    pub entries: Vec<ContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextEntry {
    pub key: String,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDecl {
    pub name: String,
    pub ty: TypeRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeAliasDecl {
    pub name: String,
    pub target: TypeRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
    pub metadata: Metadata,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub describe: Option<String>,
    pub tags: Vec<Expression>,
    pub requires: Vec<Expression>,
    pub ensures: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Statement {
    Let { name: String, value: Expression },
    Return { value: Expression },
    If(IfStatement),
    Expr { expression: Expression },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfStatement {
    pub condition: Expression,
    pub then_branch: Block,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElseBranch {
    Block(Block),
    If(Box<IfStatement>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expression {
    Literal { value: Literal },
    Identifier { name: String },
    Call { callee: String, args: Vec<Expression> },
    List { items: Vec<Expression> },
    Object { fields: Vec<ObjectField> },
    Binary { operator: BinaryOperator, left: Box<Expression>, right: Box<Expression> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectField {
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Literal {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRef {
    pub name: String,
    pub arguments: Vec<TypeRef>,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    And,
    Or,
}

impl BinaryOperator {
    #[must_use]
    pub fn precedence(self) -> u8 {
        match self {
            Self::Or => 1,
            Self::And => 2,
            Self::Equal | Self::NotEqual => 3,
            Self::Greater | Self::GreaterEqual | Self::Less | Self::LessEqual => 4,
            Self::Add | Self::Subtract => 5,
            Self::Multiply | Self::Divide => 6,
        }
    }
}

impl Display for TypeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if !self.arguments.is_empty() {
            f.write_char('<')?;
            for (index, argument) in self.arguments.iter().enumerate() {
                if index > 0 {
                    f.write_str(", ")?;
                }
                Display::fmt(argument, f)?;
            }
            f.write_char('>')?;
        }
        if self.optional {
            f.write_char('?')?;
        }
        Ok(())
    }
}

impl Display for BinaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::And => "&&",
            Self::Or => "||",
        };
        f.write_str(symbol)
    }
}

#[must_use]
pub fn format_expression(expression: &Expression) -> String {
    fn write_expression(
        expression: &Expression,
        parent_precedence: u8,
        out: &mut String,
    ) -> fmt::Result {
        match expression {
            Expression::Literal { value } => write_literal(value, out),
            Expression::Identifier { name } => out.write_str(name),
            Expression::Call { callee, args } => {
                out.write_str(callee)?;
                out.write_char('(')?;
                for (index, argument) in args.iter().enumerate() {
                    if index > 0 {
                        out.write_str(", ")?;
                    }
                    write_expression(argument, 0, out)?;
                }
                out.write_char(')')
            }
            Expression::List { items } => {
                out.write_char('[')?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.write_str(", ")?;
                    }
                    write_expression(item, 0, out)?;
                }
                out.write_char(']')
            }
            Expression::Object { fields } => {
                out.write_char('{')?;
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        out.write_str(", ")?;
                    }
                    out.write_str(&field.name)?;
                    out.write_str(": ")?;
                    write_expression(&field.value, 0, out)?;
                }
                out.write_char('}')
            }
            Expression::Binary { operator, left, right } => {
                let needs_parens = operator.precedence() < parent_precedence;
                if needs_parens {
                    out.write_char('(')?;
                }

                write_expression(left, operator.precedence(), out)?;
                write!(out, " {operator} ")?;
                write_expression(right, operator.precedence() + 1, out)?;

                if needs_parens {
                    out.write_char(')')?;
                }
                Ok(())
            }
        }
    }

    fn write_literal(literal: &Literal, out: &mut String) -> fmt::Result {
        match literal {
            Literal::String(value) => {
                out.write_char('"')?;
                for ch in value.chars() {
                    match ch {
                        '\\' => out.write_str("\\\\")?,
                        '"' => out.write_str("\\\"")?,
                        '\n' => out.write_str("\\n")?,
                        '\r' => out.write_str("\\r")?,
                        '\t' => out.write_str("\\t")?,
                        other => out.write_char(other)?,
                    }
                }
                out.write_char('"')
            }
            Literal::Integer(value) => write!(out, "{value}"),
            Literal::Float(value) => {
                let mut rendered = value.to_string();
                if !rendered.contains('.') && !rendered.contains('e') && !rendered.contains('E') {
                    rendered.push_str(".0");
                }
                out.write_str(&rendered)
            }
            Literal::Boolean(value) => write!(out, "{value}"),
            Literal::Null => out.write_str("null"),
        }
    }

    let mut rendered = String::new();
    write_expression(expression, 0, &mut rendered).expect("writing to string cannot fail");
    rendered
}

#[cfg(test)]
mod tests {
    use super::{format_expression, BinaryOperator, Expression, Literal, ObjectField, TypeRef};

    #[test]
    fn renders_nested_expression_with_parentheses() {
        let expression = Expression::Binary {
            operator: BinaryOperator::Multiply,
            left: Box::new(Expression::Binary {
                operator: BinaryOperator::Add,
                left: Box::new(Expression::Literal { value: Literal::Integer(1) }),
                right: Box::new(Expression::Literal { value: Literal::Integer(2) }),
            }),
            right: Box::new(Expression::Call {
                callee: "len".to_string(),
                args: vec![Expression::List {
                    items: vec![
                        Expression::Literal { value: Literal::String("a".to_string()) },
                        Expression::Object {
                            fields: vec![ObjectField {
                                name: "ok".to_string(),
                                value: Expression::Literal { value: Literal::Boolean(true) },
                            }],
                        },
                    ],
                }],
            }),
        };

        assert_eq!(format_expression(&expression), "(1 + 2) * len([\"a\", {ok: true}])");
    }

    #[test]
    fn type_ref_display_includes_generics_and_optional_marker() {
        let ty = TypeRef {
            name: "List".to_string(),
            arguments: vec![TypeRef {
                name: "Str".to_string(),
                arguments: Vec::new(),
                optional: false,
            }],
            optional: true,
        };

        assert_eq!(ty.to_string(), "List<Str>?");
    }
}
