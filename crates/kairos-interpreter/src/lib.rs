use std::{collections::BTreeMap, error::Error, fmt};

use kairos_ir::{
    format_kir_expression, KirElseBranch, KirExpression, KirFunction, KirLiteral, KirProgram,
    KirStatement,
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

struct Interpreter<'a> {
    program: &'a KirProgram,
    functions: BTreeMap<&'a str, &'a KirFunction>,
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
        match callee {
            "len" => match args.as_slice() {
                [RuntimeValue::String(value)] => {
                    Ok(RuntimeValue::Integer(value.chars().count() as i64))
                }
                [RuntimeValue::List(items)] => Ok(RuntimeValue::Integer(items.len() as i64)),
                [_] => Err(InterpreterError::new("`len` expects a string or list")),
                _ => Err(InterpreterError::new("`len` expects exactly one argument")),
            },
            "concat" => match args.as_slice() {
                [RuntimeValue::String(left), RuntimeValue::String(right)] => {
                    Ok(RuntimeValue::String(format!("{left}{right}")))
                }
                _ => Err(InterpreterError::new("`concat` expects two strings")),
            },
            "abs" => match args.as_slice() {
                [RuntimeValue::Integer(value)] => Ok(RuntimeValue::Integer(value.abs())),
                _ => Err(InterpreterError::new("`abs` expects one integer")),
            },
            "min" => match args.as_slice() {
                [RuntimeValue::Integer(left), RuntimeValue::Integer(right)] => {
                    Ok(RuntimeValue::Integer((*left).min(*right)))
                }
                _ => Err(InterpreterError::new("`min` expects two integers")),
            },
            "max" => match args.as_slice() {
                [RuntimeValue::Integer(left), RuntimeValue::Integer(right)] => {
                    Ok(RuntimeValue::Integer((*left).max(*right)))
                }
                _ => Err(InterpreterError::new("`max` expects two integers")),
            },
            _ => self.call(callee, args),
        }
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

#[cfg(test)]
mod tests {
    use kairos_ir::lower;
    use kairos_parser::parse_source;
    use kairos_semantic::analyze;

    use super::{run, RuntimeValue};

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
}
