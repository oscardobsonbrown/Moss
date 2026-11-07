mod functions;
mod parser;

use crate::{EngineError, error};
use parser::{Expr, Parser};
use rotion_core::model::Property;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Values are keyed by property ID. `prop` accepts a property name or ID.
pub fn evaluate(
    source: &str,
    values: &BTreeMap<String, Value>,
    properties: &[Property],
) -> Result<Value, EngineError> {
    let expr = Parser::parse(source)?;
    eval(&expr, values, properties)
}

fn eval(
    expr: &Expr,
    values: &BTreeMap<String, Value>,
    properties: &[Property],
) -> Result<Value, EngineError> {
    match expr {
        Expr::Value(v) => Ok(v.clone()),
        Expr::List(items) => items
            .iter()
            .map(|x| eval(x, values, properties))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Expr::Unary(op, inner) => {
            let v = eval(inner, values, properties)?;
            match op.as_str() {
                "-" => number(-num(&v)?),
                "!" | "not" => Ok(json!(!boolean(&v)?)),
                _ => Err(error("Unknown unary operator")),
            }
        }
        Expr::Binary(op, a, b) => {
            let a = eval(a, values, properties)?;
            if (op == "and" || op == "&&") && !boolean(&a)? {
                return Ok(json!(false));
            }
            if (op == "or" || op == "||") && boolean(&a)? {
                return Ok(json!(true));
            }
            let b = eval(b, values, properties)?;
            match op.as_str() {
                "+" if a.is_string() && b.is_string() => Ok(json!(format!(
                    "{}{}",
                    a.as_str().unwrap(),
                    b.as_str().unwrap()
                ))),
                "+" => number(num(&a)? + num(&b)?),
                "-" => number(num(&a)? - num(&b)?),
                "*" => number(num(&a)? * num(&b)?),
                "/" | "%" => {
                    let d = num(&b)?;
                    if d == 0.0 {
                        return Err(error("Division by zero"));
                    }
                    number(if op == "/" {
                        num(&a)? / d
                    } else {
                        num(&a)? % d
                    })
                }
                "==" => Ok(json!(equal(&a, &b))),
                "!=" => Ok(json!(!equal(&a, &b))),
                ">" | ">=" | "<" | "<=" => {
                    let ord = compare(&a, &b)?;
                    Ok(json!(match op.as_str() {
                        ">" => ord.is_gt(),
                        ">=" => !ord.is_lt(),
                        "<" => ord.is_lt(),
                        _ => !ord.is_gt(),
                    }))
                }
                "and" | "&&" | "or" | "||" => Ok(json!(boolean(&b)?)),
                _ => Err(error(format!("Unknown operator {op}"))),
            }
        }
        Expr::Call(name, args) => {
            if name == "if" || name == "ifs" {
                if (name == "if" && args.len() != 3)
                    || (name == "ifs" && (args.len() < 3 || args.len() % 2 == 0))
                {
                    return Err(error(format!("Invalid arguments for {name}")));
                }
                for pair in args[..args.len() - 1].chunks_exact(2) {
                    if boolean(&eval(&pair[0], values, properties)?)? {
                        return eval(&pair[1], values, properties);
                    }
                }
                return eval(args.last().unwrap(), values, properties);
            }
            if name == "and" || name == "or" {
                if args.is_empty() {
                    return Err(error(format!("{name} requires at least one argument")));
                }
                for arg in args {
                    let result = boolean(&eval(arg, values, properties)?)?;
                    if (name == "and" && !result) || (name == "or" && result) {
                        return Ok(json!(result));
                    }
                }
                return Ok(json!(name == "and"));
            }
            if name == "prop" {
                if args.len() != 1 {
                    return Err(error("prop requires one argument"));
                }
                let key = eval(&args[0], values, properties)?;
                let key = key
                    .as_str()
                    .ok_or_else(|| error("prop requires a string"))?;
                let id = if let Some(property) = properties.iter().find(|p| p.id == key) {
                    property.id.as_str()
                } else {
                    let mut named = properties.iter().filter(|p| p.name == key);
                    if let Some(property) = named.next() {
                        if named.next().is_some() {
                            return Err(error(format!("Ambiguous property name: {key}")));
                        }
                        property.id.as_str()
                    } else if values.contains_key(key) {
                        key
                    } else {
                        return Err(error(format!("Unknown property: {key}")));
                    }
                };
                return Ok(values.get(id).cloned().unwrap_or(Value::Null));
            }
            let evaluated = args
                .iter()
                .map(|a| eval(a, values, properties))
                .collect::<Result<Vec<_>, _>>()?;
            functions::call(name, &evaluated)
        }
    }
}
fn num(v: &Value) -> Result<f64, EngineError> {
    v.as_f64().ok_or_else(|| error("Expected number"))
}
fn boolean(v: &Value) -> Result<bool, EngineError> {
    v.as_bool().ok_or_else(|| error("Expected boolean"))
}
fn number(v: f64) -> Result<Value, EngineError> {
    serde_json::Number::from_f64(v)
        .map(Value::Number)
        .ok_or_else(|| error("Non-finite number"))
}
fn compare(a: &Value, b: &Value) -> Result<std::cmp::Ordering, EngineError> {
    if let (Some(a), Some(b)) = (a.as_f64(), b.as_f64()) {
        return a.partial_cmp(&b).ok_or_else(|| error("Invalid comparison"));
    }
    if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
        return Ok(a.cmp(b));
    }
    Err(error("Values cannot be compared"))
}

fn equal(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => a == b,
        _ => a == b,
    }
}
