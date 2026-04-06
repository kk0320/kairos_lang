use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use kairos_ast::{
    format_expression, BinaryOperator, Block, ElseBranch, EnumDecl, Expression, FieldDecl,
    FunctionDecl, Literal, Param, Program, SchemaDecl, Statement, TypeAliasDecl, TypeRef,
};
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, SemanticError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticLocation {
    pub path: Option<String>,
    pub module: Option<String>,
    pub symbol: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedDiagnostic {
    pub message: String,
    pub location: Option<DiagnosticLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<DiagnosticLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedDiagnostic>,
}

impl Diagnostic {
    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            location: None,
            related: Vec::new(),
        }
    }

    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            location: None,
            related: Vec::new(),
        }
    }

    pub fn with_location(mut self, location: DiagnosticLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_related(
        mut self,
        message: impl Into<String>,
        location: Option<DiagnosticLocation>,
    ) -> Self {
        self.related.push(RelatedDiagnostic { message: message.into(), location });
        self
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
            if let Some(location) = &diagnostic.location {
                write!(f, " ({})", format_diagnostic_location(location))?;
            }
        }
        Ok(())
    }
}

impl Error for SemanticError {}

#[derive(Debug, Clone)]
pub enum ImportedTypeKind {
    Schema,
    Enum,
    Alias(TypeRef),
}

#[derive(Debug, Clone)]
pub struct ImportedType {
    pub name: String,
    pub module: String,
    pub kind: ImportedTypeKind,
}

#[derive(Debug, Clone)]
pub struct ImportedFunction {
    pub name: String,
    pub module: String,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisContext {
    pub file_path: Option<String>,
    pub module: Option<String>,
    pub imported_types: Vec<ImportedType>,
    pub imported_functions: Vec<ImportedFunction>,
}

pub fn analyze(program: Program) -> Result<AnalyzedProgram> {
    let module = program.module.clone();
    analyze_with_context(
        program,
        &AnalysisContext { module: Some(module), ..AnalysisContext::default() },
    )
}

pub fn analyze_with_context(
    program: Program,
    context: &AnalysisContext,
) -> Result<AnalyzedProgram> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let default_location = default_location_for(context, None);

    if program.module.trim().is_empty() {
        errors.push(
            Diagnostic::error("missing_module", "Kairos source must declare a module path")
                .with_location(default_location.clone()),
        );
    }

    validate_context(&program, context, &mut errors, &mut warnings);

    let type_index = build_type_index(&program, context, &mut errors);
    validate_declared_types(&program, context, &type_index, &mut errors);
    let function_index = build_function_index(&program, context, &type_index, &mut errors);

    for function in &program.functions {
        validate_function(
            function,
            context,
            &function_index,
            &type_index,
            &mut errors,
            &mut warnings,
        );
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

fn default_location_for(context: &AnalysisContext, symbol: Option<String>) -> DiagnosticLocation {
    DiagnosticLocation {
        path: context.file_path.clone(),
        module: context.module.clone(),
        symbol,
        line: None,
        column: None,
    }
}

fn format_diagnostic_location(location: &DiagnosticLocation) -> String {
    let mut parts = Vec::new();
    if let Some(path) = &location.path {
        parts.push(path.clone());
    }
    if let Some(module) = &location.module {
        parts.push(format!("module {module}"));
    }
    if let Some(symbol) = &location.symbol {
        parts.push(format!("symbol {symbol}"));
    }
    if let (Some(line), Some(column)) = (location.line, location.column) {
        parts.push(format!("line {line}, column {column}"));
    }
    parts.join(", ")
}

fn validate_context(
    program: &Program,
    analysis_context: &AnalysisContext,
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
            errors.push(
                Diagnostic::error(
                    "duplicate_context_key",
                    format!("context key `{}` is declared more than once", entry.key),
                )
                .with_location(default_location_for(analysis_context, Some(entry.key.clone()))),
            );
        }

        if !is_constant_expression(&entry.value) {
            errors.push(
                Diagnostic::error(
                    "non_constant_context_value",
                    format!(
                        "context key `{}` must use a literal, list, or object literal value",
                        entry.key
                    ),
                )
                .with_location(default_location_for(analysis_context, Some(entry.key.clone()))),
            );
        }

        if !known_keys.contains(&entry.key) {
            warnings.push(
                Diagnostic::warning(
                    "custom_context_key",
                    format!(
                        "context key `{}` is not part of the core key set and will be treated as custom metadata",
                        entry.key
                    ),
                )
                .with_location(default_location_for(analysis_context, Some(entry.key.clone()))),
            );
        }

        match entry.key.as_str() {
            "goal" | "audience" | "domain" => {
                if !matches!(entry.value, Expression::Literal { value: Literal::String(_) }) {
                    errors.push(
                        Diagnostic::error(
                            "invalid_context_value",
                            format!("context key `{}` must be a string literal", entry.key),
                        )
                        .with_location(default_location_for(
                            analysis_context,
                            Some(entry.key.clone()),
                        )),
                    );
                }
            }
            "assumptions" => {
                let Expression::List { items } = &entry.value else {
                    errors.push(
                        Diagnostic::error(
                            "invalid_context_value",
                            "context key `assumptions` must be a list of string literals",
                        )
                        .with_location(default_location_for(
                            analysis_context,
                            Some(entry.key.clone()),
                        )),
                    );
                    continue;
                };

                for item in items {
                    if !matches!(item, Expression::Literal { value: Literal::String(_) }) {
                        errors.push(
                            Diagnostic::error(
                                "invalid_context_value",
                                "context key `assumptions` must contain only string literals",
                            )
                            .with_location(default_location_for(
                                analysis_context,
                                Some(entry.key.clone()),
                            )),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

fn build_type_index(
    program: &Program,
    context: &AnalysisContext,
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<String, TypeDefinition> {
    let mut index = BTreeMap::new();

    for schema in &program.schemas {
        insert_type_definition(
            &mut index,
            &schema.name,
            TypeDefinition::Schema,
            errors,
            default_location_for(context, Some(schema.name.clone())),
        );
        validate_schema_shape(schema, context, errors);
    }

    for enum_decl in &program.enums {
        insert_type_definition(
            &mut index,
            &enum_decl.name,
            TypeDefinition::Enum,
            errors,
            default_location_for(context, Some(enum_decl.name.clone())),
        );
        validate_enum_shape(enum_decl, context, errors);
    }

    for type_alias in &program.type_aliases {
        insert_type_definition(
            &mut index,
            &type_alias.name,
            TypeDefinition::Alias(type_alias.target.clone()),
            errors,
            default_location_for(context, Some(type_alias.name.clone())),
        );
    }

    for imported_type in &context.imported_types {
        let definition = match &imported_type.kind {
            ImportedTypeKind::Schema => TypeDefinition::Schema,
            ImportedTypeKind::Enum => TypeDefinition::Enum,
            ImportedTypeKind::Alias(target) => TypeDefinition::Alias(target.clone()),
        };
        let imported_location = DiagnosticLocation {
            path: context.file_path.clone(),
            module: Some(imported_type.module.clone()),
            symbol: Some(imported_type.name.clone()),
            line: None,
            column: None,
        };
        if is_builtin_type_name(&imported_type.name) {
            errors.push(
                Diagnostic::error(
                    "reserved_type_name",
                    format!("`{}` is a reserved builtin type name", imported_type.name),
                )
                .with_location(imported_location),
            );
            continue;
        }
        if let Some(previous) = index.insert(imported_type.name.clone(), definition) {
            let _ = previous;
            errors.push(
                Diagnostic::error(
                    "duplicate_imported_type",
                    format!(
                        "module imports create an ambiguous type name `{}`",
                        imported_type.name
                    ),
                )
                .with_location(default_location_for(context, Some(imported_type.name.clone())))
                .with_related(
                    format!("also imported from module `{}`", imported_type.module),
                    Some(imported_location),
                ),
            );
        }
    }

    index
}

fn insert_type_definition(
    index: &mut BTreeMap<String, TypeDefinition>,
    name: &str,
    definition: TypeDefinition,
    errors: &mut Vec<Diagnostic>,
    location: DiagnosticLocation,
) {
    if is_builtin_type_name(name) {
        errors.push(
            Diagnostic::error(
                "reserved_type_name",
                format!("`{name}` is a reserved builtin type name"),
            )
            .with_location(location),
        );
        return;
    }

    if index.insert(name.to_string(), definition).is_some() {
        errors.push(
            Diagnostic::error(
                "duplicate_type_definition",
                format!("type `{name}` is declared more than once"),
            )
            .with_location(location),
        );
    }
}

fn validate_schema_shape(
    schema: &SchemaDecl,
    context: &AnalysisContext,
    errors: &mut Vec<Diagnostic>,
) {
    let mut field_names = BTreeSet::new();
    for FieldDecl { name, .. } in &schema.fields {
        if !field_names.insert(name.clone()) {
            errors.push(
                Diagnostic::error(
                    "duplicate_schema_field",
                    format!("schema `{}` declares field `{name}` more than once", schema.name),
                )
                .with_location(default_location_for(context, Some(schema.name.clone()))),
            );
        }
    }
}

fn validate_enum_shape(
    enum_decl: &EnumDecl,
    context: &AnalysisContext,
    errors: &mut Vec<Diagnostic>,
) {
    let mut variants = BTreeSet::new();
    for variant in &enum_decl.variants {
        if !variants.insert(variant.clone()) {
            errors.push(
                Diagnostic::error(
                    "duplicate_enum_variant",
                    format!(
                        "enum `{}` declares variant `{variant}` more than once",
                        enum_decl.name
                    ),
                )
                .with_location(default_location_for(context, Some(enum_decl.name.clone()))),
            );
        }
    }
}

fn validate_declared_types(
    program: &Program,
    context: &AnalysisContext,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
) {
    for schema in &program.schemas {
        for field in &schema.fields {
            if let Err(message) = resolve_type_ref(&field.ty, type_index, &mut BTreeSet::new()) {
                errors.push(
                    Diagnostic::error(
                        "unknown_type",
                        format!(
                            "schema `{}` field `{}` uses an unknown type: {message}",
                            schema.name, field.name
                        ),
                    )
                    .with_location(default_location_for(context, Some(schema.name.clone()))),
                );
            }
        }
    }

    for TypeAliasDecl { name, target, .. } in &program.type_aliases {
        if let Err(message) = resolve_type_ref(target, type_index, &mut BTreeSet::new()) {
            errors.push(
                Diagnostic::error(
                    "unknown_type",
                    format!("type alias `{name}` targets an unknown type: {message}"),
                )
                .with_location(default_location_for(context, Some(name.clone()))),
            );
        }
    }
}

fn build_function_index(
    program: &Program,
    context: &AnalysisContext,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<String, FunctionSignature> {
    let mut functions = BTreeMap::new();

    for function in &program.functions {
        let mut seen_params = BTreeSet::new();
        let mut params = Vec::new();
        for param in &function.params {
            if !seen_params.insert(param.name.clone()) {
                errors.push(
                    Diagnostic::error(
                        "duplicate_parameter",
                        format!(
                            "function `{}` declares parameter `{}` more than once",
                            function.name, param.name
                        ),
                    )
                    .with_location(default_location_for(context, Some(function.name.clone()))),
                );
            }
            if param.name == "result" {
                errors.push(
                    Diagnostic::error(
                        "reserved_binding",
                        format!(
                            "function `{}` cannot use reserved parameter name `result`",
                            function.name
                        ),
                    )
                    .with_location(default_location_for(context, Some(function.name.clone()))),
                );
            }
            match resolve_type_ref(&param.ty, type_index, &mut BTreeSet::new()) {
                Ok(ty) => params.push(ty),
                Err(message) => errors.push(
                    Diagnostic::error(
                        "unknown_type",
                        format!(
                            "function `{}` parameter `{}` uses an unknown type: {message}",
                            function.name, param.name
                        ),
                    )
                    .with_location(default_location_for(context, Some(function.name.clone()))),
                ),
            }
        }

        let return_type =
            match resolve_type_ref(&function.return_type, type_index, &mut BTreeSet::new()) {
                Ok(ty) => ty,
                Err(message) => {
                    errors.push(
                        Diagnostic::error(
                            "unknown_type",
                            format!(
                                "function `{}` has an unknown return type: {message}",
                                function.name
                            ),
                        )
                        .with_location(default_location_for(context, Some(function.name.clone()))),
                    );
                    ValueType::Any
                }
            };

        if functions
            .insert(function.name.clone(), FunctionSignature { params, return_type })
            .is_some()
        {
            errors.push(
                Diagnostic::error(
                    "duplicate_function",
                    format!("function `{}` is declared more than once", function.name),
                )
                .with_location(default_location_for(context, Some(function.name.clone()))),
            );
        }
    }

    for imported_function in &context.imported_functions {
        let params = imported_function
            .params
            .iter()
            .filter_map(|param| resolve_type_ref(&param.ty, type_index, &mut BTreeSet::new()).ok())
            .collect::<Vec<_>>();
        let return_type =
            resolve_type_ref(&imported_function.return_type, type_index, &mut BTreeSet::new())
                .unwrap_or(ValueType::Any);

        if functions
            .insert(imported_function.name.clone(), FunctionSignature { params, return_type })
            .is_some()
        {
            errors.push(
                Diagnostic::error(
                    "duplicate_imported_function",
                    format!(
                        "module imports create an ambiguous function name `{}`",
                        imported_function.name
                    ),
                )
                .with_location(default_location_for(context, Some(imported_function.name.clone())))
                .with_related(
                    format!("also imported from module `{}`", imported_function.module),
                    Some(DiagnosticLocation {
                        path: context.file_path.clone(),
                        module: Some(imported_function.module.clone()),
                        symbol: Some(imported_function.name.clone()),
                        line: None,
                        column: None,
                    }),
                ),
            );
        }
    }

    functions
}

fn validate_function(
    function: &FunctionDecl,
    context: &AnalysisContext,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    warnings: &mut Vec<Diagnostic>,
) {
    let Some(signature) = functions.get(&function.name) else {
        return;
    };

    if function.is_test {
        if !function.params.is_empty() {
            errors.push(
                Diagnostic::error(
                    "invalid_test_signature",
                    format!("test function `{}` must not declare parameters", function.name),
                )
                .with_location(default_location_for(context, Some(function.name.clone()))),
            );
        }

        if !type_matches(&signature.return_type, &ValueType::Bool) {
            errors.push(
                Diagnostic::error(
                    "invalid_test_signature",
                    format!("test function `{}` must return Bool", function.name),
                )
                .with_location(default_location_for(context, Some(function.name.clone()))),
            );
        }
    }

    if function.metadata.describe.as_deref().is_none_or(|value| value.trim().is_empty()) {
        errors.push(
            Diagnostic::error(
                "missing_describe",
                format!("function `{}` must declare `describe` metadata", function.name),
            )
            .with_location(default_location_for(context, Some(function.name.clone()))),
        );
    }

    let mut tag_values = BTreeSet::new();
    for tag in &function.metadata.tags {
        match tag {
            Expression::Literal { value: Literal::String(value) } => {
                if !tag_values.insert(value.clone()) {
                    warnings.push(
                        Diagnostic::warning(
                            "duplicate_tag",
                            format!("function `{}` repeats tag `{value}`", function.name),
                        )
                        .with_location(default_location_for(context, Some(function.name.clone()))),
                    );
                }
            }
            _ => errors.push(
                Diagnostic::error(
                    "invalid_tag",
                    format!("function `{}` tags must be string literals", function.name),
                )
                .with_location(default_location_for(context, Some(function.name.clone()))),
            ),
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
            context,
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
            context,
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
        context,
        functions,
        type_index,
        errors,
        &function.name,
    );

    if !guarantees_return {
        errors.push(
            Diagnostic::error(
                "missing_return",
                format!(
                    "function `{}` does not return a value on every control-flow path",
                    function.name
                ),
            )
            .with_location(default_location_for(context, Some(function.name.clone()))),
        );
    }
}

fn validate_boolean_expression(
    expression: &Expression,
    scope: &ScopeStack,
    context: &AnalysisContext,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
    label: &str,
) {
    let ty = infer_expression_type(
        expression,
        scope,
        context,
        functions,
        type_index,
        errors,
        function_name,
    );
    if !type_matches(&ty, &ValueType::Bool) {
        errors.push(
            Diagnostic::error(
                "invalid_contract_expression",
                format!(
                    "function `{function_name}` `{label}` expression `{}` must evaluate to Bool, found {ty}",
                    format_expression(expression)
                ),
            )
            .with_location(default_location_for(context, Some(function_name.to_string()))),
        );
    }
}

fn validate_block(
    block: &Block,
    scope: &mut ScopeStack,
    expected_return_type: &ValueType,
    context: &AnalysisContext,
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
            context,
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
    context: &AnalysisContext,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> bool {
    match statement {
        Statement::Let { name, value } => {
            if name == "result" {
                errors.push(
                    Diagnostic::error(
                        "reserved_binding",
                        format!("function `{function_name}` cannot bind reserved name `result`"),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
                return false;
            }

            if scope.contains(name) {
                errors.push(
                    Diagnostic::error(
                        "duplicate_binding",
                        format!("function `{function_name}` redeclares local binding `{name}`"),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
                return false;
            }

            let ty = infer_expression_type(
                value,
                scope,
                context,
                functions,
                type_index,
                errors,
                function_name,
            );
            scope.declare(name, ty);
            false
        }
        Statement::Return { value } => {
            let actual = infer_expression_type(
                value,
                scope,
                context,
                functions,
                type_index,
                errors,
                function_name,
            );
            if !type_matches(&actual, expected_return_type) {
                errors.push(
                    Diagnostic::error(
                        "return_type_mismatch",
                        format!(
                            "function `{function_name}` returns `{}` but expected {expected_return_type}",
                            format_expression(value)
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
            }
            true
        }
        Statement::If(if_statement) => {
            let condition = infer_expression_type(
                &if_statement.condition,
                scope,
                context,
                functions,
                type_index,
                errors,
                function_name,
            );
            if !type_matches(&condition, &ValueType::Bool) {
                errors.push(
                    Diagnostic::error(
                        "invalid_condition",
                        format!(
                            "function `{function_name}` uses non-boolean if condition `{}`",
                            format_expression(&if_statement.condition)
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
            }

            let mut then_scope = scope.clone();
            let then_returns = validate_block(
                &if_statement.then_branch,
                &mut then_scope,
                expected_return_type,
                context,
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
                        context,
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
                        context,
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
            infer_expression_type(
                expression,
                scope,
                context,
                functions,
                type_index,
                errors,
                function_name,
            );
            false
        }
    }
}

fn infer_expression_type(
    expression: &Expression,
    scope: &ScopeStack,
    context: &AnalysisContext,
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
            errors.push(
                Diagnostic::error(
                    "undefined_identifier",
                    format!("function `{function_name}` references undefined identifier `{name}`"),
                )
                .with_location(default_location_for(context, Some(function_name.to_string()))),
            );
            ValueType::Any
        }),
        Expression::Call { callee, args } => infer_call_type(
            callee,
            args,
            scope,
            context,
            functions,
            type_index,
            errors,
            function_name,
        ),
        Expression::List { items } => {
            infer_list_type(items, scope, context, functions, type_index, errors, function_name)
        }
        Expression::Object { fields } => {
            let mut seen = BTreeSet::new();
            for field in fields {
                if !seen.insert(field.name.clone()) {
                    errors.push(
                        Diagnostic::error(
                            "duplicate_object_field",
                            format!(
                                "function `{function_name}` repeats object field `{}`",
                                field.name
                            ),
                        )
                        .with_location(default_location_for(
                            context,
                            Some(function_name.to_string()),
                        )),
                    );
                }
                infer_expression_type(
                    &field.value,
                    scope,
                    context,
                    functions,
                    type_index,
                    errors,
                    function_name,
                );
            }
            ValueType::Object
        }
        Expression::Binary { operator, left, right } => {
            let left_ty = infer_expression_type(
                left,
                scope,
                context,
                functions,
                type_index,
                errors,
                function_name,
            );
            let right_ty = infer_expression_type(
                right,
                scope,
                context,
                functions,
                type_index,
                errors,
                function_name,
            );
            infer_binary_type(
                *operator,
                &left_ty,
                &right_ty,
                context,
                errors,
                function_name,
                expression,
            )
        }
    }
}

fn infer_call_type(
    callee: &str,
    args: &[Expression],
    scope: &ScopeStack,
    context: &AnalysisContext,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> ValueType {
    let argument_types: Vec<ValueType> = args
        .iter()
        .map(|arg| {
            infer_expression_type(arg, scope, context, functions, type_index, errors, function_name)
        })
        .collect();

    if let Some(ty) =
        infer_builtin_call_type(callee, &argument_types, context, errors, function_name)
    {
        return ty;
    }

    let Some(signature) = functions.get(callee) else {
        errors.push(
            Diagnostic::error(
                "undefined_function",
                format!("function `{function_name}` calls unknown function `{callee}`"),
            )
            .with_location(default_location_for(context, Some(function_name.to_string()))),
        );
        return ValueType::Any;
    };

    if signature.params.len() != argument_types.len() {
        errors.push(
            Diagnostic::error(
                "invalid_argument_count",
                format!(
                    "function `{function_name}` calls `{callee}` with {} arguments but expected {}",
                    argument_types.len(),
                    signature.params.len()
                ),
            )
            .with_location(default_location_for(context, Some(function_name.to_string()))),
        );
        return signature.return_type.clone();
    }

    for (index, (actual, expected)) in argument_types.iter().zip(&signature.params).enumerate() {
        if !type_matches(actual, expected) {
            errors.push(
                Diagnostic::error(
                    "argument_type_mismatch",
                    format!(
                        "function `{function_name}` passes argument {} to `{callee}` as {actual}, expected {expected}",
                        index + 1
                    ),
                )
                .with_location(default_location_for(context, Some(function_name.to_string()))),
            );
        }
    }

    signature.return_type.clone()
}

fn infer_builtin_call_type(
    callee: &str,
    argument_types: &[ValueType],
    context: &AnalysisContext,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> Option<ValueType> {
    match callee {
        "len" => {
            if argument_types.len() != 1 {
                errors.push(
                    Diagnostic::error(
                        "invalid_argument_count",
                        format!(
                            "function `{function_name}` calls `len` with {} arguments but expected 1",
                            argument_types.len()
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
            } else if !matches!(argument_types[0], ValueType::Str | ValueType::List(_)) {
                errors.push(
                    Diagnostic::error(
                        "argument_type_mismatch",
                        format!(
                            "function `{function_name}` calls `len` with {}, expected Str or List<_>",
                            argument_types[0]
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
            }
            Some(ValueType::Int)
        }
        "concat" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Str, ValueType::Str],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Str)
        }
        "abs" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Int],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Int)
        }
        "min" | "max" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Int, ValueType::Int],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Int)
        }
        "contains" => {
            if argument_types.len() == 2 {
                match (&argument_types[0], &argument_types[1]) {
                    (ValueType::Str, ValueType::Str)
                    | (ValueType::List(_), _)
                    | (ValueType::Object, ValueType::Str) => {}
                    _ => errors.push(
                        Diagnostic::error(
                            "argument_type_mismatch",
                            format!(
                                "function `{function_name}` calls `contains` with incompatible arguments"
                            ),
                        )
                        .with_location(default_location_for(
                            context,
                            Some(function_name.to_string()),
                        )),
                    ),
                }
            } else {
                errors.push(
                    Diagnostic::error(
                        "invalid_argument_count",
                        format!(
                            "function `{function_name}` calls `contains` with {} arguments but expected 2",
                            argument_types.len()
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
            }
            Some(ValueType::Bool)
        }
        "starts_with" | "ends_with" | "trim" | "upper" | "lower" | "normalize_space" => {
            let expected: &[ValueType] =
                if matches!(callee, "trim" | "upper" | "lower" | "normalize_space") {
                    &[ValueType::Str]
                } else {
                    &[ValueType::Str, ValueType::Str]
                };
            check_builtin_args(callee, argument_types, expected, context, errors, function_name);
            if matches!(callee, "starts_with" | "ends_with") {
                Some(ValueType::Bool)
            } else {
                Some(ValueType::Str)
            }
        }
        "join" => {
            if argument_types.len() == 2 {
                if !matches!(&argument_types[0], ValueType::List(inner) if matches!(inner.as_ref(), ValueType::Str | ValueType::Any))
                    || !matches!(&argument_types[1], ValueType::Str)
                {
                    errors.push(
                        Diagnostic::error(
                            "argument_type_mismatch",
                            format!(
                                "function `{function_name}` calls `join` with {}, {}, expected List<Str> and Str",
                                argument_types[0], argument_types[1]
                            ),
                        )
                        .with_location(default_location_for(
                            context,
                            Some(function_name.to_string()),
                        )),
                    );
                }
            } else {
                errors.push(
                    Diagnostic::error(
                        "invalid_argument_count",
                        format!(
                            "function `{function_name}` calls `join` with {} arguments but expected 2",
                            argument_types.len()
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
            }
            Some(ValueType::Str)
        }
        "first" | "last" => {
            if argument_types.len() == 1 {
                match &argument_types[0] {
                    ValueType::List(inner) => Some(ValueType::Optional(inner.clone())),
                    other => {
                        errors.push(
                            Diagnostic::error(
                                "argument_type_mismatch",
                                format!(
                                    "function `{function_name}` calls `{callee}` with {other}, expected List<_>"
                                ),
                            )
                            .with_location(default_location_for(
                                context,
                                Some(function_name.to_string()),
                            )),
                        );
                        Some(ValueType::Any)
                    }
                }
            } else {
                errors.push(
                    Diagnostic::error(
                        "invalid_argument_count",
                        format!(
                            "function `{function_name}` calls `{callee}` with {} arguments but expected 1",
                            argument_types.len()
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
                Some(ValueType::Any)
            }
        }
        "all" | "any" => {
            if argument_types.len() == 1 {
                if !matches!(&argument_types[0], ValueType::List(inner) if matches!(inner.as_ref(), ValueType::Bool | ValueType::Any))
                {
                    errors.push(
                        Diagnostic::error(
                            "argument_type_mismatch",
                            format!(
                                "function `{function_name}` calls `{callee}` with {}, expected List<Bool>",
                                argument_types[0]
                            ),
                        )
                        .with_location(default_location_for(
                            context,
                            Some(function_name.to_string()),
                        )),
                    );
                }
            } else {
                errors.push(
                    Diagnostic::error(
                        "invalid_argument_count",
                        format!(
                            "function `{function_name}` calls `{callee}` with {} arguments but expected 1",
                            argument_types.len()
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
            }
            Some(ValueType::Bool)
        }
        "has_key" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Object, ValueType::Str],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Bool)
        }
        "get_str" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Object, ValueType::Str],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Optional(Box::new(ValueType::Str)))
        }
        "get_int" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Object, ValueType::Str],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Optional(Box::new(ValueType::Int)))
        }
        "get_bool" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Object, ValueType::Str],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Optional(Box::new(ValueType::Bool)))
        }
        "get_list" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Object, ValueType::Str],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Optional(Box::new(ValueType::List(Box::new(ValueType::Any)))))
        }
        "get_obj" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Object, ValueType::Str],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Optional(Box::new(ValueType::Object)))
        }
        "keys" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Object],
                context,
                errors,
                function_name,
            );
            Some(ValueType::List(Box::new(ValueType::Str)))
        }
        "count" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::List(Box::new(ValueType::Any))],
                context,
                errors,
                function_name,
            );
            Some(ValueType::Int)
        }
        "sort" | "unique" => {
            if argument_types.len() == 1 {
                match &argument_types[0] {
                    ValueType::List(inner) => {
                        if callee == "sort"
                            && !matches!(
                                inner.as_ref(),
                                ValueType::Int | ValueType::Float | ValueType::Str | ValueType::Any
                            )
                        {
                            errors.push(
                                Diagnostic::error(
                                    "argument_type_mismatch",
                                    format!(
                                        "function `{function_name}` calls `sort` with List<{inner}>, expected List<Int>, List<Float>, or List<Str>"
                                    ),
                                )
                                .with_location(default_location_for(
                                    context,
                                    Some(function_name.to_string()),
                                )),
                            );
                        }
                        Some(argument_types[0].clone())
                    }
                    other => {
                        errors.push(
                            Diagnostic::error(
                                "argument_type_mismatch",
                                format!(
                                    "function `{function_name}` calls `{callee}` with {other}, expected List<_>"
                                ),
                            )
                            .with_location(default_location_for(
                                context,
                                Some(function_name.to_string()),
                            )),
                        );
                        Some(ValueType::Any)
                    }
                }
            } else {
                errors.push(
                    Diagnostic::error(
                        "invalid_argument_count",
                        format!(
                            "function `{function_name}` calls `{callee}` with {} arguments but expected 1",
                            argument_types.len()
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
                Some(ValueType::Any)
            }
        }
        "clamp" => {
            check_builtin_args(
                callee,
                argument_types,
                &[ValueType::Int, ValueType::Int, ValueType::Int],
                context,
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
    context: &AnalysisContext,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) {
    if actual.len() != expected.len() {
        errors.push(
            Diagnostic::error(
                "invalid_argument_count",
                format!(
                    "function `{function_name}` calls `{callee}` with {} arguments but expected {}",
                    actual.len(),
                    expected.len()
                ),
            )
            .with_location(default_location_for(context, Some(function_name.to_string()))),
        );
        return;
    }

    for (index, (actual_ty, expected_ty)) in actual.iter().zip(expected).enumerate() {
        if !type_matches(actual_ty, expected_ty) {
            errors.push(
                Diagnostic::error(
                    "argument_type_mismatch",
                    format!(
                        "function `{function_name}` passes argument {} to `{callee}` as {actual_ty}, expected {expected_ty}",
                        index + 1
                    ),
                )
                .with_location(default_location_for(context, Some(function_name.to_string()))),
            );
        }
    }
}

fn infer_list_type(
    items: &[Expression],
    scope: &ScopeStack,
    context: &AnalysisContext,
    functions: &BTreeMap<String, FunctionSignature>,
    type_index: &BTreeMap<String, TypeDefinition>,
    errors: &mut Vec<Diagnostic>,
    function_name: &str,
) -> ValueType {
    let mut item_type: Option<ValueType> = None;

    for item in items {
        let current = infer_expression_type(
            item,
            scope,
            context,
            functions,
            type_index,
            errors,
            function_name,
        );
        item_type = Some(match item_type {
            Some(previous) if type_matches(&current, &previous) => previous,
            Some(previous) if type_matches(&previous, &current) => current,
            Some(previous) => {
                errors.push(
                    Diagnostic::error(
                        "inconsistent_list_item_types",
                        format!(
                            "function `{function_name}` mixes list item types {previous} and {current}"
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
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
    context: &AnalysisContext,
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
                errors.push(
                    Diagnostic::error(
                        "invalid_binary_operands",
                        format!(
                            "function `{function_name}` uses incompatible numeric operands in `{}`",
                            format_expression(expression)
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
                ValueType::Any
            }
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            if !type_matches(left, right) && !type_matches(right, left) {
                errors.push(
                    Diagnostic::error(
                        "invalid_binary_operands",
                        format!(
                            "function `{function_name}` compares incompatible values in `{}`",
                            format_expression(expression)
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
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
                errors.push(
                    Diagnostic::error(
                        "invalid_binary_operands",
                        format!(
                            "function `{function_name}` uses non-comparable operands in `{}`",
                            format_expression(expression)
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
                ValueType::Bool
            }
        }
        BinaryOperator::And | BinaryOperator::Or => {
            if matches!(left, ValueType::Bool) && matches!(right, ValueType::Bool) {
                ValueType::Bool
            } else {
                errors.push(
                    Diagnostic::error(
                        "invalid_binary_operands",
                        format!(
                            "function `{function_name}` uses non-boolean operands in `{}`",
                            format_expression(expression)
                        ),
                    )
                    .with_location(default_location_for(context, Some(function_name.to_string()))),
                );
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
    fn rejects_test_function_with_parameters() {
        let program = parse_source(
            r#"
module demo.tests;

test fn smoke(value: Int) -> Bool
describe "invalid test"
tags ["test"]
requires []
ensures [result == true]
{
  return value > 0;
}
"#,
        )
        .expect("source should parse");

        let error = analyze(program).expect_err("analysis should fail");
        assert!(error.to_string().contains("invalid_test_signature"));
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
            &super::AnalysisContext::default(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &mut Vec::new(),
            "demo",
        );

        assert_eq!(ty, ValueType::List(Box::new(ValueType::Int)));
    }
}
