mod functions;
mod parser;

use crate::model::Property;
use crate::{EngineError, error};
use parser::{Expr, Parser};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Validate formula syntax without evaluating values or mutating data.
pub fn validate_formula(source: &str) -> Result<(), EngineError> {
    Parser::parse(source).map(|_| ())
}

/// Resolve static property references and reject unsupported or dynamic calls.
pub fn dependencies(source: &str, properties: &[Property]) -> Result<Vec<String>, EngineError> {
    fn walk(
        expr: &Expr,
        properties: &[Property],
        out: &mut Vec<String>,
    ) -> Result<(), EngineError> {
        match expr {
            Expr::Value(_) | Expr::Current | Expr::Index => {}
            Expr::List(items) => {
                for item in items {
                    walk(item, properties, out)?;
                }
            }
            Expr::Unary(_, item) => walk(item, properties, out)?,
            Expr::Binary(_, a, b) => {
                walk(a, properties, out)?;
                walk(b, properties, out)?;
            }
            Expr::Call(name, args) if name == "prop" => {
                let [Expr::Value(Value::String(key))] = args.as_slice() else {
                    return Err(error("prop requires a literal property name or ID"));
                };
                let id = if let Some(property) = properties.iter().find(|p| p.id == *key) {
                    &property.id
                } else {
                    let mut named = properties.iter().filter(|p| p.name == *key);
                    let property = named
                        .next()
                        .ok_or_else(|| error(format!("Unknown property: {key}")))?;
                    if named.next().is_some() {
                        return Err(error(format!("Ambiguous property name: {key}")));
                    }
                    &property.id
                };
                if !out.contains(id) {
                    out.push(id.clone());
                }
            }
            Expr::Call(name, args) => {
                if !matches!(
                    name.as_str(),
                    "if" | "ifs"
                        | "and"
                        | "or"
                        | "empty"
                        | "length"
                        | "substring"
                        | "slice"
                        | "split"
                        | "includes"
                        | "flat"
                        | "map"
                        | "filter"
                        | "find"
                        | "findIndex"
                        | "some"
                        | "every"
                        | "first"
                        | "last"
                        | "at"
                        | "join"
                        | "reverse"
                        | "unique"
                        | "format"
                        | "concat"
                        | "contains"
                        | "test"
                        | "match"
                        | "replace"
                        | "replaceAll"
                        | "lower"
                        | "upper"
                        | "round"
                        | "sqrt"
                        | "cbrt"
                        | "exp"
                        | "ln"
                        | "log10"
                        | "log2"
                        | "sign"
                        | "pi"
                        | "e"
                        | "toNumber"
                        | "median"
                        | "floor"
                        | "ceil"
                        | "abs"
                        | "min"
                        | "max"
                        | "sum"
                        | "mean"
                        | "parseDate"
                        | "dateAdd"
                        | "dateBetween"
                        | "formatDate"
                ) {
                    return Err(error(format!("Unsupported function: {name}")));
                }
                for arg in args {
                    walk(arg, properties, out)?;
                }
            }
        }
        Ok(())
    }
    let expr = Parser::parse(source)?;
    let mut result = Vec::new();
    walk(&expr, properties, &mut result)?;
    Ok(result)
}

/// Capture property-name references by ID so later renames do not change their meaning.
pub fn bindings(
    source: &str,
    properties: &[Property],
) -> Result<BTreeMap<String, String>, EngineError> {
    fn collect(expr: &Expr, keys: &mut Vec<String>) {
        match expr {
            Expr::Value(_) | Expr::Current | Expr::Index => {}
            Expr::List(items) => {
                for item in items {
                    collect(item, keys);
                }
            }
            Expr::Unary(_, item) => collect(item, keys),
            Expr::Binary(_, a, b) => {
                collect(a, keys);
                collect(b, keys);
            }
            Expr::Call(name, args) => {
                if name == "prop" {
                    if let [Expr::Value(Value::String(key))] = args.as_slice() {
                        keys.push(key.clone());
                    }
                }
                for arg in args {
                    collect(arg, keys);
                }
            }
        }
    }
    dependencies(source, properties)?;
    let mut keys = Vec::new();
    collect(&Parser::parse(source)?, &mut keys);
    let mut result = BTreeMap::new();
    for key in keys {
        let prop = properties
            .iter()
            .find(|p| p.id == key)
            .or_else(|| properties.iter().find(|p| p.name == key))
            .ok_or_else(|| error(format!("Unknown property: {key}")))?;
        result.insert(key, prop.id.clone());
    }
    Ok(result)
}

/// Values are keyed by property ID. `prop` accepts a property name or ID.
pub fn evaluate(
    source: &str,
    values: &BTreeMap<String, Value>,
    properties: &[Property],
) -> Result<Value, EngineError> {
    let expr = Parser::parse(source)?;
    eval(&expr, values, properties, &BTreeMap::new(), None)
}

/// Evaluate using stable bindings captured when the formula was saved.
pub fn evaluate_bound(
    source: &str,
    values: &BTreeMap<String, Value>,
    properties: &[Property],
    bindings: &BTreeMap<String, String>,
) -> Result<Value, EngineError> {
    let expr = Parser::parse(source)?;
    eval(&expr, values, properties, bindings, None)
}

fn eval(
    expr: &Expr,
    values: &BTreeMap<String, Value>,
    properties: &[Property],
    bindings: &BTreeMap<String, String>,
    scope: Option<(&Value, usize)>,
) -> Result<Value, EngineError> {
    match expr {
        Expr::Value(v) => Ok(v.clone()),
        Expr::Current => scope
            .map(|(value, _)| value.clone())
            .ok_or_else(|| error("current requires a list operation")),
        Expr::Index => scope
            .map(|(_, index)| json!(index))
            .ok_or_else(|| error("index requires a list operation")),
        Expr::List(items) => items
            .iter()
            .map(|x| eval(x, values, properties, bindings, scope))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Expr::Unary(op, inner) => {
            let v = eval(inner, values, properties, bindings, scope)?;
            match op.as_str() {
                "-" => number(-num(&v)?),
                "!" | "not" => Ok(json!(!boolean(&v)?)),
                _ => Err(error("Unknown unary operator")),
            }
        }
        Expr::Binary(op, a, b) => {
            let a = eval(a, values, properties, bindings, scope)?;
            if (op == "and" || op == "&&") && !boolean(&a)? {
                return Ok(json!(false));
            }
            if (op == "or" || op == "||") && boolean(&a)? {
                return Ok(json!(true));
            }
            let b = eval(b, values, properties, bindings, scope)?;
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
            if matches!(
                name.as_str(),
                "map" | "filter" | "find" | "findIndex" | "some" | "every"
            ) {
                if args.len() != 2 {
                    return Err(error(format!("{name} requires two arguments")));
                }
                let input = eval(&args[0], values, properties, bindings, scope)?;
                let list = input.as_array().ok_or_else(|| error("Expected a list"))?;
                let mut output = Vec::new();
                for (index, item) in list.iter().enumerate() {
                    let result = eval(&args[1], values, properties, bindings, Some((item, index)))?;
                    if name == "map" {
                        output.push(result);
                        continue;
                    }
                    let matches = boolean(&result)?;
                    match name.as_str() {
                        "filter" if matches => output.push(item.clone()),
                        "find" if matches => return Ok(item.clone()),
                        "findIndex" if matches => return Ok(json!(index)),
                        "some" if matches => return Ok(json!(true)),
                        "every" if !matches => return Ok(json!(false)),
                        _ => {}
                    }
                }
                return Ok(match name.as_str() {
                    "map" | "filter" => Value::Array(output),
                    "find" => Value::Null,
                    "findIndex" => json!(-1),
                    "some" => json!(false),
                    "every" => json!(true),
                    _ => unreachable!(),
                });
            }
            if name == "if" || name == "ifs" {
                if (name == "if" && args.len() != 3)
                    || (name == "ifs" && (args.len() < 3 || args.len() % 2 == 0))
                {
                    return Err(error(format!("Invalid arguments for {name}")));
                }
                for pair in args[..args.len() - 1].chunks_exact(2) {
                    if boolean(&eval(&pair[0], values, properties, bindings, scope)?)? {
                        return eval(&pair[1], values, properties, bindings, scope);
                    }
                }
                return eval(args.last().unwrap(), values, properties, bindings, scope);
            }
            if name == "and" || name == "or" {
                if args.is_empty() {
                    return Err(error(format!("{name} requires at least one argument")));
                }
                for arg in args {
                    let result = boolean(&eval(arg, values, properties, bindings, scope)?)?;
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
                let key = eval(&args[0], values, properties, bindings, scope)?;
                let key = key
                    .as_str()
                    .ok_or_else(|| error("prop requires a string"))?;
                let id = if let Some(id) = bindings.get(key) {
                    if !properties.iter().any(|p| &p.id == id) {
                        return Err(error(format!("Property no longer exists: {key}")));
                    }
                    id.as_str()
                } else if let Some(property) = properties.iter().find(|p| p.id == key) {
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
                .map(|a| eval(a, values, properties, bindings, scope))
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
