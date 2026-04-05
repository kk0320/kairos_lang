use std::fmt;

use kairos_ast::{
    BinaryOperator, Block, ContextBlock, ContextEntry, ElseBranch, EnumDecl, Expression, FieldDecl,
    FunctionDecl, IfStatement, Literal, Metadata, ObjectField, Param, Program, SchemaDecl,
    Statement, TypeAliasDecl, TypeRef,
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Module,
    Use,
    Context,
    Schema,
    Enum,
    Type,
    Fn,
    Describe,
    Tags,
    Requires,
    Ensures,
    Let,
    Return,
    If,
    Else,
    True,
    False,
    Null,
    Identifier(String),
    String(String),
    Integer(String),
    Float(String),
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Arrow,
    Question,
    Equal,
    DoubleEqual,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    AndAnd,
    OrOr,
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message} at line {line}, column {column}")]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl ParseError {
    fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self { message: message.into(), line, column }
    }
}

pub fn lex_source(source: &str) -> Result<Vec<Token>> {
    Lexer::new(source).lex()
}

pub fn parse_source(source: &str) -> Result<Program> {
    let tokens = lex_source(source)?;
    Parser::new(tokens).parse_program().map(|program| program.with_source(source))
}

struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0, line: 1, column: 1 }
    }

    fn lex(mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.advance_char();
                continue;
            }

            if ch == '/' && self.peek_next_char() == Some('/') {
                self.skip_line_comment();
                continue;
            }

            let line = self.line;
            let column = self.column;
            let kind = match ch {
                '{' => {
                    self.advance_char();
                    TokenKind::LeftBrace
                }
                '}' => {
                    self.advance_char();
                    TokenKind::RightBrace
                }
                '(' => {
                    self.advance_char();
                    TokenKind::LeftParen
                }
                ')' => {
                    self.advance_char();
                    TokenKind::RightParen
                }
                '[' => {
                    self.advance_char();
                    TokenKind::LeftBracket
                }
                ']' => {
                    self.advance_char();
                    TokenKind::RightBracket
                }
                ',' => {
                    self.advance_char();
                    TokenKind::Comma
                }
                ':' => {
                    self.advance_char();
                    TokenKind::Colon
                }
                ';' => {
                    self.advance_char();
                    TokenKind::Semicolon
                }
                '.' => {
                    self.advance_char();
                    TokenKind::Dot
                }
                '?' => {
                    self.advance_char();
                    TokenKind::Question
                }
                '+' => {
                    self.advance_char();
                    TokenKind::Plus
                }
                '*' => {
                    self.advance_char();
                    TokenKind::Star
                }
                '/' => {
                    self.advance_char();
                    TokenKind::Slash
                }
                '-' => {
                    self.advance_char();
                    if self.peek_char() == Some('>') {
                        self.advance_char();
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    }
                }
                '=' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::DoubleEqual
                    } else {
                        TokenKind::Equal
                    }
                }
                '!' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::NotEqual
                    } else {
                        return Err(ParseError::new("unexpected `!`", line, column));
                    }
                }
                '>' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::GreaterEqual
                    } else {
                        TokenKind::Greater
                    }
                }
                '<' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::LessEqual
                    } else {
                        TokenKind::Less
                    }
                }
                '&' => {
                    self.advance_char();
                    if self.peek_char() == Some('&') {
                        self.advance_char();
                        TokenKind::AndAnd
                    } else {
                        return Err(ParseError::new("unexpected `&`", line, column));
                    }
                }
                '|' => {
                    self.advance_char();
                    if self.peek_char() == Some('|') {
                        self.advance_char();
                        TokenKind::OrOr
                    } else {
                        return Err(ParseError::new("unexpected `|`", line, column));
                    }
                }
                '"' => TokenKind::String(self.lex_string(line, column)?),
                value if is_identifier_start(value) => self.lex_identifier(),
                value if value.is_ascii_digit() => self.lex_number(),
                _ => {
                    return Err(ParseError::new(
                        format!("unexpected character `{ch}`"),
                        line,
                        column,
                    ));
                }
            };

            tokens.push(Token { kind, line, column });
        }

        tokens.push(Token { kind: TokenKind::Eof, line: self.line, column: self.column });

        Ok(tokens)
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut chars = self.source[self.offset..].chars();
        chars.next()?;
        chars.next()
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.offset += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.advance_char();
            if ch == '\n' {
                break;
            }
        }
    }

    fn lex_string(&mut self, line: usize, column: usize) -> Result<String> {
        self.advance_char();
        let mut value = String::new();

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.advance_char();
                    return Ok(value);
                }
                '\\' => {
                    self.advance_char();
                    let escaped = self.advance_char().ok_or_else(|| {
                        ParseError::new("unterminated string literal", line, column)
                    })?;
                    let decoded = match escaped {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        other => {
                            return Err(ParseError::new(
                                format!("unsupported escape `\\{other}`"),
                                self.line,
                                self.column.saturating_sub(1),
                            ))
                        }
                    };
                    value.push(decoded);
                }
                '\n' | '\r' => {
                    return Err(ParseError::new("unterminated string literal", line, column))
                }
                other => {
                    self.advance_char();
                    value.push(other);
                }
            }
        }

        Err(ParseError::new("unterminated string literal", line, column))
    }

    fn lex_identifier(&mut self) -> TokenKind {
        let mut text = String::new();
        while let Some(ch) = self.peek_char() {
            if !is_identifier_continue(ch) {
                break;
            }
            text.push(ch);
            self.advance_char();
        }

        match text.as_str() {
            "module" => TokenKind::Module,
            "use" => TokenKind::Use,
            "context" => TokenKind::Context,
            "schema" => TokenKind::Schema,
            "enum" => TokenKind::Enum,
            "type" => TokenKind::Type,
            "fn" => TokenKind::Fn,
            "describe" => TokenKind::Describe,
            "tags" => TokenKind::Tags,
            "requires" => TokenKind::Requires,
            "ensures" => TokenKind::Ensures,
            "let" => TokenKind::Let,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            _ => TokenKind::Identifier(text),
        }
    }

    fn lex_number(&mut self) -> TokenKind {
        let mut text = String::new();

        while let Some(ch) = self.peek_char() {
            if !ch.is_ascii_digit() {
                break;
            }
            text.push(ch);
            self.advance_char();
        }

        if self.peek_char() == Some('.')
            && self.peek_next_char().is_some_and(|value| value.is_ascii_digit())
        {
            text.push('.');
            self.advance_char();
            while let Some(ch) = self.peek_char() {
                if !ch.is_ascii_digit() {
                    break;
                }
                text.push(ch);
                self.advance_char();
            }
            TokenKind::Float(text)
        } else {
            TokenKind::Integer(text)
        }
    }
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn parse_program(&mut self) -> Result<Program> {
        let module = self.parse_module_decl()?;
        let mut program = Program { module, ..Program::default() };

        while !self.at(TokenMatcher::Eof) {
            if self.at(TokenMatcher::Use) {
                program.uses.push(self.parse_use_decl()?);
            } else if self.at(TokenMatcher::Context) {
                if program.context.is_some() {
                    return Err(self.error_here("duplicate `context` block"));
                }
                program.context = Some(self.parse_context_decl()?);
            } else if self.at(TokenMatcher::Schema) {
                program.schemas.push(self.parse_schema_decl()?);
            } else if self.at(TokenMatcher::Enum) {
                program.enums.push(self.parse_enum_decl()?);
            } else if self.at(TokenMatcher::Type) {
                program.type_aliases.push(self.parse_type_alias()?);
            } else if self.at(TokenMatcher::Fn) {
                program.functions.push(self.parse_function_decl()?);
            } else {
                return Err(self.error_here("expected a top-level declaration"));
            }
        }

        self.expect(TokenMatcher::Eof, "end of file")?;
        Ok(program)
    }

    fn parse_module_decl(&mut self) -> Result<String> {
        self.expect(TokenMatcher::Module, "`module`")?;
        let path = self.parse_path()?;
        self.expect(TokenMatcher::Semicolon, "`;`")?;
        Ok(path)
    }

    fn parse_use_decl(&mut self) -> Result<String> {
        self.expect(TokenMatcher::Use, "`use`")?;
        let path = self.parse_path()?;
        self.expect(TokenMatcher::Semicolon, "`;`")?;
        Ok(path)
    }

    fn parse_context_decl(&mut self) -> Result<ContextBlock> {
        self.expect(TokenMatcher::Context, "`context`")?;
        self.expect(TokenMatcher::LeftBrace, "`{`")?;
        let mut entries = Vec::new();

        while !self.at(TokenMatcher::RightBrace) {
            let key = self.expect_identifier()?;
            self.expect(TokenMatcher::Colon, "`:`")?;
            let value = self.parse_expression()?;
            self.expect(TokenMatcher::Semicolon, "`;`")?;
            entries.push(ContextEntry { key, value });
        }

        self.expect(TokenMatcher::RightBrace, "`}`")?;
        Ok(ContextBlock { entries })
    }

    fn parse_schema_decl(&mut self) -> Result<SchemaDecl> {
        self.expect(TokenMatcher::Schema, "`schema`")?;
        let name = self.expect_identifier()?;
        self.expect(TokenMatcher::LeftBrace, "`{`")?;
        let mut fields = Vec::new();

        while !self.at(TokenMatcher::RightBrace) {
            let field_name = self.expect_identifier()?;
            self.expect(TokenMatcher::Colon, "`:`")?;
            let ty = self.parse_type_ref()?;
            if self.at(TokenMatcher::Comma) {
                self.bump();
            } else if !self.at(TokenMatcher::RightBrace) {
                return Err(self.error_here("expected `,` or `}` after schema field"));
            }
            fields.push(FieldDecl { name: field_name, ty });
        }

        self.expect(TokenMatcher::RightBrace, "`}`")?;
        Ok(SchemaDecl { name, fields })
    }

    fn parse_enum_decl(&mut self) -> Result<EnumDecl> {
        self.expect(TokenMatcher::Enum, "`enum`")?;
        let name = self.expect_identifier()?;
        self.expect(TokenMatcher::LeftBrace, "`{`")?;
        let mut variants = Vec::new();

        while !self.at(TokenMatcher::RightBrace) {
            variants.push(self.expect_identifier()?);
            if self.at(TokenMatcher::Comma) {
                self.bump();
                if self.at(TokenMatcher::RightBrace) {
                    break;
                }
            } else if !self.at(TokenMatcher::RightBrace) {
                return Err(self.error_here("expected `,` or `}` after enum variant"));
            }
        }

        self.expect(TokenMatcher::RightBrace, "`}`")?;
        Ok(EnumDecl { name, variants })
    }

    fn parse_type_alias(&mut self) -> Result<TypeAliasDecl> {
        self.expect(TokenMatcher::Type, "`type`")?;
        let name = self.expect_identifier()?;
        self.expect(TokenMatcher::Equal, "`=`")?;
        let target = self.parse_type_ref()?;
        self.expect(TokenMatcher::Semicolon, "`;`")?;
        Ok(TypeAliasDecl { name, target })
    }

    fn parse_function_decl(&mut self) -> Result<FunctionDecl> {
        self.expect(TokenMatcher::Fn, "`fn`")?;
        let name = self.expect_identifier()?;
        self.expect(TokenMatcher::LeftParen, "`(`")?;
        let params = self.parse_params()?;
        self.expect(TokenMatcher::RightParen, "`)`")?;
        self.expect(TokenMatcher::Arrow, "`->`")?;
        let return_type = self.parse_type_ref()?;
        let metadata = self.parse_metadata()?;
        let body = self.parse_block()?;

        Ok(FunctionDecl { name, params, return_type, metadata, body })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        if self.at(TokenMatcher::RightParen) {
            return Ok(params);
        }

        loop {
            let name = self.expect_identifier()?;
            self.expect(TokenMatcher::Colon, "`:`")?;
            let ty = self.parse_type_ref()?;
            params.push(Param { name, ty });

            if self.at(TokenMatcher::Comma) {
                self.bump();
                continue;
            }
            break;
        }

        Ok(params)
    }

    fn parse_metadata(&mut self) -> Result<Metadata> {
        let mut metadata = Metadata::default();

        loop {
            if self.at(TokenMatcher::Describe) {
                self.bump();
                if metadata.describe.is_some() {
                    return Err(self.error_here("duplicate `describe` metadata"));
                }
                metadata.describe = Some(self.expect_string_literal()?);
            } else if self.at(TokenMatcher::Tags) {
                self.bump();
                if !metadata.tags.is_empty() {
                    return Err(self.error_here("duplicate `tags` metadata"));
                }
                metadata.tags = self.parse_list_items()?;
            } else if self.at(TokenMatcher::Requires) {
                self.bump();
                if !metadata.requires.is_empty() {
                    return Err(self.error_here("duplicate `requires` metadata"));
                }
                metadata.requires = self.parse_list_items()?;
            } else if self.at(TokenMatcher::Ensures) {
                self.bump();
                if !metadata.ensures.is_empty() {
                    return Err(self.error_here("duplicate `ensures` metadata"));
                }
                metadata.ensures = self.parse_list_items()?;
            } else {
                break;
            }
        }

        Ok(metadata)
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef> {
        let name = self.expect_identifier()?;
        let mut arguments = Vec::new();
        if self.at(TokenMatcher::Less) {
            self.bump();
            loop {
                arguments.push(self.parse_type_ref()?);
                if self.at(TokenMatcher::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
            self.expect(TokenMatcher::Greater, "`>`")?;
        }

        let optional = self.at(TokenMatcher::Question);
        if optional {
            self.bump();
        }

        Ok(TypeRef { name, arguments, optional })
    }

    fn parse_block(&mut self) -> Result<Block> {
        self.expect(TokenMatcher::LeftBrace, "`{`")?;
        let mut statements = Vec::new();
        while !self.at(TokenMatcher::RightBrace) {
            statements.push(self.parse_statement()?);
        }
        self.expect(TokenMatcher::RightBrace, "`}`")?;
        Ok(Block { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        if self.at(TokenMatcher::Let) {
            self.bump();
            let name = self.expect_identifier()?;
            self.expect(TokenMatcher::Equal, "`=`")?;
            let value = self.parse_expression()?;
            self.expect(TokenMatcher::Semicolon, "`;`")?;
            Ok(Statement::Let { name, value })
        } else if self.at(TokenMatcher::Return) {
            self.bump();
            let value = self.parse_expression()?;
            self.expect(TokenMatcher::Semicolon, "`;`")?;
            Ok(Statement::Return { value })
        } else if self.at(TokenMatcher::If) {
            self.parse_if_statement().map(Statement::If)
        } else {
            let expression = self.parse_expression()?;
            self.expect(TokenMatcher::Semicolon, "`;`")?;
            Ok(Statement::Expr { expression })
        }
    }

    fn parse_if_statement(&mut self) -> Result<IfStatement> {
        self.expect(TokenMatcher::If, "`if`")?;
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.at(TokenMatcher::Else) {
            self.bump();
            if self.at(TokenMatcher::If) {
                Some(ElseBranch::If(Box::new(self.parse_if_statement()?)))
            } else {
                Some(ElseBranch::Block(self.parse_block()?))
            }
        } else {
            None
        };

        Ok(IfStatement { condition, then_branch, else_branch })
    }

    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_or_expression()
    }

    fn parse_or_expression(&mut self) -> Result<Expression> {
        self.parse_binary_expression(Self::parse_and_expression, &[TokenMatcher::OrOr])
    }

    fn parse_and_expression(&mut self) -> Result<Expression> {
        self.parse_binary_expression(Self::parse_equality_expression, &[TokenMatcher::AndAnd])
    }

    fn parse_equality_expression(&mut self) -> Result<Expression> {
        self.parse_binary_expression(
            Self::parse_comparison_expression,
            &[TokenMatcher::DoubleEqual, TokenMatcher::NotEqual],
        )
    }

    fn parse_comparison_expression(&mut self) -> Result<Expression> {
        self.parse_binary_expression(
            Self::parse_term_expression,
            &[
                TokenMatcher::Greater,
                TokenMatcher::GreaterEqual,
                TokenMatcher::Less,
                TokenMatcher::LessEqual,
            ],
        )
    }

    fn parse_term_expression(&mut self) -> Result<Expression> {
        self.parse_binary_expression(
            Self::parse_factor_expression,
            &[TokenMatcher::Plus, TokenMatcher::Minus],
        )
    }

    fn parse_factor_expression(&mut self) -> Result<Expression> {
        self.parse_binary_expression(
            Self::parse_primary_expression,
            &[TokenMatcher::Star, TokenMatcher::Slash],
        )
    }

    fn parse_binary_expression(
        &mut self,
        parse_operand: fn(&mut Self) -> Result<Expression>,
        operators: &[TokenMatcher],
    ) -> Result<Expression> {
        let mut expression = parse_operand(self)?;

        while let Some(operator) = self.current_binary_operator(operators) {
            self.bump();
            let right = parse_operand(self)?;
            expression =
                Expression::Binary { operator, left: Box::new(expression), right: Box::new(right) };
        }

        Ok(expression)
    }

    fn parse_primary_expression(&mut self) -> Result<Expression> {
        if let Some(identifier) = self.consume_identifier() {
            if self.at(TokenMatcher::LeftParen) {
                self.bump();
                let mut args = Vec::new();
                if !self.at(TokenMatcher::RightParen) {
                    loop {
                        args.push(self.parse_expression()?);
                        if self.at(TokenMatcher::Comma) {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                self.expect(TokenMatcher::RightParen, "`)`")?;
                Ok(Expression::Call { callee: identifier, args })
            } else {
                Ok(Expression::Identifier { name: identifier })
            }
        } else if let Some(value) = self.consume_string() {
            Ok(Expression::Literal { value: Literal::String(value) })
        } else if let Some(value) = self.consume_integer()? {
            Ok(Expression::Literal { value: Literal::Integer(value) })
        } else if let Some(value) = self.consume_float()? {
            Ok(Expression::Literal { value: Literal::Float(value) })
        } else if self.at(TokenMatcher::True) {
            self.bump();
            Ok(Expression::Literal { value: Literal::Boolean(true) })
        } else if self.at(TokenMatcher::False) {
            self.bump();
            Ok(Expression::Literal { value: Literal::Boolean(false) })
        } else if self.at(TokenMatcher::Null) {
            self.bump();
            Ok(Expression::Literal { value: Literal::Null })
        } else if self.at(TokenMatcher::LeftBracket) {
            let items = self.parse_list_items()?;
            Ok(Expression::List { items })
        } else if self.at(TokenMatcher::LeftBrace) {
            self.parse_object_literal()
        } else if self.at(TokenMatcher::LeftParen) {
            self.bump();
            let expression = self.parse_expression()?;
            self.expect(TokenMatcher::RightParen, "`)`")?;
            Ok(expression)
        } else {
            Err(self.error_here("expected an expression"))
        }
    }

    fn parse_object_literal(&mut self) -> Result<Expression> {
        self.expect(TokenMatcher::LeftBrace, "`{`")?;
        let mut fields = Vec::new();

        while !self.at(TokenMatcher::RightBrace) {
            let name = self.expect_identifier()?;
            self.expect(TokenMatcher::Colon, "`:`")?;
            let value = self.parse_expression()?;
            fields.push(ObjectField { name, value });

            if self.at(TokenMatcher::Comma) {
                self.bump();
                if self.at(TokenMatcher::RightBrace) {
                    break;
                }
            } else if !self.at(TokenMatcher::RightBrace) {
                return Err(self.error_here("expected `,` or `}` after object field"));
            }
        }

        self.expect(TokenMatcher::RightBrace, "`}`")?;
        Ok(Expression::Object { fields })
    }

    fn parse_list_items(&mut self) -> Result<Vec<Expression>> {
        self.expect(TokenMatcher::LeftBracket, "`[`")?;
        let mut items = Vec::new();

        while !self.at(TokenMatcher::RightBracket) {
            items.push(self.parse_expression()?);
            if self.at(TokenMatcher::Comma) {
                self.bump();
                if self.at(TokenMatcher::RightBracket) {
                    break;
                }
            } else if !self.at(TokenMatcher::RightBracket) {
                return Err(self.error_here("expected `,` or `]` after list item"));
            }
        }

        self.expect(TokenMatcher::RightBracket, "`]`")?;
        Ok(items)
    }

    fn parse_path(&mut self) -> Result<String> {
        let mut segments = vec![self.expect_identifier()?];
        while self.at(TokenMatcher::Dot) {
            self.bump();
            segments.push(self.expect_identifier()?);
        }
        Ok(segments.join("."))
    }

    fn current_binary_operator(&self, operators: &[TokenMatcher]) -> Option<BinaryOperator> {
        let matcher = operators.iter().find(|matcher| self.at(**matcher))?;
        Some(match matcher {
            TokenMatcher::Plus => BinaryOperator::Add,
            TokenMatcher::Minus => BinaryOperator::Subtract,
            TokenMatcher::Star => BinaryOperator::Multiply,
            TokenMatcher::Slash => BinaryOperator::Divide,
            TokenMatcher::DoubleEqual => BinaryOperator::Equal,
            TokenMatcher::NotEqual => BinaryOperator::NotEqual,
            TokenMatcher::Greater => BinaryOperator::Greater,
            TokenMatcher::GreaterEqual => BinaryOperator::GreaterEqual,
            TokenMatcher::Less => BinaryOperator::Less,
            TokenMatcher::LessEqual => BinaryOperator::LessEqual,
            TokenMatcher::AndAnd => BinaryOperator::And,
            TokenMatcher::OrOr => BinaryOperator::Or,
            _ => return None,
        })
    }

    fn expect(&mut self, matcher: TokenMatcher, expected: &str) -> Result<Token> {
        if self.at(matcher) {
            Ok(self.bump())
        } else {
            Err(self.error_here(format!("expected {expected}")))
        }
    }

    fn expect_identifier(&mut self) -> Result<String> {
        self.consume_identifier().ok_or_else(|| self.error_here("expected an identifier"))
    }

    fn expect_string_literal(&mut self) -> Result<String> {
        self.consume_string().ok_or_else(|| self.error_here("expected a string literal"))
    }

    fn consume_identifier(&mut self) -> Option<String> {
        let token = self.current()?;
        if let TokenKind::Identifier(value) = &token.kind {
            let value = value.clone();
            self.bump();
            Some(value)
        } else {
            None
        }
    }

    fn consume_string(&mut self) -> Option<String> {
        let token = self.current()?;
        if let TokenKind::String(value) = &token.kind {
            let value = value.clone();
            self.bump();
            Some(value)
        } else {
            None
        }
    }

    fn consume_integer(&mut self) -> Result<Option<i64>> {
        let token = match self.current() {
            Some(token) => token.clone(),
            None => return Ok(None),
        };

        if let TokenKind::Integer(value) = token.kind {
            self.bump();
            let parsed = value.parse::<i64>().map_err(|_| {
                ParseError::new(
                    format!("invalid integer literal `{value}`"),
                    token.line,
                    token.column,
                )
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    fn consume_float(&mut self) -> Result<Option<f64>> {
        let token = match self.current() {
            Some(token) => token.clone(),
            None => return Ok(None),
        };

        if let TokenKind::Float(value) = token.kind {
            self.bump();
            let parsed = value.parse::<f64>().map_err(|_| {
                ParseError::new(
                    format!("invalid float literal `{value}`"),
                    token.line,
                    token.column,
                )
            })?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    fn at(&self, matcher: TokenMatcher) -> bool {
        self.current().is_some_and(|token| matcher.matches(&token.kind))
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn bump(&mut self) -> Token {
        let token =
            self.tokens.get(self.index).cloned().expect("parser should never advance beyond EOF");
        if !matches!(token.kind, TokenKind::Eof) {
            self.index += 1;
        }
        token
    }

    fn error_here(&self, message: impl fmt::Display) -> ParseError {
        let token = self.current().expect("parser should always have a current token");
        ParseError::new(message.to_string(), token.line, token.column)
    }
}

#[derive(Debug, Clone, Copy)]
enum TokenMatcher {
    Module,
    Use,
    Context,
    Schema,
    Enum,
    Type,
    Fn,
    Describe,
    Tags,
    Requires,
    Ensures,
    Let,
    Return,
    If,
    Else,
    True,
    False,
    Null,
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Arrow,
    Question,
    Equal,
    DoubleEqual,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    AndAnd,
    OrOr,
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

impl TokenMatcher {
    fn matches(self, kind: &TokenKind) -> bool {
        matches!(
            (self, kind),
            (Self::Module, TokenKind::Module)
                | (Self::Use, TokenKind::Use)
                | (Self::Context, TokenKind::Context)
                | (Self::Schema, TokenKind::Schema)
                | (Self::Enum, TokenKind::Enum)
                | (Self::Type, TokenKind::Type)
                | (Self::Fn, TokenKind::Fn)
                | (Self::Describe, TokenKind::Describe)
                | (Self::Tags, TokenKind::Tags)
                | (Self::Requires, TokenKind::Requires)
                | (Self::Ensures, TokenKind::Ensures)
                | (Self::Let, TokenKind::Let)
                | (Self::Return, TokenKind::Return)
                | (Self::If, TokenKind::If)
                | (Self::Else, TokenKind::Else)
                | (Self::True, TokenKind::True)
                | (Self::False, TokenKind::False)
                | (Self::Null, TokenKind::Null)
                | (Self::LeftBrace, TokenKind::LeftBrace)
                | (Self::RightBrace, TokenKind::RightBrace)
                | (Self::LeftParen, TokenKind::LeftParen)
                | (Self::RightParen, TokenKind::RightParen)
                | (Self::LeftBracket, TokenKind::LeftBracket)
                | (Self::RightBracket, TokenKind::RightBracket)
                | (Self::Comma, TokenKind::Comma)
                | (Self::Colon, TokenKind::Colon)
                | (Self::Semicolon, TokenKind::Semicolon)
                | (Self::Dot, TokenKind::Dot)
                | (Self::Arrow, TokenKind::Arrow)
                | (Self::Question, TokenKind::Question)
                | (Self::Equal, TokenKind::Equal)
                | (Self::DoubleEqual, TokenKind::DoubleEqual)
                | (Self::NotEqual, TokenKind::NotEqual)
                | (Self::Greater, TokenKind::Greater)
                | (Self::GreaterEqual, TokenKind::GreaterEqual)
                | (Self::Less, TokenKind::Less)
                | (Self::LessEqual, TokenKind::LessEqual)
                | (Self::AndAnd, TokenKind::AndAnd)
                | (Self::OrOr, TokenKind::OrOr)
                | (Self::Plus, TokenKind::Plus)
                | (Self::Minus, TokenKind::Minus)
                | (Self::Star, TokenKind::Star)
                | (Self::Slash, TokenKind::Slash)
                | (Self::Eof, TokenKind::Eof)
        )
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use kairos_ast::{Expression, Literal, Statement};

    use super::{lex_source, parse_source, TokenKind};

    #[test]
    fn lexes_keywords_literals_and_comments() {
        let tokens = lex_source(
            r#"
            module demo.test;
            // comment
            fn hello() -> Str { return "ok"; }
            "#,
        )
        .expect("source should lex");

        let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();
        assert_eq!(kinds[0], TokenKind::Module);
        assert_eq!(kinds[1], TokenKind::Identifier("demo".to_string()));
        assert_eq!(kinds[2], TokenKind::Dot);
        assert_eq!(kinds[3], TokenKind::Identifier("test".to_string()));
        assert!(kinds.contains(&TokenKind::Fn));
        assert!(kinds.contains(&TokenKind::Arrow));
        assert!(kinds.contains(&TokenKind::String("ok".to_string())));
        assert_eq!(kinds.last(), Some(&TokenKind::Eof));
    }

    #[test]
    fn parses_example_program_with_metadata() {
        let program = parse_source(include_str!("../../../examples/hello_context/src/main.kai"))
            .expect("example should parse");

        assert_eq!(program.module, "demo.hello_context");
        assert_eq!(program.context.as_ref().map(|context| context.entries.len()), Some(4));
        assert_eq!(program.functions.len(), 1);
        assert_eq!(
            program.functions[0].metadata.describe.as_deref(),
            Some("Return a static greeting")
        );
        assert_eq!(program.functions[0].metadata.tags.len(), 2);
        assert_eq!(program.functions[0].metadata.requires.len(), 0);
        assert_eq!(program.functions[0].metadata.ensures.len(), 1);
    }

    #[test]
    fn parses_if_else_chain() {
        let source = r#"
module demo.rules;

fn classify(score: Int) -> Str
describe "demo"
tags ["rules"]
requires [score >= 0]
ensures [len(result) > 0]
{
  if score >= 80 {
    return "HIGH";
  } else if score >= 50 {
    return "MEDIUM";
  } else {
    return "LOW";
  }
}
"#;

        let program = parse_source(source).expect("source should parse");
        let Statement::If(if_statement) = &program.functions[0].body.statements[0] else {
            panic!("expected first statement to be if");
        };

        let Expression::Binary { .. } = if_statement.condition else {
            panic!("expected binary condition");
        };
        let Statement::Return { value: Expression::Literal { value: Literal::String(value) } } =
            &if_statement.then_branch.statements[0]
        else {
            panic!("expected string return");
        };

        assert_eq!(value, "HIGH");
        assert!(if_statement.else_branch.is_some());
    }

    #[test]
    fn rejects_missing_module_declaration() {
        let error =
            parse_source(include_str!("../../../tests/fixtures/invalid/missing_module.kai"))
                .expect_err("source should fail");

        assert_eq!(error.line, 1);
        assert!(error.message.contains("expected `module`"));
    }
}
