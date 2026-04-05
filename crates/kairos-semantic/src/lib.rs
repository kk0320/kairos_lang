use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use kairos_ast::{
    format_expression, BinaryOperator, Block, ElseBranch, EnumDecl, Expression, FieldDecl,
    FunctionDecl, Literal, Program, SchemaDecl, Statement, TypeAliasDecl, TypeRef,
};

pub type Result<T> = std::result::Result<T, SemanticError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

impl Diagnostic {
    fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, code, message: message.into() }
    }

    fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self { severity: Severity::Error, code, message: message.into() }
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzedProgram {
    pub program: Program,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct SemanticError {
    pub diagnostics: Vec<Diagnostic>,
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "[{}] {}", diagnostic.code, diagnostic.message)?;
        }
        Ok(())
    }
}

impl Error for SemanticError {}

pub fn analyze(program: Program) -> Result<AnalyzedProgram> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if program.module.trim().is_empty() {
        errors
            .push(Diagnostic::error("missing_module", "Kairos source must declare a module path"));
    }

    validate_context(&program, &mut errors, &mut warnings);

    let type_index = build_type_index(&program, &mut errors);
    validate_declared_types(&program, &type_index, &mut errors);
    let function_index = build_function_index(&program, &type_index, &mut errors);

    for function in &program.functions {
        validate_function(function, &function_index, &type_index, &mut errors, &mut warnings);
    }

    if errors.is_empty() {
        Ok(AnalyzedProgram { program, warnings })
    } else {
        Err(SemanticError { diagnostics: errors })
    }
}

#[derive(Debug, Clone)]
enum TypeDefinition {
    Schema,
    Enum,
    Alias(TypeRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionSignature {
    params: Vec<ValueType>,
    return_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ValueType {
    Int,
    Float,
    Bool,
    Str,
    Null,
    List(Box<ValueType>),
    Object,
    Named(String),
    Optional(Box<ValueType>),
    Any,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => f.write_str("Int"),
            Self::Float => f.write_str("Float"),
            Self::Bool => f.write_str("Bool"),
            Self::Str => f.write_str("Str"),
            Self::Null => f.write_str("Null"),
            Self::List(inner) => write!(f, "List<{inner}>"),
            Self::Object => f.write_str("Object"),
            Self::Named(name) => f.write_str(name),
            Self::Optional(inner) => write!(f, "{inner}?"),
            Self::Any => f.write_str("Any"),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ScopeStack {
    scopes: Vec<BTreeMap<String, ValueType>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self { scopes: vec![BTreeMap::new()] }
    }

    fn push(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn contains(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains_key(name))
    }

    fn declare(&mut self, name: &str, ty: ValueType) {
        self.scopes
            .last_mut()
            .expect("scope stack must have at least one scope")
            .insert(name.to_string(), ty);
    }

    fn lookup(&self, name: &str) -> Option<&ValueType> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }
}

fn validate_context(
    program: &Program,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let Some(context) = &program.context else {
        return;
    };

    let mut seen = BTreeSet::new();
    let known_keys = BTreeSet::from([
        "goal".to_string(),
        "audience".to_string(),
        "domain".to_string(),
        "assumptions".to_string(),
    ]);

    for entry in &context.entries {
        if !seen.insert(entry.key.clone()) {
            errors.push(Diagnostic::error(
                "duplicate_context_key",
                format!("context key `{}` is declared more than once", entry.key),
            ));
        }

        if !is_constant_expression(&entry.value) {
            errors.push(Diagnostic::error(
                "non_constant_context_value",
                format!(
                    "context key `{}` must use a literal, list, or object literal value",
                    entry.key
                ),
            ));
        }

        if !known_keys.contains(&entry.key) {
            warnings.push(Diagnostic::warning(
                "custom_context_key",
                format!(
                    "context key `{}` is not part of the core key set and will be treated as custom metadata",
                    entry.key
                ),
            ));
        }

        match entry.key.as_str() {
            "goal" | "audience" | "domain" => {
                if !matches!(entry.value, Expression::Literal { value: Literal::String(_) }) {
                    errors.push(Diagnostic::error(
                        "invalid_context_value",
                        format!("context key `{}` must be a string literal", entry.key),
                    ));
                }
            }
            "assumptions" => {
                let Expression::List { items } = &entry.value else {
                    errors.push(Diagnostic::error(
                        "invalid_context_value",
                        "context key `assumptions` must be a list of string literals",
                    ));
                    continue;
                };

                for item in items {
                    if !matches!(item, Expression::Literal { value: Literal::String(_) }) {
                        errors.push(Diagnostic::error(
                            "invalid_context_value",
                            "context key `assumptions` must contain only string literals",
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

fn build_type_index(
    program: &Program,
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<String, TypeDefinition> {
    let mut index = BTreeMap::new();

    for schema in &program.schemas {
        insert_type_definition(&mut index, &schema.name, TypeDefinition::Schema, errors);
        validate_schema_shape(schema, errors);
    }

    for enum_decl in &program.enums {
        insert_type_definition(&mut index, &enum_decl.name, TypeDefinition::Enum, errors);
        validate_enum_shape(enum_decl, errors);
    }

    for type_alias in &program.type_aliases {
        insert_type_definition(
            &mut index,
            &type_alias.name,
            TypeDefinition::Alias(type_alias.target.clone()),
            errors,
        );
    }

    index
}

fn insert_type_definition(
    index: &mut BTreeMap<String, TypeDefinition>,
    name: &str,
    definition: TypeDefinition,
    errors: &mut Vec<Diagnostic>,
) {
    if is_builtin_type_name(name) {
        errors.push(Diagnostic::error(
            "reserved_type_name",
            format!("`{name}` is a reserved builtin type name"),
        ));
        return;
    }

    if index.insert(name.to_string(), definition).is_some() {
        errors.push(Diagnostic::error(
            "duplicate_type_definition",
            format!("type `{name}` is declared more than once"),
        ));
    }
}

fn validate_schema_shape(schema: &SchemaDecl, errors: &mut Vec<Diagnostic>) {
    let mut field_names = BTreeSet::new();
    for FieldDecl { name, .. } in &schema.fields {
        if !field_names.insert(name.clone()) {
            errors.push(Diagnostic::error(
                "duplicate_schema_field",
                format!("schema `{}` declares field `{name}` more than once", schema.name),
            ));
        }
    }
}

fn validate_enum_shape(enum_decl: &EnumDecl, errors: &mut Vec<Diagnostic>) {
    let mut variants = BTreeSet::new();
    for variant in &enum_decl.variants {
        if !variants.insert(variant.clone()) {
            errors.push(Diagnostic::error(
                "duplicate_enum_variant",
                format!("enum `{}` declares variant `{variant}` more than once", enum_decl.name),
            ));
        }
    }
}

fn validate_declared_types(
    program: &Program,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
) {
    for schema in &program.schemas {
        for field in &schema.fields {
            if let Err(message) = resolve_type_ref(&field.ty, type_index, &mut BTreeSet::new()) {
                errors.push(Diagnostic::error(
                    "unknown_type",
                    format!(
                        "schema `{}` field `{}` uses an unknown type: {message}",
                        schema.name, field.name
                    ),
                ));
            }
        }
    }

    for TypeAliasDecl { name, target } in &program.type_aliases {
        if let Err(message) = resolve_type_ref(target, type_index, &mut BTreeSet::new()) {
            errors.push(Diagnostic::error(
                "unknown_type",
                format!("type alias `{name}` targets an unknown type: {message}"),
            ));
        }
    }
}

fn build_function_index(
    program: &Program,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<String, FunctionSignature> {
    let mut functions = BTreeMap::new();

    for function in &program.functions {
        let mut seen_params = BTreeSet::new();
        let mut params = Vec::new();
        for param in &function.params {
            if !seen_params.insert(param.name.clone()) {
                errors.push(Diagnostic::error(
                    "duplicate_parameter",
                    format!(
                        "function `{}` declares parameter `{}` more than once",
                        function.name, param.name
                    ),
                ));
            }
            if param.name == "result" {
                errors.push(Diagnostic::error(
                    "reserved_binding",
                    format!(
                        "function `{}` cannot use reserved parameter name `result`",
                        function.name
                    ),
                ));
            }
            match resolve_type_ref(&param.ty, type_index, &mut BTreeSet::new()) {
                Ok(ty) => params.push(ty),
                Err(message) => errors.push(Diagnostic::error(
                    "unknown_type",
                    format!(
                        "function `{}` parameter `{}` uses an unknown type: {message}",
                        function.name, param.name
                    ),
                )),
            }
        }

        let return_type =
            match resolve_type_ref(&function.return_type, type_index, &mut BTreeSet::new()) {
                Ok(ty) => ty,
                Err(message) => {
                    errors.push(Diagnostic::error(
                        "unknown_type",
                        format!(
                            "function `{}` has an unknown return type: {message}",
                            function.name
                        ),
                    ));
                    ValueType::Any
                }
            };

        if functions
            .insert(function.name.clone(), FunctionSignature { params, return_type })
            .is_some()
        {
            errors.push(Diagnostic::error(
                "duplicate_function",
                format!("function `{}` is declared more than once", function.name),
            ));
        }
    }

    functions
}

fn validate_function(
    function: &FunctionDecl,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let Some(signature) = functions.get(&function.name) else {
        return;
    };

    if function.metadata.describe.as_deref().is_none_or(|value| value.trim().is_empty()) {
        errors.push(Diagnostic::error(
            "missing_describe",
            format!("function `{}` must declare `describe` metadata", function.name),
        ));
    }

    let mut tag_values = BTreeSet::new();
    for tag in &function.metadata.tags {
        match tag {
            Expression::Literal { value: Literal::String(value) } => {
                if !tag_values.insert(value.clone()) {
                    warnings.push(Diagnostic::warning(
                        "duplicate_tag",
                        format!("function `{}` repeats tag `{value}`", function.name),
                    ));
                }
            }
            _ => errors.push(Diagnostic::error(
                "invalid_tag",
                format!("function `{}` tags must be string literals", function.name),
            )),
        }
    }

    let mut scope = ScopeStack::new();
    for (param, ty) in function.params.iter().zip(&signature.params) {
        scope.declare(&param.name, ty.clone());
    }

    for require in &function.metadata.requires {
        validate_boolean_expression(
            require,
            &scope,
            functions,
            type_index,
            errors,
            &function.name,
            "requires",
        );
    }

    let mut ensure_scope = scope.clone();
    ensure_scope.declare("result", signature.return_type.clone());
    for ensure in &function.metadata.ensures {
        validate_boolean_expression(
            ensure,
            &ensure_scope,
            functions,
            type_index,
            errors,
            &function.name,
            "ensures",
        );
    }

    let mut body_scope = scope;
    let guarantees_return = validate_block(
        &function.body,
        &mut body_scope,
        &signature.return_type,
        functions,
        type_index,
        errors,
        &function.name,
    );

    if !guarantees_return {
        errors.push(Diagnostic::error(
            "missing_return",
            format!(
                "function `{}` does not return a value on every control-flow path",
                function.name
            ),
        ));
    }
}

fn validate_boolean_expression(
    expression: &Expression,
    scope: &ScopeStack,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
    label: &str,
) {
    let ty = infer_expression_type(expression, scope, functions, type_index, errors, function_name);
    if !type_matches(&ty, &ValueType::Bool) {
        errors.push(Diagnostic::error(
            "invalid_contract_expression",
            format!(
                "function `{function_name}` `{label}` expression `{}` must evaluate to Bool, found {ty}",
                format_expression(expression)
            ),
        ));
    }
}

fn validate_block(
    block: &Block,
    scope: &mut ScopeStack,
    expected_return_type: &ValueType,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> bool {
    scope.push();
    let mut guarantees_return = false;

    for statement in &block.statements {
        let statement_returns = validate_statement(
            statement,
            scope,
            expected_return_type,
            functions,
            type_index,
            errors,
            function_name,
        );

        if statement_returns {
            guarantees_return = true;
            break;
        }
    }

    scope.pop();
    guarantees_return
}

fn validate_statement(
    statement: &Statement,
    scope: &mut ScopeStack,
    expected_return_type: &ValueType,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> bool {
    match statement {
        Statement::Let { name, value } => {
            if name == "result" {
                errors.push(Diagnostic::error(
                    "reserved_binding",
                    format!("function `{function_name}` cannot bind reserved name `result`"),
                ));
                return false;
            }

            if scope.contains(name) {
                errors.push(Diagnostic::error(
                    "duplicate_binding",
                    format!("function `{function_name}` redeclares local binding `{name}`"),
                ));
                return false;
            }

            let ty =
                infer_expression_type(value, scope, functions, type_index, errors, function_name);
            scope.declare(name, ty);
            false
        }
        Statement::Return { value } => {
            let actual =
                infer_expression_type(value, scope, functions, type_index, errors, function_name);
            if !type_matches(&actual, expected_return_type) {
                errors.push(Diagnostic::error(
                    "return_type_mismatch",
                    format!(
                        "function `{function_name}` returns `{}` but expected {expected_return_type}",
                        format_expression(value)
                    ),
                ));
            }
            true
        }
        Statement::If(if_statement) => {
            let condition = infer_expression_type(
                &if_statement.condition,
                scope,
                functions,
                type_index,
                errors,
                function_name,
            );
            if !type_matches(&condition, &ValueType::Bool) {
                errors.push(Diagnostic::error(
                    "invalid_condition",
                    format!(
                        "function `{function_name}` uses non-boolean if condition `{}`",
                        format_expression(&if_statement.condition)
                    ),
                ));
            }

            let mut then_scope = scope.clone();
            let then_returns = validate_block(
                &if_statement.then_branch,
                &mut then_scope,
                expected_return_type,
                functions,
                type_index,
                errors,
                function_name,
            );

            let else_returns = match &if_statement.else_branch {
                Some(ElseBranch::Block(block)) => {
                    let mut else_scope = scope.clone();
                    validate_block(
                        block,
                        &mut else_scope,
                        expected_return_type,
                        functions,
                        type_index,
                        errors,
                        function_name,
                    )
                }
                Some(ElseBranch::If(nested)) => {
                    let mut else_scope = scope.clone();
                    validate_statement(
                        &Statement::If((**nested).clone()),
                        &mut else_scope,
                        expected_return_type,
                        functions,
                        type_index,
                        errors,
                        function_name,
                    )
                }
                None => false,
            };

            then_returns && else_returns
        }
        Statement::Expr { expression } => {
            infer_expression_type(expression, scope, functions, type_index, errors, function_name);
            false
        }
    }
}

fn infer_expression_type(
    expression: &Expression,
    scope: &ScopeStack,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> ValueType {
    match expression {
        Expression::Literal { value } => match value {
            Literal::String(_) => ValueType::Str,
            Literal::Integer(_) => ValueType::Int,
            Literal::Float(_) => ValueType::Float,
            Literal::Boolean(_) => ValueType::Bool,
            Literal::Null => ValueType::Null,
        },
        Expression::Identifier { name } => scope.lookup(name).cloned().unwrap_or_else(|| {
            errors.push(Diagnostic::error(
                "undefined_identifier",
                format!("function `{function_name}` references undefined identifier `{name}`"),
            ));
            ValueType::Any
        }),
        Expression::Call { callee, args } => {
            infer_call_type(callee, args, scope, functions, type_index, errors, function_name)
        }
        Expression::List { items } => {
            infer_list_type(items, scope, functions, type_index, errors, function_name)
        }
        Expression::Object { fields } => {
            let mut seen = BTreeSet::new();
            for field in fields {
                if !seen.insert(field.name.clone()) {
                    errors.push(Diagnostic::error(
                        "duplicate_object_field",
                        format!("function `{function_name}` repeats object field `{}`", field.name),
                    ));
                }
                infer_expression_type(
                    &field.value,
                    scope,
                    functions,
                    type_index,
                    errors,
                    function_name,
                );
            }
            ValueType::Object
        }
        Expression::Binary { operator, left, right } => {
            let left_ty =
                infer_expression_type(left, scope, functions, type_index, errors, function_name);
            let right_ty =
                infer_expression_type(right, scope, functions, type_index, errors, function_name);
            infer_binary_type(*operator, &left_ty, &right_ty, errors, function_name, expression)
        }
    }
}

fn infer_call_type(
    callee: &str,
    args: &[Expression],
    scope: &ScopeStack,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> ValueType {
    let argument_types: Vec<ValueType> = args
        .iter()
        .map(|arg| infer_expression_type(arg, scope, functions, type_index, errors, function_name))
        .collect();

    if let Some(ty) = infer_builtin_call_type(callee, &argument_types, errors, function_name) {
        return ty;
    }

    let Some(signature) = functions.get(callee) else {
        errors.push(Diagnostic::error(
            "undefined_function",
            format!("function `{function_name}` calls unknown function `{callee}`"),
        ));
        return ValueType::Any;
    };

    if signature.params.len() != argument_types.len() {
        errors.push(Diagnostic::error(
            "invalid_argument_count",
            format!(
                "function `{function_name}` calls `{callee}` with {} arguments but expected {}",
                argument_types.len(),
                signature.params.len()
            ),
        ));
        return signature.return_type.clone();
    }

    for (index, (actual, expected)) in argument_types.iter().zip(&signature.params).enumerate() {
        if !type_matches(actual, expected) {
            errors.push(Diagnostic::error(
                "argument_type_mismatch",
                format!(
                    "function `{function_name}` passes argument {} to `{callee}` as {actual}, expected {expected}",
                    index + 1
                ),
            ));
        }
    }

    signature.return_type.clone()
}

fn infer_builtin_call_type(
    callee: &str,
    argument_types: &[ValueType],
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> Option<ValueType> {
    match callee {
        "len" => {
            if argument_types.len() != 1 {
                errors.push(Diagnostic::error(
                    "invalid_argument_count",
                    format!(
                        "function `{function_name}` calls `len` with {} arguments but expected 1",
                        argument_types.len()
                    ),
                ));
            } else if !matches!(argument_types[0], ValueType::Str | ValueType::List(_)) {
                errors.push(Diagnostic::error(
                    "argument_type_mismatch",
                    format!(
                        "function `{function_name}` calls `len` with {}, expected Str or List<_>",
                        argument_types[0]
                    ),
                ));
            }
            Some(ValueType::Int)
        }
        "concat" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Str, ValueType::Str],
                errors,
                function_name,
            );
            Some(ValueType::Str)
        }
        "abs" => {
            check_builtin_args(callee, argument_types, &[ValueType::Int], errors, function_name);
            Some(ValueType::Int)
        }
        "min" | "max" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Int, ValueType::Int],
                errors,
                function_name,
            );
            Some(ValueType::Int)
        }
        _ => None,
    }
}

fn check_builtin_args(
    callee: &str,
    actual: &[ValueType],
    expected: &[ValueType],
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) {
    if actual.len() != expected.len() {
        errors.push(Diagnostic::error(
            "invalid_argument_count",
            format!(
                "function `{function_name}` calls `{callee}` with {} arguments but expected {}",
                actual.len(),
                expected.len()
            ),
        ));
        return;
    }

    for (index, (actual_ty, expected_ty)) in actual.iter().zip(expected).enumerate() {
        if !type_matches(actual_ty, expected_ty) {
            errors.push(Diagnostic::error(
                "argument_type_mismatch",
                format!(
                    "function `{function_name}` passes argument {} to `{callee}` as {actual_ty}, expected {expected_ty}",
                    index + 1
                ),
            ));
        }
    }
}

fn infer_list_type(
    items: &[Expression],
    scope: &ScopeStack,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> ValueType {
    let mut item_type: Option<ValueType> = None;

    for item in items {
        let current =
            infer_expression_type(item, scope, functions, type_index, errors, function_name);
        item_type = Some(match item_type {
            Some(previous) if type_matches(&current, &previous) => previous,
            Some(previous) if type_matches(&previous, &current) => current,
            Some(previous) => {
                errors.push(Diagnostic::error(
                    "inconsistent_list_item_types",
                    format!(
                        "function `{function_name}` mixes list item types {previous} and {current}"
                    ),
                ));
                ValueType::Any
            }
            None => current,
        });
    }

    ValueType::List(Box::new(item_type.unwrap_or(ValueType::Any)))
}

fn infer_binary_type(
    operator: BinaryOperator,
    left: &ValueType,
    right: &ValueType,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
    expression: &Expression,
) -> ValueType {
    match operator {
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide => {
            if matches!(left, ValueType::Int | ValueType::Float) && type_matches(left, right) {
                left.clone()
            } else {
                errors.push(Diagnostic::error(
                    "invalid_binary_operands",
                    format!(
                        "function `{function_name}` uses incompatible numeric operands in `{}`",
                        format_expression(expression)
                    ),
                ));
                ValueType::Any
            }
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            if !type_matches(left, right) && !type_matches(right, left) {
                errors.push(Diagnostic::error(
                    "invalid_binary_operands",
                    format!(
                        "function `{function_name}` compares incompatible values in `{}`",
                        format_expression(expression)
                    ),
                ));
            }
            ValueType::Bool
        }
        BinaryOperator::Greater
        | BinaryOperator::GreaterEqual
        | BinaryOperator::Less
        | BinaryOperator::LessEqual => {
            if matches!(left, ValueType::Int | ValueType::Float) && type_matches(left, right) {
                ValueType::Bool
            } else {
                errors.push(Diagnostic::error(
                    "invalid_binary_operands",
                    format!(
                        "function `{function_name}` uses non-comparable operands in `{}`",
                        format_expression(expression)
                    ),
                ));
                ValueType::Bool
            }
        }
        BinaryOperator::And | BinaryOperator::Or => {
            if matches!(left, ValueType::Bool) && matches!(right, ValueType::Bool) {
                ValueType::Bool
            } else {
                errors.push(Diagnostic::error(
                    "invalid_binary_operands",
                    format!(
                        "function `{function_name}` uses non-boolean operands in `{}`",
                        format_expression(expression)
                    ),
                ));
                ValueType::Bool
            }
        }
    }
}

fn resolve_type_ref(
    type_ref: &TypeRef,
    type_index: &BTreeMap<String, TypeDefinition>,
    seen_aliases: &mut BTreeSet<String>,
) -> std::result::Result<ValueType, String> {
    let base = match type_ref.name.as_str() {
        "Int" => {
            ensure_no_generic_arguments(type_ref, "Int")?;
            ValueType::Int
        }
        "Float" => {
            ensure_no_generic_arguments(type_ref, "Float")?;
            ValueType::Float
        }
        "Bool" => {
            ensure_no_generic_arguments(type_ref, "Bool")?;
            ValueType::Bool
        }
        "Str" => {
            ensure_no_generic_arguments(type_ref, "Str")?;
            ValueType::Str
        }
        "Null" => {
            ensure_no_generic_arguments(type_ref, "Null")?;
            ValueType::Null
        }
        "Any" => {
            ensure_no_generic_arguments(type_ref, "Any")?;
            ValueType::Any
        }
        "List" => {
            if type_ref.arguments.len() != 1 {
                return Err("`List` expects exactly one type argument".to_string());
            }
            ValueType::List(Box::new(resolve_type_ref(
                &type_ref.arguments[0],
                type_index,
                seen_aliases,
            )?))
        }
        other => match type_index.get(other) {
            Some(TypeDefinition::Schema) | Some(TypeDefinition::Enum) => {
                if !type_ref.arguments.is_empty() {
                    return Err(format!("`{other}` does not accept generic type arguments"));
                }
                ValueType::Named(other.to_string())
            }
            Some(TypeDefinition::Alias(target)) => {
                if !seen_aliases.insert(other.to_string()) {
                    return Err(format!("type alias cycle detected at `{other}`"));
                }
                let resolved = resolve_type_ref(target, type_index, seen_aliases)?;
                seen_aliases.remove(other);
                resolved
            }
            None => return Err(format!("unknown type `{other}`")),
        },
    };

    if type_ref.optional {
        Ok(ValueType::Optional(Box::new(base)))
    } else {
        Ok(base)
    }
}

fn ensure_no_generic_arguments(type_ref: &TypeRef, name: &str) -> std::result::Result<(), String> {
    if type_ref.arguments.is_empty() {
        Ok(())
    } else {
        Err(format!("`{name}` does not accept generic type arguments"))
    }
}

fn type_matches(actual: &ValueType, expected: &ValueType) -> bool {
    if matches!(actual, ValueType::Any) || matches!(expected, ValueType::Any) {
        return true;
    }

    match (actual, expected) {
        (ValueType::Null, ValueType::Optional(_)) => true,
        (actual, ValueType::Optional(inner)) => type_matches(actual, inner),
        (ValueType::Optional(inner), expected) => type_matches(inner, expected),
        (ValueType::List(left), ValueType::List(right)) => type_matches(left, right),
        (ValueType::Named(left), ValueType::Named(right)) => left == right,
        _ => actual == expected,
    }
}

fn is_constant_expression(expression: &Expression) -> bool {
    match expression {
        Expression::Literal { .. } => true,
        Expression::List { items } => items.iter().all(is_constant_expression),
        Expression::Object { fields } => {
            fields.iter().all(|field| is_constant_expression(&field.value))
        }
        Expression::Identifier { .. } | Expression::Call { .. } | Expression::Binary { .. } => {
            false
        }
    }
}

fn is_builtin_type_name(name: &str) -> bool {
    matches!(name, "Int" | "Float" | "Bool" | "Str" | "Null" | "Any" | "List")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kairos_ast::{Expression, Literal};
    use kairos_parser::parse_source;

    use super::{analyze, Severity, ValueType};

    #[test]
    fn analyzes_valid_example() {
        let program = parse_source(include_str!("../../../examples/hello_context/src/main.kai"))
            .expect("example should parse");
        let analyzed = analyze(program).expect("example should analyze");

        assert!(analyzed.warnings.is_empty());
    }

    #[test]
    fn reports_undefined_identifier() {
        let program = parse_source(
            r#"
module demo.bad;

fn broken() -> Str
describe "broken"
tags ["demo"]
requires []
ensures [len(result) > 0]
{
  return missing;
}
"#,
        )
        .expect("source should parse");

        let error = analyze(program).expect_err("analysis should fail");
        assert!(error.to_string().contains("undefined identifier `missing`"));
    }

    #[test]
    fn rejects_non_boolean_requires_expression() {
        let program = parse_source(
            r#"
module demo.bad;

fn broken() -> Str
describe "broken"
tags ["demo"]
requires ["nope"]
ensures [len(result) > 0]
{
  return "ok";
}
"#,
        )
        .expect("source should parse");

        let error = analyze(program).expect_err("analysis should fail");
        assert!(error.to_string().contains("must evaluate to Bool"));
    }

    #[test]
    fn warns_on_custom_context_keys() {
        let program = parse_source(
            r#"
module demo.ctx;

context {
  goal: "ok";
  custom_note: "yes";
}

fn hello() -> Str
describe "hello"
tags ["demo"]
requires []
ensures [len(result) > 0]
{
  return "ok";
}
"#,
        )
        .expect("source should parse");

        let analyzed = analyze(program).expect("analysis should succeed");
        assert_eq!(analyzed.warnings.len(), 1);
        assert_eq!(analyzed.warnings[0].severity, Severity::Warning);
    }

    #[test]
    fn infers_list_type_consistently() {
        let expression = Expression::List {
            items: vec![
                Expression::Literal { value: Literal::Integer(1) },
                Expression::Literal { value: Literal::Integer(2) },
            ],
        };

        let ty = super::infer_list_type(
            match &expression {
                Expression::List { items } => items,
                _ => unreachable!(),
            },
            &super::ScopeStack::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &mut Vec::new(),
            "demo",
        );

        assert_eq!(ty, ValueType::List(Box::new(ValueType::Int)));
    }
}
