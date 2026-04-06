use std::{
    collections::BTreeMap,
    fmt::{self, Write as _},
};

use kairos_ast::{
    format_expression, BinaryOperator, ElseBranch, Expression, FieldDecl, Literal, Program,
    Statement, TypeRef, Visibility,
};
use kairos_project::AnalyzedProject;
use kairos_semantic::AnalyzedProgram;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirProgram {
    pub package: String,
    pub module: String,
    pub relative_path: String,
    pub imports: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub import_bindings: Vec<KirImportBinding>,
    pub context: BTreeMap<String, Value>,
    pub schemas: Vec<KirSchema>,
    pub enums: Vec<KirEnum>,
    pub type_aliases: Vec<KirTypeAlias>,
    pub functions: Vec<KirFunction>,
    pub source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirProject {
    pub package: KirPackage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<KirPackageNode>,
    pub modules: Vec<KirProgram>,
    pub source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirPackage {
    pub name: String,
    pub version: String,
    pub entry_file: String,
    pub entry_module: String,
    pub emit: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<KirPackageDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirPackageNode {
    pub name: String,
    pub version: String,
    pub entry_file: String,
    pub entry_module: String,
    pub is_root: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<KirPackageDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirPackageDependency {
    pub alias: String,
    pub package: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirImportBinding {
    pub module: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<KirImportItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirImportItem {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirSchema {
    pub visibility: KirVisibility,
    pub name: String,
    pub fields: Vec<KirField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirEnum {
    pub visibility: KirVisibility,
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirTypeAlias {
    pub visibility: KirVisibility,
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirFunction {
    pub visibility: KirVisibility,
    pub is_test: bool,
    pub name: String,
    #[serde(rename = "return_type")]
    pub return_type: String,
    pub params: Vec<KirParam>,
    pub metadata: KirMetadata,
    pub body: Vec<KirStatement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirParam {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirMetadata {
    pub describe: String,
    pub tags: Vec<String>,
    pub requires: Vec<KirExpression>,
    pub ensures: Vec<KirExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KirStatement {
    Let {
        name: String,
        value: KirExpression,
    },
    Return {
        value: KirExpression,
    },
    If {
        condition: KirExpression,
        then_branch: Vec<KirStatement>,
        else_branch: Option<KirElseBranch>,
    },
    Expr {
        expression: KirExpression,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KirElseBranch {
    Block { statements: Vec<KirStatement> },
    If { statement: Box<KirStatement> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KirExpression {
    Literal { value: KirLiteral },
    Identifier { name: String },
    Call { callee: String, args: Vec<KirExpression> },
    List { items: Vec<KirExpression> },
    Object { fields: Vec<KirObjectField> },
    Binary { operator: KirBinaryOperator, left: Box<KirExpression>, right: Box<KirExpression> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KirObjectField {
    pub name: String,
    pub value: KirExpression,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum KirLiteral {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KirBinaryOperator {
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KirVisibility {
    Public,
    Internal,
}

pub fn lower(analyzed: &AnalyzedProgram) -> KirProgram {
    lower_program_with_context(analyzed, "", "")
}

fn lower_program_with_context(
    analyzed: &AnalyzedProgram,
    package: &str,
    relative_path: &str,
) -> KirProgram {
    let program = &analyzed.program;

    KirProgram {
        package: package.to_string(),
        module: program.module.clone(),
        relative_path: relative_path.to_string(),
        imports: program.uses.clone(),
        import_bindings: program
            .imports
            .iter()
            .map(|import| KirImportBinding {
                module: import.module.clone(),
                alias: import.alias.clone(),
                items: import
                    .items
                    .iter()
                    .map(|item| KirImportItem {
                        name: item.name.clone(),
                        alias: item.alias.clone(),
                    })
                    .collect(),
            })
            .collect(),
        context: lower_context(program),
        schemas: program
            .schemas
            .iter()
            .map(|schema| KirSchema {
                visibility: lower_visibility(schema.visibility),
                name: schema.name.clone(),
                fields: schema
                    .fields
                    .iter()
                    .map(|FieldDecl { name, ty }| KirField {
                        name: name.clone(),
                        ty: format_type_ref(ty),
                    })
                    .collect(),
            })
            .collect(),
        enums: program
            .enums
            .iter()
            .map(|enum_decl| KirEnum {
                visibility: lower_visibility(enum_decl.visibility),
                name: enum_decl.name.clone(),
                variants: enum_decl.variants.clone(),
            })
            .collect(),
        type_aliases: program
            .type_aliases
            .iter()
            .map(|alias| KirTypeAlias {
                visibility: lower_visibility(alias.visibility),
                name: alias.name.clone(),
                target: format_type_ref(&alias.target),
            })
            .collect(),
        functions: program.functions.iter().map(lower_function).collect(),
        source_hash: hash_source(&program.source),
    }
}

pub fn lower_project(project: &AnalyzedProject) -> KirProject {
    let modules = project
        .project
        .modules
        .iter()
        .zip(&project.modules)
        .map(|(project_module, analyzed_module)| {
            lower_program_with_context(
                &analyzed_module.analyzed,
                &project_module.package,
                &project_module.relative_path,
            )
        })
        .collect::<Vec<_>>();

    KirProject {
        package: KirPackage {
            name: project.project.manifest.package.name.clone(),
            version: project.project.manifest.package.version.clone(),
            entry_file: project.project.entry_file.clone(),
            entry_module: project.project.entry_module.clone(),
            emit: project.project.manifest.build.emit.clone(),
            dependencies: project
                .project
                .package(&project.project.manifest.package.name)
                .map(|package| {
                    package
                        .dependencies
                        .iter()
                        .map(|dependency| KirPackageDependency {
                            alias: dependency.alias.clone(),
                            package: dependency.package.clone(),
                            path: dependency.path.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        packages: project
            .project
            .packages
            .iter()
            .map(|package| KirPackageNode {
                name: package.name.clone(),
                version: package.version.clone(),
                entry_file: package.entry_file.clone(),
                entry_module: package.entry_module.clone(),
                is_root: package.is_root,
                dependencies: package
                    .dependencies
                    .iter()
                    .map(|dependency| KirPackageDependency {
                        alias: dependency.alias.clone(),
                        package: dependency.package.clone(),
                        path: dependency.path.clone(),
                    })
                    .collect(),
            })
            .collect(),
        source_hash: hash_project(
            &project.project.manifest.package.name,
            &project.project.manifest.package.version,
            &project.project.entry_file,
            &project
                .project
                .packages
                .iter()
                .map(|package| package.name.clone())
                .collect::<Vec<_>>(),
            &modules,
        ),
        modules,
    }
}

pub fn render_prompt(program: &KirProgram) -> String {
    let mut output = String::new();
    writeln!(output, "# Kairos System Context").expect("writing to string cannot fail");
    writeln!(output).expect("writing to string cannot fail");
    writeln!(output, "## Module").expect("writing to string cannot fail");
    if !program.package.is_empty() {
        writeln!(output, "- package: {}", program.package).expect("writing to string cannot fail");
    }
    writeln!(output, "- name: {}", program.module).expect("writing to string cannot fail");
    if !program.relative_path.is_empty() {
        writeln!(output, "- path: {}", program.relative_path)
            .expect("writing to string cannot fail");
    }
    writeln!(output, "- source_hash: {}", program.source_hash)
        .expect("writing to string cannot fail");
    if program.imports.is_empty() {
        writeln!(output, "- imports: none").expect("writing to string cannot fail");
    } else {
        writeln!(output, "- imports: {}", program.imports.join(", "))
            .expect("writing to string cannot fail");
    }
    writeln!(output).expect("writing to string cannot fail");

    writeln!(output, "## Context").expect("writing to string cannot fail");
    write_context_line(&mut output, program, "goal");
    write_context_line(&mut output, program, "audience");
    write_context_line(&mut output, program, "domain");

    if let Some(assumptions) = program.context.get("assumptions").and_then(Value::as_array) {
        writeln!(output, "## Assumptions").expect("writing to string cannot fail");
        if assumptions.is_empty() {
            writeln!(output, "- none").expect("writing to string cannot fail");
        } else {
            for assumption in assumptions {
                writeln!(output, "- {}", assumption.as_str().unwrap_or("<non-string assumption>"))
                    .expect("writing to string cannot fail");
            }
        }
        writeln!(output).expect("writing to string cannot fail");
    }

    writeln!(output, "## Types").expect("writing to string cannot fail");
    if program.schemas.is_empty() && program.enums.is_empty() && program.type_aliases.is_empty() {
        writeln!(output, "- none").expect("writing to string cannot fail");
    } else {
        for schema in &program.schemas {
            let fields = schema
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name, field.ty))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "- schema {} {{ {} }}", schema.name, fields)
                .expect("writing to string cannot fail");
        }
        for enum_decl in &program.enums {
            writeln!(output, "- enum {} = {}", enum_decl.name, enum_decl.variants.join(" | "))
                .expect("writing to string cannot fail");
        }
        for alias in &program.type_aliases {
            writeln!(output, "- type {} = {}", alias.name, alias.target)
                .expect("writing to string cannot fail");
        }
    }
    writeln!(output).expect("writing to string cannot fail");

    writeln!(output, "## Functions").expect("writing to string cannot fail");
    if program.functions.is_empty() {
        writeln!(output, "- none").expect("writing to string cannot fail");
    } else {
        for function in &program.functions {
            writeln!(
                output,
                "### {}({}) -> {}",
                function.name,
                function
                    .params
                    .iter()
                    .map(|param| format!("{}: {}", param.name, param.ty))
                    .collect::<Vec<_>>()
                    .join(", "),
                function.return_type
            )
            .expect("writing to string cannot fail");
            writeln!(output, "- describe: {}", function.metadata.describe)
                .expect("writing to string cannot fail");
            writeln!(
                output,
                "- tags: {}",
                if function.metadata.tags.is_empty() {
                    "(none)".to_string()
                } else {
                    function.metadata.tags.join(", ")
                }
            )
            .expect("writing to string cannot fail");

            writeln!(output, "- requires:").expect("writing to string cannot fail");
            if function.metadata.requires.is_empty() {
                writeln!(output, "  - none").expect("writing to string cannot fail");
            } else {
                for require in &function.metadata.requires {
                    writeln!(output, "  - {}", format_kir_expression(require))
                        .expect("writing to string cannot fail");
                }
            }

            writeln!(output, "- ensures:").expect("writing to string cannot fail");
            if function.metadata.ensures.is_empty() {
                writeln!(output, "  - none").expect("writing to string cannot fail");
            } else {
                for ensure in &function.metadata.ensures {
                    writeln!(output, "  - {}", format_kir_expression(ensure))
                        .expect("writing to string cannot fail");
                }
            }

            writeln!(output).expect("writing to string cannot fail");
        }
    }

    writeln!(output, "## Notes for Downstream LLMs").expect("writing to string cannot fail");
    writeln!(output, "- Inputs and exports are deterministic.")
        .expect("writing to string cannot fail");
    writeln!(output, "- Context keys outside the core set should be treated as custom metadata.")
        .expect("writing to string cannot fail");
    writeln!(
        output,
        "- KIR bodies preserve the executable subset for validation and interpretation."
    )
    .expect("writing to string cannot fail");

    output
}

pub fn render_project_prompt(project: &KirProject) -> String {
    let mut output = String::new();
    writeln!(output, "# Kairos Project Context").expect("writing to string cannot fail");
    writeln!(output).expect("writing to string cannot fail");
    writeln!(output, "## Package").expect("writing to string cannot fail");
    writeln!(output, "- name: {}", project.package.name).expect("writing to string cannot fail");
    writeln!(output, "- version: {}", project.package.version)
        .expect("writing to string cannot fail");
    writeln!(output, "- entry_file: {}", project.package.entry_file)
        .expect("writing to string cannot fail");
    writeln!(output, "- entry_module: {}", project.package.entry_module)
        .expect("writing to string cannot fail");
    if project.package.dependencies.is_empty() {
        writeln!(output, "- dependencies: none").expect("writing to string cannot fail");
    } else {
        writeln!(
            output,
            "- dependencies: {}",
            project
                .package
                .dependencies
                .iter()
                .map(|dependency| format!("{} -> {}", dependency.alias, dependency.package))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .expect("writing to string cannot fail");
    }
    writeln!(output, "- source_hash: {}", project.source_hash)
        .expect("writing to string cannot fail");
    writeln!(output).expect("writing to string cannot fail");

    writeln!(output, "## Package Graph").expect("writing to string cannot fail");
    for package in &project.packages {
        let role = if package.is_root { "root" } else { "dependency" };
        writeln!(output, "- {} ({role}) -> {}", package.name, package.entry_module)
            .expect("writing to string cannot fail");
    }
    writeln!(output).expect("writing to string cannot fail");

    writeln!(output, "## Modules").expect("writing to string cannot fail");
    for module in &project.modules {
        writeln!(output, "### {}", module.module).expect("writing to string cannot fail");
        if module.imports.is_empty() {
            writeln!(output, "- imports: none").expect("writing to string cannot fail");
        } else {
            writeln!(output, "- imports: {}", module.imports.join(", "))
                .expect("writing to string cannot fail");
        }

        let module_prompt = render_prompt(module);
        for line in module_prompt.lines().skip(2) {
            writeln!(output, "{line}").expect("writing to string cannot fail");
        }
        writeln!(output).expect("writing to string cannot fail");
    }

    writeln!(output, "## Notes for Downstream LLMs").expect("writing to string cannot fail");
    writeln!(
        output,
        "- Module imports are already validated for deterministic project resolution."
    )
    .expect("writing to string cannot fail");
    writeln!(
        output,
        "- Entry module context should be treated as the primary execution and prompt boundary."
    )
    .expect("writing to string cannot fail");

    output
}

fn write_context_line(output: &mut String, program: &KirProgram, key: &str) {
    let value =
        program.context.get(key).map(render_json_value).unwrap_or_else(|| "(missing)".to_string());
    writeln!(output, "- {key}: {value}").expect("writing to string cannot fail");
}

fn lower_context(program: &Program) -> BTreeMap<String, Value> {
    program
        .context
        .as_ref()
        .map(|context| {
            context
                .entries
                .iter()
                .map(|entry| (entry.key.clone(), lower_constant_expression(&entry.value)))
                .collect()
        })
        .unwrap_or_default()
}

fn lower_function(function: &kairos_ast::FunctionDecl) -> KirFunction {
    let describe = function.metadata.describe.clone().unwrap_or_default();
    let tags = function
        .metadata
        .tags
        .iter()
        .filter_map(|tag| match tag {
            Expression::Literal { value: Literal::String(value) } => Some(value.clone()),
            _ => None,
        })
        .collect();

    KirFunction {
        visibility: lower_visibility(function.visibility),
        is_test: function.is_test,
        name: function.name.clone(),
        return_type: format_type_ref(&function.return_type),
        params: function
            .params
            .iter()
            .map(|param| KirParam { name: param.name.clone(), ty: format_type_ref(&param.ty) })
            .collect(),
        metadata: KirMetadata {
            describe,
            tags,
            requires: function.metadata.requires.iter().map(lower_expression).collect(),
            ensures: function.metadata.ensures.iter().map(lower_expression).collect(),
        },
        body: function.body.statements.iter().map(lower_statement).collect(),
    }
}

fn lower_visibility(visibility: Visibility) -> KirVisibility {
    match visibility {
        Visibility::Public => KirVisibility::Public,
        Visibility::Internal => KirVisibility::Internal,
    }
}

fn lower_statement(statement: &Statement) -> KirStatement {
    match statement {
        Statement::Let { name, value } => {
            KirStatement::Let { name: name.clone(), value: lower_expression(value) }
        }
        Statement::Return { value } => KirStatement::Return { value: lower_expression(value) },
        Statement::If(if_statement) => KirStatement::If {
            condition: lower_expression(&if_statement.condition),
            then_branch: if_statement.then_branch.statements.iter().map(lower_statement).collect(),
            else_branch: if_statement.else_branch.as_ref().map(|branch| match branch {
                ElseBranch::Block(block) => KirElseBranch::Block {
                    statements: block.statements.iter().map(lower_statement).collect(),
                },
                ElseBranch::If(statement) => KirElseBranch::If {
                    statement: Box::new(lower_statement(&Statement::If((**statement).clone()))),
                },
            }),
        },
        Statement::Expr { expression } => {
            KirStatement::Expr { expression: lower_expression(expression) }
        }
    }
}

fn lower_expression(expression: &Expression) -> KirExpression {
    match expression {
        Expression::Literal { value } => KirExpression::Literal {
            value: match value {
                Literal::String(value) => KirLiteral::String(value.clone()),
                Literal::Integer(value) => KirLiteral::Integer(*value),
                Literal::Float(value) => KirLiteral::Float(*value),
                Literal::Boolean(value) => KirLiteral::Boolean(*value),
                Literal::Null => KirLiteral::Null,
            },
        },
        Expression::Identifier { name } => KirExpression::Identifier { name: name.clone() },
        Expression::Call { callee, args } => KirExpression::Call {
            callee: callee.clone(),
            args: args.iter().map(lower_expression).collect(),
        },
        Expression::List { items } => {
            KirExpression::List { items: items.iter().map(lower_expression).collect() }
        }
        Expression::Object { fields } => KirExpression::Object {
            fields: fields
                .iter()
                .map(|field| KirObjectField {
                    name: field.name.clone(),
                    value: lower_expression(&field.value),
                })
                .collect(),
        },
        Expression::Binary { operator, left, right } => KirExpression::Binary {
            operator: lower_binary_operator(*operator),
            left: Box::new(lower_expression(left)),
            right: Box::new(lower_expression(right)),
        },
    }
}

fn lower_binary_operator(operator: BinaryOperator) -> KirBinaryOperator {
    match operator {
        BinaryOperator::Add => KirBinaryOperator::Add,
        BinaryOperator::Subtract => KirBinaryOperator::Subtract,
        BinaryOperator::Multiply => KirBinaryOperator::Multiply,
        BinaryOperator::Divide => KirBinaryOperator::Divide,
        BinaryOperator::Equal => KirBinaryOperator::Equal,
        BinaryOperator::NotEqual => KirBinaryOperator::NotEqual,
        BinaryOperator::Greater => KirBinaryOperator::Greater,
        BinaryOperator::GreaterEqual => KirBinaryOperator::GreaterEqual,
        BinaryOperator::Less => KirBinaryOperator::Less,
        BinaryOperator::LessEqual => KirBinaryOperator::LessEqual,
        BinaryOperator::And => KirBinaryOperator::And,
        BinaryOperator::Or => KirBinaryOperator::Or,
    }
}

fn lower_constant_expression(expression: &Expression) -> Value {
    match expression {
        Expression::Literal { value } => match value {
            Literal::String(value) => Value::String(value.clone()),
            Literal::Integer(value) => Value::Number((*value).into()),
            Literal::Float(value) => {
                serde_json::Number::from_f64(*value).map(Value::Number).unwrap_or(Value::Null)
            }
            Literal::Boolean(value) => Value::Bool(*value),
            Literal::Null => Value::Null,
        },
        Expression::List { items } => {
            Value::Array(items.iter().map(lower_constant_expression).collect())
        }
        Expression::Object { fields } => {
            let mut map = serde_json::Map::new();
            for field in fields {
                map.insert(field.name.clone(), lower_constant_expression(&field.value));
            }
            Value::Object(map)
        }
        Expression::Identifier { .. } | Expression::Call { .. } | Expression::Binary { .. } => {
            Value::String(format_expression(expression))
        }
    }
}

fn render_json_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "null".to_string()),
    }
}

fn format_type_ref(ty: &TypeRef) -> String {
    ty.to_string()
}

pub fn format_kir_expression(expression: &KirExpression) -> String {
    let mut rendered = String::new();
    write_kir_expression(expression, 0, &mut rendered).expect("writing to string cannot fail");
    rendered
}

fn write_kir_expression(
    expression: &KirExpression,
    parent_precedence: u8,
    out: &mut String,
) -> fmt::Result {
    match expression {
        KirExpression::Literal { value } => write_kir_literal(value, out),
        KirExpression::Identifier { name } => out.write_str(name),
        KirExpression::Call { callee, args } => {
            out.write_str(callee)?;
            out.write_char('(')?;
            for (index, arg) in args.iter().enumerate() {
                if index > 0 {
                    out.write_str(", ")?;
                }
                write_kir_expression(arg, 0, out)?;
            }
            out.write_char(')')
        }
        KirExpression::List { items } => {
            out.write_char('[')?;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.write_str(", ")?;
                }
                write_kir_expression(item, 0, out)?;
            }
            out.write_char(']')
        }
        KirExpression::Object { fields } => {
            out.write_char('{')?;
            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    out.write_str(", ")?;
                }
                out.write_str(&field.name)?;
                out.write_str(": ")?;
                write_kir_expression(&field.value, 0, out)?;
            }
            out.write_char('}')
        }
        KirExpression::Binary { operator, left, right } => {
            let precedence = kir_precedence(*operator);
            let needs_parens = precedence < parent_precedence;
            if needs_parens {
                out.write_char('(')?;
            }
            write_kir_expression(left, precedence, out)?;
            write!(out, " {} ", kir_operator_symbol(*operator))?;
            write_kir_expression(right, precedence + 1, out)?;
            if needs_parens {
                out.write_char(')')?;
            }
            Ok(())
        }
    }
}

fn write_kir_literal(literal: &KirLiteral, out: &mut String) -> fmt::Result {
    match literal {
        KirLiteral::String(value) => {
            write!(out, "\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
        KirLiteral::Integer(value) => write!(out, "{value}"),
        KirLiteral::Float(value) => write!(out, "{value}"),
        KirLiteral::Boolean(value) => write!(out, "{value}"),
        KirLiteral::Null => out.write_str("null"),
    }
}

fn kir_precedence(operator: KirBinaryOperator) -> u8 {
    match operator {
        KirBinaryOperator::Or => 1,
        KirBinaryOperator::And => 2,
        KirBinaryOperator::Equal | KirBinaryOperator::NotEqual => 3,
        KirBinaryOperator::Greater
        | KirBinaryOperator::GreaterEqual
        | KirBinaryOperator::Less
        | KirBinaryOperator::LessEqual => 4,
        KirBinaryOperator::Add | KirBinaryOperator::Subtract => 5,
        KirBinaryOperator::Multiply | KirBinaryOperator::Divide => 6,
    }
}

fn kir_operator_symbol(operator: KirBinaryOperator) -> &'static str {
    match operator {
        KirBinaryOperator::Add => "+",
        KirBinaryOperator::Subtract => "-",
        KirBinaryOperator::Multiply => "*",
        KirBinaryOperator::Divide => "/",
        KirBinaryOperator::Equal => "==",
        KirBinaryOperator::NotEqual => "!=",
        KirBinaryOperator::Greater => ">",
        KirBinaryOperator::GreaterEqual => ">=",
        KirBinaryOperator::Less => "<",
        KirBinaryOperator::LessEqual => "<=",
        KirBinaryOperator::And => "&&",
        KirBinaryOperator::Or => "||",
    }
}

fn hash_source(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn hash_project(
    name: &str,
    version: &str,
    entry_file: &str,
    packages: &[String],
    modules: &[KirProgram],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update(version.as_bytes());
    hasher.update(entry_file.as_bytes());
    for package in packages {
        hasher.update(package.as_bytes());
    }
    for module in modules {
        hasher.update(module.package.as_bytes());
        hasher.update(module.module.as_bytes());
        hasher.update(module.source_hash.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kairos_parser::parse_source;
    use kairos_project::{analyze_project, load_project};
    use kairos_semantic::analyze;

    use super::{
        format_kir_expression, lower, lower_project, render_project_prompt, render_prompt,
        KirExpression,
    };

    fn fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../").join(path)
    }

    #[test]
    fn lowers_example_to_expected_ir_shape() {
        let program = parse_source(include_str!("../../../examples/video_context/src/main.kai"))
            .expect("example should parse");
        let analyzed = analyze(program).expect("example should analyze");
        let kir = lower(&analyzed);

        assert_eq!(kir.module, "demo.video_context");
        assert_eq!(kir.schemas.len(), 1);
        assert_eq!(kir.enums.len(), 1);
        assert_eq!(kir.functions.len(), 2);
        assert_eq!(
            kir.context["goal"],
            "Turn a technical video into a reusable system-context prompt"
        );
        assert_eq!(
            kir.functions[0].metadata.describe,
            "Return the number of canonical compilation stages described in the source"
        );
    }

    #[test]
    fn prompt_contains_contract_sections() {
        let program = parse_source(include_str!("../../../examples/hello_context/src/main.kai"))
            .expect("example should parse");
        let analyzed = analyze(program).expect("example should analyze");
        let prompt = render_prompt(&lower(&analyzed));

        assert!(prompt.contains("## Module"));
        assert!(prompt.contains("## Context"));
        assert!(prompt.contains("## Functions"));
        assert!(prompt.contains("## Notes for Downstream LLMs"));
    }

    #[test]
    fn formats_kir_expression_deterministically() {
        let expression = KirExpression::Binary {
            operator: super::KirBinaryOperator::Multiply,
            left: Box::new(KirExpression::Binary {
                operator: super::KirBinaryOperator::Add,
                left: Box::new(KirExpression::Literal { value: super::KirLiteral::Integer(1) }),
                right: Box::new(KirExpression::Literal { value: super::KirLiteral::Integer(2) }),
            }),
            right: Box::new(KirExpression::Identifier { name: "value".to_string() }),
        };

        assert_eq!(format_kir_expression(&expression), "(1 + 2) * value");
    }

    #[test]
    fn lowers_project_to_stable_shape() {
        let project =
            load_project(&fixture("examples/assistant_briefing")).expect("project should load");
        let analyzed = analyze_project(&project).expect("project should analyze");
        let kir = lower_project(&analyzed);

        assert_eq!(kir.package.name, "assistant_briefing");
        assert_eq!(kir.package.entry_module, "demo.assistant_briefing");
        assert_eq!(kir.modules.len(), 3);
        assert!(kir
            .modules
            .iter()
            .any(|module| module.module == "demo.assistant_briefing.briefing"));
    }

    #[test]
    fn project_prompt_contains_package_and_modules() {
        let project =
            load_project(&fixture("examples/decision_bundle")).expect("project should load");
        let analyzed = analyze_project(&project).expect("project should analyze");
        let prompt = render_project_prompt(&lower_project(&analyzed));

        assert!(prompt.contains("# Kairos Project Context"));
        assert!(prompt.contains("## Package"));
        assert!(prompt.contains("### demo.decision_bundle.scoring"));
        assert!(prompt.contains("## Notes for Downstream LLMs"));
    }

    #[test]
    fn lowers_local_dependency_project_with_package_graph() {
        let project =
            load_project(&fixture("examples/package_reuse_demo")).expect("project should load");
        let analyzed = analyze_project(&project).expect("project should analyze");
        let kir = lower_project(&analyzed);

        assert_eq!(kir.package.name, "package_reuse_demo");
        assert_eq!(kir.packages.len(), 2);
        assert!(kir.modules.iter().any(|module| module.module == "shared.rules_lib.api"));
        assert!(kir
            .modules
            .iter()
            .find(|module| module.module == "demo.package_reuse_demo")
            .is_some_and(|module| !module.import_bindings.is_empty()));
    }
}
