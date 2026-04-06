use std::fmt::Write as _;

use kairos_ast::{
    format_expression, format_use_decl, Block, ElseBranch, Expression, FunctionDecl, Program,
    Statement, Visibility,
};

const INDENT: &str = "  ";

pub fn format_program(program: &Program) -> String {
    let mut sections = Vec::new();

    let mut header = String::new();
    writeln!(header, "module {};", program.module).expect("writing to string cannot fail");
    for import in imports_for_formatting(program) {
        writeln!(header, "{}", format_use_decl(&import)).expect("writing to string cannot fail");
    }
    sections.push(header.trim_end().to_string());

    if let Some(context) = &program.context {
        let mut section = String::new();
        writeln!(section, "context {{").expect("writing to string cannot fail");
        for entry in &context.entries {
            let rendered = format_value_expression(&entry.value, 1);
            writeln!(section, "{INDENT}{}: {rendered};", entry.key)
                .expect("writing to string cannot fail");
        }
        write!(section, "}}").expect("writing to string cannot fail");
        sections.push(section);
    }

    for schema in &program.schemas {
        let mut section = String::new();
        writeln!(section, "{}schema {} {{", visibility_prefix(schema.visibility), schema.name)
            .expect("writing to string cannot fail");
        for field in &schema.fields {
            writeln!(section, "{INDENT}{}: {},", field.name, field.ty)
                .expect("writing to string cannot fail");
        }
        write!(section, "}}").expect("writing to string cannot fail");
        sections.push(section);
    }

    for enum_decl in &program.enums {
        let mut section = String::new();
        writeln!(section, "{}enum {} {{", visibility_prefix(enum_decl.visibility), enum_decl.name)
            .expect("writing to string cannot fail");
        for variant in &enum_decl.variants {
            writeln!(section, "{INDENT}{variant},").expect("writing to string cannot fail");
        }
        write!(section, "}}").expect("writing to string cannot fail");
        sections.push(section);
    }

    for alias in &program.type_aliases {
        sections.push(format!(
            "{}type {} = {};",
            visibility_prefix(alias.visibility),
            alias.name,
            alias.target
        ));
    }

    for function in &program.functions {
        sections.push(format_function(function));
    }

    format!("{}\n", sections.join("\n\n"))
}

fn format_function(function: &FunctionDecl) -> String {
    let mut section = String::new();
    let mut modifiers = String::new();
    modifiers.push_str(visibility_prefix(function.visibility));
    if function.is_test {
        modifiers.push_str("test ");
    }
    writeln!(
        section,
        "{}fn {}({}) -> {}",
        modifiers,
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

    if let Some(describe) = &function.metadata.describe {
        writeln!(section, "describe {}", quoted(describe)).expect("writing to string cannot fail");
    }
    writeln!(section, "tags {}", format_metadata_list(&function.metadata.tags, 0))
        .expect("writing to string cannot fail");
    writeln!(section, "requires {}", format_metadata_list(&function.metadata.requires, 0))
        .expect("writing to string cannot fail");
    writeln!(section, "ensures {}", format_metadata_list(&function.metadata.ensures, 0))
        .expect("writing to string cannot fail");
    write!(section, "{}", format_block(&function.body, 0)).expect("writing to string cannot fail");
    section
}

fn imports_for_formatting(program: &Program) -> Vec<kairos_ast::UseDecl> {
    if !program.imports.is_empty() {
        program.imports.clone()
    } else {
        program
            .uses
            .iter()
            .map(|module| kairos_ast::UseDecl {
                module: module.clone(),
                alias: None,
                items: Vec::new(),
            })
            .collect()
    }
}

fn visibility_prefix(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "pub ",
        Visibility::Internal => "",
    }
}

fn format_block(block: &Block, indent_level: usize) -> String {
    let mut rendered = String::new();
    writeln!(rendered, "{{").expect("writing to string cannot fail");
    for statement in &block.statements {
        writeln!(
            rendered,
            "{}{}",
            INDENT.repeat(indent_level + 1),
            format_statement(statement, indent_level + 1)
        )
        .expect("writing to string cannot fail");
    }
    write!(rendered, "{}}}", INDENT.repeat(indent_level)).expect("writing to string cannot fail");
    rendered
}

fn format_statement(statement: &Statement, indent_level: usize) -> String {
    match statement {
        Statement::Let { name, value } => {
            format!("let {name} = {};", format_value_expression(value, indent_level))
        }
        Statement::Return { value } => {
            format!("return {};", format_value_expression(value, indent_level))
        }
        Statement::Expr { expression } => {
            format!("{};", format_value_expression(expression, indent_level))
        }
        Statement::If(if_statement) => {
            let mut rendered = String::new();
            write!(
                rendered,
                "if {} {}",
                format_expression(&if_statement.condition),
                format_block(&if_statement.then_branch, indent_level)
            )
            .expect("writing to string cannot fail");

            if let Some(else_branch) = &if_statement.else_branch {
                match else_branch {
                    ElseBranch::Block(block) => {
                        write!(rendered, " else {}", format_block(block, indent_level))
                            .expect("writing to string cannot fail");
                    }
                    ElseBranch::If(nested) => {
                        write!(
                            rendered,
                            " else {}",
                            format_statement(&Statement::If((**nested).clone()), indent_level)
                        )
                        .expect("writing to string cannot fail");
                    }
                }
            }

            rendered
        }
    }
}

fn format_metadata_list(items: &[Expression], indent_level: usize) -> String {
    format_list(items, indent_level, true)
}

fn format_value_expression(expression: &Expression, indent_level: usize) -> String {
    match expression {
        Expression::List { items } => format_list(items, indent_level, false),
        Expression::Object { fields } => {
            let inline = format_expression(expression);
            if inline.len() <= 40
                && fields.iter().all(|field| {
                    !matches!(field.value, Expression::List { .. } | Expression::Object { .. })
                })
            {
                inline
            } else {
                let mut rendered = String::new();
                writeln!(rendered, "{{").expect("writing to string cannot fail");
                for field in fields {
                    writeln!(
                        rendered,
                        "{}{}: {},",
                        INDENT.repeat(indent_level + 1),
                        field.name,
                        format_value_expression(&field.value, indent_level + 1)
                    )
                    .expect("writing to string cannot fail");
                }
                write!(rendered, "{}}}", INDENT.repeat(indent_level))
                    .expect("writing to string cannot fail");
                rendered
            }
        }
        _ => format_expression(expression),
    }
}

fn format_list(items: &[Expression], indent_level: usize, prefer_inline: bool) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }

    let inline_items: Vec<String> = items.iter().map(format_expression).collect();
    let inline = format!("[{}]", inline_items.join(", "));
    let fits_inline = prefer_inline && inline.len() <= 48
        || inline.len() <= 36
            && items
                .iter()
                .all(|item| !matches!(item, Expression::List { .. } | Expression::Object { .. }));

    if fits_inline {
        inline
    } else {
        let mut rendered = String::new();
        writeln!(rendered, "[").expect("writing to string cannot fail");
        for item in items {
            writeln!(
                rendered,
                "{}{},",
                INDENT.repeat(indent_level + 1),
                format_value_expression(item, indent_level + 1)
            )
            .expect("writing to string cannot fail");
        }
        write!(rendered, "{}]", INDENT.repeat(indent_level))
            .expect("writing to string cannot fail");
        rendered
    }
}

fn quoted(value: &str) -> String {
    format!("{:?}", value)
}

#[cfg(test)]
mod tests {
    use kairos_parser::parse_source;

    use super::format_program;

    #[test]
    fn formats_example_with_canonical_sections() {
        let program = parse_source(include_str!("../../../examples/hello_context/src/main.kai"))
            .expect("example should parse");
        let formatted = format_program(&program);

        assert!(formatted.starts_with("module demo.hello_context;"));
        assert!(formatted.contains("context {\n"));
        assert!(formatted.contains("tags [\"demo\", \"hello\"]"));
        assert!(formatted.ends_with("}\n"));
    }

    #[test]
    fn formatting_round_trips_through_parser() {
        let program = parse_source(include_str!("../../../examples/risk_rules/src/main.kai"))
            .expect("example should parse");
        let formatted = format_program(&program);
        let reparsed = parse_source(&formatted).expect("formatted source should parse");

        assert_eq!(reparsed.module, program.module);
        assert_eq!(reparsed.functions.len(), program.functions.len());
        assert_eq!(
            reparsed.context.as_ref().map(|ctx| ctx.entries.len()),
            program.context.as_ref().map(|ctx| ctx.entries.len())
        );
    }
}
