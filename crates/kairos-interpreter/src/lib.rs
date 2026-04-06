use std::{collections::BTreeMap, error::Error, fmt};

use kairos_ir::{
    format_kir_expression, KirElseBranch, KirExpression, KirFunction, KirLiteral, KirProgram,
    KirProject, KirStatement,
};
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, InterpreterError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<RuntimeValue>),
    Object(BTreeMap<String, RuntimeValue>),
    Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub function: String,
    pub value: RuntimeValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub module: String,
    pub results: Vec<ExecutionResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpreterError {
    message: String,
}

impl InterpreterError {
    fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for InterpreterError {}

pub fn run(
    program: &KirProgram,
    function: Option<&str>,
    args: &[RuntimeValue],
) -> Result<ExecutionReport> {
    let interpreter = Interpreter::new(program)?;

    let results = if let Some(function_name) = function {
        vec![ExecutionResult {
            function: function_name.to_string(),
            value: interpreter.call(function_name, args.to_vec())?,
        }]
    } else if let Some(main) = interpreter
        .program
        .functions
        .iter()
        .find(|function| function.name == "main" && function.params.is_empty())
    {
        vec![ExecutionResult {
            function: main.name.clone(),
            value: interpreter.call(&main.name, Vec::new())?,
        }]
    } else {
        let zero_arg_functions = interpreter
            .program
            .functions
            .iter()
            .filter(|function| function.params.is_empty())
            .map(|function| function.name.clone())
            .collect::<Vec<_>>();

        if zero_arg_functions.is_empty() {
            return Err(InterpreterError::new(
                "no zero-argument functions are available; pass `--function` and arguments",
            ));
        }

        let mut results = Vec::new();
        for function_name in zero_arg_functions {
            results.push(ExecutionResult {
                function: function_name.clone(),
                value: interpreter.call(&function_name, Vec::new())?,
            });
        }
        results
    };

    Ok(ExecutionReport { module: program.module.clone(), results })
}

pub fn run_project(
    project: &KirProject,
    default_module: &str,
    function: Option<&str>,
    args: &[RuntimeValue],
) -> Result<ExecutionReport> {
    let interpreter = ProjectInterpreter::new(project)?;

    let results = if let Some(function_name) = function {
        let (module_name, local_name) = split_qualified_function(function_name, default_module);
        vec![ExecutionResult {
            function: function_name.to_string(),
            value: interpreter.call_in_module(module_name, local_name, args.to_vec())?,
        }]
    } else if let Some(main) = interpreter
        .find_function(default_module, "main")
        .filter(|function| function.params.is_empty())
    {
        vec![ExecutionResult {
            function: "main".to_string(),
            value: interpreter.call_in_module(default_module, &main.name, Vec::new())?,
        }]
    } else {
        let zero_arg_functions = interpreter
            .module_functions(default_module)?
            .iter()
            .filter(|(_, function)| function.params.is_empty())
            .map(|(name, _)| (*name).to_string())
            .collect::<Vec<_>>();

        if zero_arg_functions.is_empty() {
            return Err(InterpreterError::new(
                "no zero-argument functions are available in the entry module; pass `--function` and arguments",
            ));
        }

        let mut results = Vec::new();
        for function_name in zero_arg_functions {
            results.push(ExecutionResult {
                function: function_name.clone(),
                value: interpreter.call_in_module(default_module, &function_name, Vec::new())?,
            });
        }
        results
    };

    Ok(ExecutionReport { module: default_module.to_string(), results })
}

struct Interpreter<'a> {
    program: &'a KirProgram,
    functions: BTreeMap<&'a str, &'a KirFunction>,
}

struct ProjectInterpreter<'a> {
    modules: BTreeMap<&'a str, &'a KirProgram>,
    functions: BTreeMap<(&'a str, &'a str), &'a KirFunction>,
}

impl<'a> Interpreter<'a> {
    fn new(program: &'a KirProgram) -> Result<Self> {
        let mut functions = BTreeMap::new();
        for function in &program.functions {
            if functions.insert(function.name.as_str(), function).is_some() {
                return Err(InterpreterError::new(format!(
                    "duplicate function `{}` in runtime program",
                    function.name
                )));
            }
        }
        Ok(Self { program, functions })
    }

    fn call(&self, name: &str, args: Vec<RuntimeValue>) -> Result<RuntimeValue> {
        let function = self
            .functions
            .get(name)
            .copied()
            .ok_or_else(|| InterpreterError::new(format!("unknown function `{name}`")))?;

        if function.params.len() != args.len() {
            return Err(InterpreterError::new(format!(
                "function `{name}` expects {} arguments but received {}",
                function.params.len(),
                args.len()
            )));
        }

        let mut env = BTreeMap::new();
        for (param, value) in function.params.iter().zip(args) {
            env.insert(param.name.clone(), value);
        }

        for require in &function.metadata.requires {
            match self.evaluate_expression(require, &mut env.clone())? {
                RuntimeValue::Boolean(true) => {}
                RuntimeValue::Boolean(false) => {
                    return Err(InterpreterError::new(format!(
                        "precondition failed for `{name}`: {}",
                        format_kir_expression(require)
                    )))
                }
                _ => {
                    return Err(InterpreterError::new(format!(
                        "precondition for `{name}` did not evaluate to Bool"
                    )))
                }
            }
        }

        let result = self.execute_block(&function.body, &mut env)?.ok_or_else(|| {
            InterpreterError::new(format!("function `{name}` completed without returning a value"))
        })?;

        let mut ensure_env = env;
        ensure_env.insert("result".to_string(), result.clone());
        for ensure in &function.metadata.ensures {
            match self.evaluate_expression(ensure, &mut ensure_env.clone())? {
                RuntimeValue::Boolean(true) => {}
                RuntimeValue::Boolean(false) => {
                    return Err(InterpreterError::new(format!(
                        "postcondition failed for `{name}`: {}",
                        format_kir_expression(ensure)
                    )))
                }
                _ => {
                    return Err(InterpreterError::new(format!(
                        "postcondition for `{name}` did not evaluate to Bool"
                    )))
                }
            }
        }

        Ok(result)
    }

    fn execute_block(
        &self,
        statements: &[KirStatement],
        env: &mut BTreeMap<String, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>> {
        for statement in statements {
            if let Some(value) = self.execute_statement(statement, env)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn execute_statement(
        &self,
        statement: &KirStatement,
        env: &mut BTreeMap<String, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>> {
        match statement {
            KirStatement::Let { name, value } => {
                let value = self.evaluate_expression(value, env)?;
                env.insert(name.clone(), value);
                Ok(None)
            }
            KirStatement::Return { value } => {
                let value = self.evaluate_expression(value, env)?;
                Ok(Some(value))
            }
            KirStatement::Expr { expression } => {
                self.evaluate_expression(expression, env)?;
                Ok(None)
            }
            KirStatement::If { condition, then_branch, else_branch } => {
                let condition = self.evaluate_expression(condition, env)?;
                match condition {
                    RuntimeValue::Boolean(true) => self.execute_branch(then_branch, env),
                    RuntimeValue::Boolean(false) => match else_branch {
                        Some(KirElseBranch::Block { statements }) => {
                            self.execute_branch(statements, env)
                        }
                        Some(KirElseBranch::If { statement }) => {
                            self.execute_statement(statement, env)
                        }
                        None => Ok(None),
                    },
                    _ => Err(InterpreterError::new("if condition did not evaluate to Bool")),
                }
            }
        }
    }

    fn execute_branch(
        &self,
        statements: &[KirStatement],
        env: &BTreeMap<String, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>> {
        let mut branch_env = env.clone();
        self.execute_block(statements, &mut branch_env)
    }

    fn evaluate_expression(
        &self,
        expression: &KirExpression,
        env: &mut BTreeMap<String, RuntimeValue>,
    ) -> Result<RuntimeValue> {
        match expression {
            KirExpression::Literal { value } => Ok(match value {
                KirLiteral::String(value) => RuntimeValue::String(value.clone()),
                KirLiteral::Integer(value) => RuntimeValue::Integer(*value),
                KirLiteral::Float(value) => RuntimeValue::Float(*value),
                KirLiteral::Boolean(value) => RuntimeValue::Boolean(*value),
                KirLiteral::Null => RuntimeValue::Null,
            }),
            KirExpression::Identifier { name } => env
                .get(name)
                .cloned()
                .ok_or_else(|| InterpreterError::new(format!("unknown identifier `{name}`"))),
            KirExpression::Call { callee, args } => {
                let values = args
                    .iter()
                    .map(|arg| self.evaluate_expression(arg, env))
                    .collect::<Result<Vec<_>>>()?;
                self.evaluate_call(callee, values)
            }
            KirExpression::List { items } => Ok(RuntimeValue::List(
                items
                    .iter()
                    .map(|item| self.evaluate_expression(item, env))
                    .collect::<Result<Vec<_>>>()?,
            )),
            KirExpression::Object { fields } => {
                let mut object = BTreeMap::new();
                for field in fields {
                    object.insert(field.name.clone(), self.evaluate_expression(&field.value, env)?);
                }
                Ok(RuntimeValue::Object(object))
            }
            KirExpression::Binary { operator, left, right } => {
                let left = self.evaluate_expression(left, env)?;
                let right = self.evaluate_expression(right, env)?;
                evaluate_binary(*operator, left, right)
            }
        }
    }

    fn evaluate_call(&self, callee: &str, args: Vec<RuntimeValue>) -> Result<RuntimeValue> {
        if let Some(value) = evaluate_builtin(callee, &args)? {
            Ok(value)
        } else {
            self.call(callee, args)
        }
    }
}

impl<'a> ProjectInterpreter<'a> {
    fn new(project: &'a KirProject) -> Result<Self> {
        let mut modules = BTreeMap::new();
        let mut functions = BTreeMap::new();

        for module in &project.modules {
            if modules.insert(module.module.as_str(), module).is_some() {
                return Err(InterpreterError::new(format!(
                    "duplicate module `{}` in runtime project",
                    module.module
                )));
            }

            for function in &module.functions {
                if functions
                    .insert((module.module.as_str(), function.name.as_str()), function)
                    .is_some()
                {
                    return Err(InterpreterError::new(format!(
                        "duplicate function `{}` in module `{}`",
                        function.name, module.module
                    )));
                }
            }
        }

        Ok(Self { modules, functions })
    }

    fn module_functions(&self, module: &str) -> Result<Vec<(&'a str, &'a KirFunction)>> {
        let program = self
            .modules
            .get(module)
            .copied()
            .ok_or_else(|| InterpreterError::new(format!("unknown module `{module}`")))?;
        Ok(program.functions.iter().map(|function| (function.name.as_str(), function)).collect())
    }

    fn find_function(&self, module: &str, function: &str) -> Option<&'a KirFunction> {
        self.functions.get(&(module, function)).copied()
    }

    fn call_in_module(
        &self,
        module: &str,
        name: &str,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue> {
        let function = self
            .find_function(module, name)
            .ok_or_else(|| InterpreterError::new(format!("unknown function `{module}::{name}`")))?;

        if function.params.len() != args.len() {
            return Err(InterpreterError::new(format!(
                "function `{module}::{name}` expects {} arguments but received {}",
                function.params.len(),
                args.len()
            )));
        }

        let mut env = BTreeMap::new();
        for (param, value) in function.params.iter().zip(args) {
            env.insert(param.name.clone(), value);
        }

        for require in &function.metadata.requires {
            match self.evaluate_expression_in_module(module, require, &mut env.clone())? {
                RuntimeValue::Boolean(true) => {}
                RuntimeValue::Boolean(false) => {
                    return Err(InterpreterError::new(format!(
                        "precondition failed for `{module}::{name}`: {}",
                        format_kir_expression(require)
                    )))
                }
                _ => {
                    return Err(InterpreterError::new(format!(
                        "precondition for `{module}::{name}` did not evaluate to Bool"
                    )))
                }
            }
        }

        let result =
            self.execute_block_in_module(module, &function.body, &mut env)?.ok_or_else(|| {
                InterpreterError::new(format!(
                    "function `{module}::{name}` completed without returning a value"
                ))
            })?;

        let mut ensure_env = env;
        ensure_env.insert("result".to_string(), result.clone());
        for ensure in &function.metadata.ensures {
            match self.evaluate_expression_in_module(module, ensure, &mut ensure_env.clone())? {
                RuntimeValue::Boolean(true) => {}
                RuntimeValue::Boolean(false) => {
                    return Err(InterpreterError::new(format!(
                        "postcondition failed for `{module}::{name}`: {}",
                        format_kir_expression(ensure)
                    )))
                }
                _ => {
                    return Err(InterpreterError::new(format!(
                        "postcondition for `{module}::{name}` did not evaluate to Bool"
                    )))
                }
            }
        }

        Ok(result)
    }

    fn execute_block_in_module(
        &self,
        module: &str,
        statements: &[KirStatement],
        env: &mut BTreeMap<String, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>> {
        for statement in statements {
            if let Some(value) = self.execute_statement_in_module(module, statement, env)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn execute_statement_in_module(
        &self,
        module: &str,
        statement: &KirStatement,
        env: &mut BTreeMap<String, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>> {
        match statement {
            KirStatement::Let { name, value } => {
                let value = self.evaluate_expression_in_module(module, value, env)?;
                env.insert(name.clone(), value);
                Ok(None)
            }
            KirStatement::Return { value } => {
                let value = self.evaluate_expression_in_module(module, value, env)?;
                Ok(Some(value))
            }
            KirStatement::Expr { expression } => {
                self.evaluate_expression_in_module(module, expression, env)?;
                Ok(None)
            }
            KirStatement::If { condition, then_branch, else_branch } => {
                let condition = self.evaluate_expression_in_module(module, condition, env)?;
                match condition {
                    RuntimeValue::Boolean(true) => {
                        self.execute_branch_in_module(module, then_branch, env)
                    }
                    RuntimeValue::Boolean(false) => match else_branch {
                        Some(KirElseBranch::Block { statements }) => {
                            self.execute_branch_in_module(module, statements, env)
                        }
                        Some(KirElseBranch::If { statement }) => {
                            self.execute_statement_in_module(module, statement, env)
                        }
                        None => Ok(None),
                    },
                    _ => Err(InterpreterError::new("if condition did not evaluate to Bool")),
                }
            }
        }
    }

    fn execute_branch_in_module(
        &self,
        module: &str,
        statements: &[KirStatement],
        env: &BTreeMap<String, RuntimeValue>,
    ) -> Result<Option<RuntimeValue>> {
        let mut branch_env = env.clone();
        self.execute_block_in_module(module, statements, &mut branch_env)
    }

    fn evaluate_expression_in_module(
        &self,
        module: &str,
        expression: &KirExpression,
        env: &mut BTreeMap<String, RuntimeValue>,
    ) -> Result<RuntimeValue> {
        match expression {
            KirExpression::Literal { value } => Ok(match value {
                KirLiteral::String(value) => RuntimeValue::String(value.clone()),
                KirLiteral::Integer(value) => RuntimeValue::Integer(*value),
                KirLiteral::Float(value) => RuntimeValue::Float(*value),
                KirLiteral::Boolean(value) => RuntimeValue::Boolean(*value),
                KirLiteral::Null => RuntimeValue::Null,
            }),
            KirExpression::Identifier { name } => env
                .get(name)
                .cloned()
                .ok_or_else(|| InterpreterError::new(format!("unknown identifier `{name}`"))),
            KirExpression::Call { callee, args } => {
                let values = args
                    .iter()
                    .map(|arg| self.evaluate_expression_in_module(module, arg, env))
                    .collect::<Result<Vec<_>>>()?;
                self.evaluate_call_in_module(module, callee, values)
            }
            KirExpression::List { items } => Ok(RuntimeValue::List(
                items
                    .iter()
                    .map(|item| self.evaluate_expression_in_module(module, item, env))
                    .collect::<Result<Vec<_>>>()?,
            )),
            KirExpression::Object { fields } => {
                let mut object = BTreeMap::new();
                for field in fields {
                    object.insert(
                        field.name.clone(),
                        self.evaluate_expression_in_module(module, &field.value, env)?,
                    );
                }
                Ok(RuntimeValue::Object(object))
            }
            KirExpression::Binary { operator, left, right } => {
                let left = self.evaluate_expression_in_module(module, left, env)?;
                let right = self.evaluate_expression_in_module(module, right, env)?;
                evaluate_binary(*operator, left, right)
            }
        }
    }

    fn evaluate_call_in_module(
        &self,
        module: &str,
        callee: &str,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue> {
        if let Some(value) = evaluate_builtin(callee, &args)? {
            return Ok(value);
        }

        if self.find_function(module, callee).is_some() {
            return self.call_in_module(module, callee, args);
        }

        let module_program = self
            .modules
            .get(module)
            .copied()
            .ok_or_else(|| InterpreterError::new(format!("unknown module `{module}`")))?;

        let imported_matches = self.resolve_imported_function_targets(module_program, callee);

        match imported_matches.as_slice() {
            [(imported_module, target_name)] => {
                self.call_in_module(imported_module, target_name, args)
            }
            [] => Err(InterpreterError::new(format!(
                "unknown function `{callee}` in module `{module}`"
            ))),
            _ => Err(InterpreterError::new(format!(
                "ambiguous imported function `{callee}` in module `{module}`"
            ))),
        }
    }

    fn resolve_imported_function_targets(
        &self,
        module_program: &'a KirProgram,
        callee: &str,
    ) -> Vec<(&'a str, String)> {
        if let Some((namespace, function_name)) = callee.split_once("::") {
            return module_program
                .import_bindings
                .iter()
                .filter(|binding| binding.alias.as_deref() == Some(namespace))
                .filter_map(|binding| {
                    self.find_function(&binding.module, function_name)
                        .map(|_| (binding.module.as_str(), function_name.to_string()))
                })
                .collect();
        }

        let selective_matches = module_program
            .import_bindings
            .iter()
            .flat_map(|binding| {
                binding.items.iter().filter_map(|item| {
                    let local_name = item.alias.as_deref().unwrap_or(&item.name);
                    (local_name == callee
                        && self.find_function(&binding.module, &item.name).is_some())
                    .then_some((binding.module.as_str(), item.name.clone()))
                })
            })
            .collect::<Vec<_>>();

        if !selective_matches.is_empty() {
            return selective_matches;
        }

        module_program
            .imports
            .iter()
            .filter(|imported_module| self.find_function(imported_module, callee).is_some())
            .map(|imported_module| (imported_module.as_str(), callee.to_string()))
            .collect()
    }
}

fn evaluate_binary(
    operator: kairos_ir::KirBinaryOperator,
    left: RuntimeValue,
    right: RuntimeValue,
) -> Result<RuntimeValue> {
    use kairos_ir::KirBinaryOperator as Operator;

    match operator {
        Operator::Add => numeric_op(left, right, |a, b| a + b, |a, b| a + b),
        Operator::Subtract => numeric_op(left, right, |a, b| a - b, |a, b| a - b),
        Operator::Multiply => numeric_op(left, right, |a, b| a * b, |a, b| a * b),
        Operator::Divide => numeric_op(left, right, |a, b| a / b, |a, b| a / b),
        Operator::Equal => Ok(RuntimeValue::Boolean(left == right)),
        Operator::NotEqual => Ok(RuntimeValue::Boolean(left != right)),
        Operator::Greater => compare_op(left, right, |a, b| a > b, |a, b| a > b),
        Operator::GreaterEqual => compare_op(left, right, |a, b| a >= b, |a, b| a >= b),
        Operator::Less => compare_op(left, right, |a, b| a < b, |a, b| a < b),
        Operator::LessEqual => compare_op(left, right, |a, b| a <= b, |a, b| a <= b),
        Operator::And => bool_op(left, right, |a, b| a && b),
        Operator::Or => bool_op(left, right, |a, b| a || b),
    }
}

fn split_qualified_function<'a>(function: &'a str, default_module: &'a str) -> (&'a str, &'a str) {
    if let Some((module, name)) = function.rsplit_once("::") {
        (module, name)
    } else {
        (default_module, function)
    }
}

fn evaluate_builtin(callee: &str, args: &[RuntimeValue]) -> Result<Option<RuntimeValue>> {
    let value = match callee {
        "len" => match args {
            [RuntimeValue::String(value)] => RuntimeValue::Integer(value.chars().count() as i64),
            [RuntimeValue::List(items)] => RuntimeValue::Integer(items.len() as i64),
            [_] => return Err(InterpreterError::new("`len` expects a string or list")),
            _ => return Err(InterpreterError::new("`len` expects exactly one argument")),
        },
        "concat" => match args {
            [RuntimeValue::String(left), RuntimeValue::String(right)] => {
                RuntimeValue::String(format!("{left}{right}"))
            }
            _ => return Err(InterpreterError::new("`concat` expects two strings")),
        },
        "abs" => match args {
            [RuntimeValue::Integer(value)] => RuntimeValue::Integer(value.abs()),
            _ => return Err(InterpreterError::new("`abs` expects one integer")),
        },
        "min" => match args {
            [RuntimeValue::Integer(left), RuntimeValue::Integer(right)] => {
                RuntimeValue::Integer((*left).min(*right))
            }
            _ => return Err(InterpreterError::new("`min` expects two integers")),
        },
        "max" => match args {
            [RuntimeValue::Integer(left), RuntimeValue::Integer(right)] => {
                RuntimeValue::Integer((*left).max(*right))
            }
            _ => return Err(InterpreterError::new("`max` expects two integers")),
        },
        "contains" => match args {
            [RuntimeValue::String(value), RuntimeValue::String(needle)] => {
                RuntimeValue::Boolean(value.contains(needle))
            }
            [RuntimeValue::List(items), needle] => RuntimeValue::Boolean(items.contains(needle)),
            [RuntimeValue::Object(object), RuntimeValue::String(key)] => {
                RuntimeValue::Boolean(object.contains_key(key))
            }
            _ => {
                return Err(InterpreterError::new(
                    "`contains` expects (Str, Str), (List, Any), or (Object, Str)",
                ))
            }
        },
        "starts_with" => match args {
            [RuntimeValue::String(value), RuntimeValue::String(prefix)] => {
                RuntimeValue::Boolean(value.starts_with(prefix))
            }
            _ => return Err(InterpreterError::new("`starts_with` expects two strings")),
        },
        "ends_with" => match args {
            [RuntimeValue::String(value), RuntimeValue::String(suffix)] => {
                RuntimeValue::Boolean(value.ends_with(suffix))
            }
            _ => return Err(InterpreterError::new("`ends_with` expects two strings")),
        },
        "trim" => match args {
            [RuntimeValue::String(value)] => RuntimeValue::String(value.trim().to_string()),
            _ => return Err(InterpreterError::new("`trim` expects one string")),
        },
        "upper" => match args {
            [RuntimeValue::String(value)] => RuntimeValue::String(value.to_uppercase()),
            _ => return Err(InterpreterError::new("`upper` expects one string")),
        },
        "lower" => match args {
            [RuntimeValue::String(value)] => RuntimeValue::String(value.to_lowercase()),
            _ => return Err(InterpreterError::new("`lower` expects one string")),
        },
        "join" => match args {
            [RuntimeValue::List(items), RuntimeValue::String(separator)] => {
                let mut rendered = Vec::with_capacity(items.len());
                for item in items {
                    let RuntimeValue::String(value) = item else {
                        return Err(InterpreterError::new("`join` expects List<Str> and Str"));
                    };
                    rendered.push(value.clone());
                }
                RuntimeValue::String(rendered.join(separator))
            }
            _ => return Err(InterpreterError::new("`join` expects List<Str> and Str")),
        },
        "first" => match args {
            [RuntimeValue::List(items)] => items.first().cloned().unwrap_or(RuntimeValue::Null),
            _ => return Err(InterpreterError::new("`first` expects one list")),
        },
        "last" => match args {
            [RuntimeValue::List(items)] => items.last().cloned().unwrap_or(RuntimeValue::Null),
            _ => return Err(InterpreterError::new("`last` expects one list")),
        },
        "all" => match args {
            [RuntimeValue::List(items)] => {
                let mut result = true;
                for item in items {
                    let RuntimeValue::Boolean(value) = item else {
                        return Err(InterpreterError::new("`all` expects List<Bool>"));
                    };
                    result &= *value;
                }
                RuntimeValue::Boolean(result)
            }
            _ => return Err(InterpreterError::new("`all` expects one list")),
        },
        "any" => match args {
            [RuntimeValue::List(items)] => {
                let mut result = false;
                for item in items {
                    let RuntimeValue::Boolean(value) = item else {
                        return Err(InterpreterError::new("`any` expects List<Bool>"));
                    };
                    result |= *value;
                }
                RuntimeValue::Boolean(result)
            }
            _ => return Err(InterpreterError::new("`any` expects one list")),
        },
        "has_key" => match args {
            [RuntimeValue::Object(object), RuntimeValue::String(key)] => {
                RuntimeValue::Boolean(object.contains_key(key))
            }
            _ => return Err(InterpreterError::new("`has_key` expects Object and Str")),
        },
        "get_str" => match args {
            [RuntimeValue::Object(object), RuntimeValue::String(key)] => match object.get(key) {
                Some(RuntimeValue::String(value)) => RuntimeValue::String(value.clone()),
                Some(_) => {
                    return Err(InterpreterError::new(
                        "`get_str` found a non-string value for the requested key",
                    ))
                }
                None => RuntimeValue::Null,
            },
            _ => return Err(InterpreterError::new("`get_str` expects Object and Str")),
        },
        "get_int" => match args {
            [RuntimeValue::Object(object), RuntimeValue::String(key)] => match object.get(key) {
                Some(RuntimeValue::Integer(value)) => RuntimeValue::Integer(*value),
                Some(_) => {
                    return Err(InterpreterError::new(
                        "`get_int` found a non-integer value for the requested key",
                    ))
                }
                None => RuntimeValue::Null,
            },
            _ => return Err(InterpreterError::new("`get_int` expects Object and Str")),
        },
        "get_bool" => match args {
            [RuntimeValue::Object(object), RuntimeValue::String(key)] => match object.get(key) {
                Some(RuntimeValue::Boolean(value)) => RuntimeValue::Boolean(*value),
                Some(_) => {
                    return Err(InterpreterError::new(
                        "`get_bool` found a non-boolean value for the requested key",
                    ))
                }
                None => RuntimeValue::Null,
            },
            _ => return Err(InterpreterError::new("`get_bool` expects Object and Str")),
        },
        "get_list" => match args {
            [RuntimeValue::Object(object), RuntimeValue::String(key)] => match object.get(key) {
                Some(RuntimeValue::List(value)) => RuntimeValue::List(value.clone()),
                Some(_) => {
                    return Err(InterpreterError::new(
                        "`get_list` found a non-list value for the requested key",
                    ))
                }
                None => RuntimeValue::Null,
            },
            _ => return Err(InterpreterError::new("`get_list` expects Object and Str")),
        },
        "get_obj" => match args {
            [RuntimeValue::Object(object), RuntimeValue::String(key)] => match object.get(key) {
                Some(RuntimeValue::Object(value)) => RuntimeValue::Object(value.clone()),
                Some(_) => {
                    return Err(InterpreterError::new(
                        "`get_obj` found a non-object value for the requested key",
                    ))
                }
                None => RuntimeValue::Null,
            },
            _ => return Err(InterpreterError::new("`get_obj` expects Object and Str")),
        },
        "keys" => match args {
            [RuntimeValue::Object(object)] => RuntimeValue::List(
                object.keys().map(|key| RuntimeValue::String(key.clone())).collect(),
            ),
            _ => return Err(InterpreterError::new("`keys` expects one object")),
        },
        "count" => match args {
            [RuntimeValue::List(items)] => RuntimeValue::Integer(items.len() as i64),
            _ => return Err(InterpreterError::new("`count` expects one list")),
        },
        "sort" => match args {
            [RuntimeValue::List(items)] => RuntimeValue::List(sort_runtime_values(items)?),
            _ => return Err(InterpreterError::new("`sort` expects one list")),
        },
        "unique" => match args {
            [RuntimeValue::List(items)] => RuntimeValue::List(unique_runtime_values(items)),
            _ => return Err(InterpreterError::new("`unique` expects one list")),
        },
        "normalize_space" => match args {
            [RuntimeValue::String(value)] => {
                RuntimeValue::String(value.split_whitespace().collect::<Vec<_>>().join(" "))
            }
            _ => return Err(InterpreterError::new("`normalize_space` expects one string")),
        },
        "clamp" => match args {
            [RuntimeValue::Integer(value), RuntimeValue::Integer(minimum), RuntimeValue::Integer(maximum)] => {
                RuntimeValue::Integer((*value).clamp(*minimum, *maximum))
            }
            _ => return Err(InterpreterError::new("`clamp` expects three integers")),
        },
        _ => return Ok(None),
    };

    Ok(Some(value))
}

fn numeric_op(
    left: RuntimeValue,
    right: RuntimeValue,
    int_op: impl FnOnce(i64, i64) -> i64,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> Result<RuntimeValue> {
    match (left, right) {
        (RuntimeValue::Integer(left), RuntimeValue::Integer(right)) => {
            Ok(RuntimeValue::Integer(int_op(left, right)))
        }
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => {
            Ok(RuntimeValue::Float(float_op(left, right)))
        }
        _ => Err(InterpreterError::new("numeric operator expects matching numeric operands")),
    }
}

fn compare_op(
    left: RuntimeValue,
    right: RuntimeValue,
    int_op: impl FnOnce(i64, i64) -> bool,
    float_op: impl FnOnce(f64, f64) -> bool,
) -> Result<RuntimeValue> {
    match (left, right) {
        (RuntimeValue::Integer(left), RuntimeValue::Integer(right)) => {
            Ok(RuntimeValue::Boolean(int_op(left, right)))
        }
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => {
            Ok(RuntimeValue::Boolean(float_op(left, right)))
        }
        _ => Err(InterpreterError::new("comparison operator expects matching numeric operands")),
    }
}

fn bool_op(
    left: RuntimeValue,
    right: RuntimeValue,
    op: impl FnOnce(bool, bool) -> bool,
) -> Result<RuntimeValue> {
    match (left, right) {
        (RuntimeValue::Boolean(left), RuntimeValue::Boolean(right)) => {
            Ok(RuntimeValue::Boolean(op(left, right)))
        }
        _ => Err(InterpreterError::new("boolean operator expects Bool operands")),
    }
}

fn sort_runtime_values(items: &[RuntimeValue]) -> Result<Vec<RuntimeValue>> {
    if items.iter().all(|item| matches!(item, RuntimeValue::Integer(_))) {
        let mut values = items
            .iter()
            .map(|item| match item {
                RuntimeValue::Integer(value) => Ok(*value),
                _ => Err(InterpreterError::new("`sort` expects a homogeneous list")),
            })
            .collect::<Result<Vec<_>>>()?;
        values.sort();
        return Ok(values.into_iter().map(RuntimeValue::Integer).collect());
    }

    if items.iter().all(|item| matches!(item, RuntimeValue::Float(_))) {
        let mut values = items
            .iter()
            .map(|item| match item {
                RuntimeValue::Float(value) => Ok(*value),
                _ => Err(InterpreterError::new("`sort` expects a homogeneous list")),
            })
            .collect::<Result<Vec<_>>>()?;
        values.sort_by(f64::total_cmp);
        return Ok(values.into_iter().map(RuntimeValue::Float).collect());
    }

    if items.iter().all(|item| matches!(item, RuntimeValue::String(_))) {
        let mut values = items
            .iter()
            .map(|item| match item {
                RuntimeValue::String(value) => Ok(value.clone()),
                _ => Err(InterpreterError::new("`sort` expects a homogeneous list")),
            })
            .collect::<Result<Vec<_>>>()?;
        values.sort();
        return Ok(values.into_iter().map(RuntimeValue::String).collect());
    }

    Err(InterpreterError::new("`sort` expects a homogeneous List<Int>, List<Float>, or List<Str>"))
}

fn unique_runtime_values(items: &[RuntimeValue]) -> Vec<RuntimeValue> {
    let mut unique = Vec::new();
    for item in items {
        if !unique.iter().any(|existing| existing == item) {
            unique.push(item.clone());
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kairos_ir::lower;
    use kairos_parser::parse_source;
    use kairos_project::{analyze_project, load_project};
    use kairos_semantic::analyze;

    use super::{run, run_project, RuntimeValue};

    fn fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../").join(path)
    }

    #[test]
    fn runs_zero_arg_example_functions() {
        let program = parse_source(include_str!("../../../examples/hello_context/src/main.kai"))
            .expect("example should parse");
        let analyzed = analyze(program).expect("example should analyze");
        let report = run(&lower(&analyzed), None, &[]).expect("example should run");

        assert_eq!(report.module, "demo.hello_context");
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].value, RuntimeValue::String("Hello from Kairos".to_string()));
    }

    #[test]
    fn runs_function_with_arguments() {
        let program = parse_source(include_str!("../../../examples/risk_rules/src/main.kai"))
            .expect("example should parse");
        let analyzed = analyze(program).expect("example should analyze");
        let report = run(&lower(&analyzed), Some("classify"), &[RuntimeValue::Integer(72)])
            .expect("function should run");

        assert_eq!(report.results[0].value, RuntimeValue::String("MEDIUM".to_string()));
    }

    #[test]
    fn enforces_preconditions() {
        let program = parse_source(include_str!("../../../examples/risk_rules/src/main.kai"))
            .expect("example should parse");
        let analyzed = analyze(program).expect("example should analyze");
        let error = run(&lower(&analyzed), Some("classify"), &[RuntimeValue::Integer(101)])
            .expect_err("precondition should fail");

        assert!(error.to_string().contains("precondition failed"));
    }

    #[test]
    fn runs_multifile_project_entry() {
        let project =
            load_project(&fixture("examples/decision_bundle")).expect("project should load");
        let analyzed = analyze_project(&project).expect("project should analyze");
        let report = run_project(
            &kairos_ir::lower_project(&analyzed),
            &analyzed.project.entry_module,
            Some("classify"),
            &[RuntimeValue::Integer(72)],
        )
        .expect("project function should run");

        assert_eq!(report.module, "demo.decision_bundle");
        assert_eq!(report.results[0].value, RuntimeValue::String("MEDIUM".to_string()));
    }
}
