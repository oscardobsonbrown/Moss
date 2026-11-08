use super::{num, number};
use crate::{EngineError, error};
use chrono::{Duration, Months, NaiveDate};
use serde_json::{Value, json};

fn text(v: &Value) -> Result<&str, EngineError> {
    v.as_str().ok_or_else(|| error("Expected text"))
}
fn date(v: &Value) -> Result<NaiveDate, EngineError> {
    NaiveDate::parse_from_str(text(v)?, "%Y-%m-%d")
        .map_err(|_| error("Expected date in YYYY-MM-DD format"))
}
fn arity(name: &str, args: &[Value], n: usize) -> Result<(), EngineError> {
    if args.len() == n {
        Ok(())
    } else {
        Err(error(format!("{name} requires {n} arguments")))
    }
}

pub fn call(name: &str, args: &[Value]) -> Result<Value, EngineError> {
    match name {
        "empty" => {
            arity(name, args, 1)?;
            Ok(json!(
                args[0].is_null()
                    || args[0].as_f64().is_some_and(|n| n == 0.0)
                    || args[0].as_str().is_some_and(str::is_empty)
                    || args[0].as_array().is_some_and(Vec::is_empty)
            ))
        }
        "length" => {
            arity(name, args, 1)?;
            if let Some(s) = args[0].as_str() {
                Ok(json!(s.chars().count()))
            } else if let Some(a) = args[0].as_array() {
                Ok(json!(a.len()))
            } else {
                Err(error("length requires text or a list"))
            }
        }
        "substring" | "slice" => {
            if !(2..=3).contains(&args.len()) {
                return Err(error(format!("{name} requires two or three arguments")));
            }
            let index = |value: &Value| -> Result<usize, EngineError> {
                let n = num(value)?;
                if n.fract() != 0.0 || n < 0.0 || n > usize::MAX as f64 {
                    return Err(error("Index must be a nonnegative integer"));
                }
                Ok(n as usize)
            };
            let start = index(&args[1])?;
            if name == "substring" {
                let chars: Vec<char> = text(&args[0])?.chars().collect();
                let end = if args.len() == 3 {
                    index(&args[2])?
                } else {
                    chars.len()
                };
                let start = start.min(chars.len());
                let end = end.min(chars.len()).max(start);
                Ok(json!(chars[start..end].iter().collect::<String>()))
            } else {
                let list = args[0].as_array().ok_or_else(|| error("Expected a list"))?;
                let end = if args.len() == 3 {
                    index(&args[2])?
                } else {
                    list.len()
                };
                let start = start.min(list.len());
                let end = end.min(list.len()).max(start);
                Ok(Value::Array(list[start..end].to_vec()))
            }
        }
        "split" => {
            arity(name, args, 2)?;
            let value = text(&args[0])?;
            let separator = text(&args[1])?;
            if separator.is_empty() {
                Ok(json!(
                    value.chars().map(|c| c.to_string()).collect::<Vec<_>>()
                ))
            } else {
                Ok(json!(value.split(separator).collect::<Vec<_>>()))
            }
        }
        "includes" => {
            arity(name, args, 2)?;
            let list = args[0].as_array().ok_or_else(|| error("Expected a list"))?;
            Ok(json!(list.contains(&args[1])))
        }
        "flat" => {
            arity(name, args, 1)?;
            let list = args[0].as_array().ok_or_else(|| error("Expected a list"))?;
            let mut result = Vec::new();
            for item in list {
                if let Some(items) = item.as_array() {
                    result.extend(items.iter().cloned());
                } else {
                    result.push(item.clone());
                }
            }
            Ok(Value::Array(result))
        }
        "first" | "last" => {
            arity(name, args, 1)?;
            let list = args[0].as_array().ok_or_else(|| error("Expected a list"))?;
            Ok(if name == "first" {
                list.first()
            } else {
                list.last()
            }
            .cloned()
            .unwrap_or(Value::Null))
        }
        "at" => {
            arity(name, args, 2)?;
            let list = args[0].as_array().ok_or_else(|| error("Expected a list"))?;
            let index = num(&args[1])?;
            if index.fract() != 0.0 || index < 0.0 || index > usize::MAX as f64 {
                return Err(error("List index must be a nonnegative integer"));
            }
            Ok(list.get(index as usize).cloned().unwrap_or(Value::Null))
        }
        "join" => {
            arity(name, args, 2)?;
            let list = args[0].as_array().ok_or_else(|| error("Expected a list"))?;
            let separator = text(&args[1])?;
            let parts = list.iter().map(text).collect::<Result<Vec<_>, _>>()?;
            Ok(json!(parts.join(separator)))
        }
        "reverse" | "unique" => {
            arity(name, args, 1)?;
            let list = args[0].as_array().ok_or_else(|| error("Expected a list"))?;
            let mut result = Vec::new();
            if name == "reverse" {
                result.extend(list.iter().rev().cloned());
            } else {
                for value in list {
                    if !result.contains(value) {
                        result.push(value.clone());
                    }
                }
            }
            Ok(Value::Array(result))
        }
        "format" => {
            arity(name, args, 1)?;
            Ok(json!(match &args[0] {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                v => v.to_string(),
            }))
        }
        "concat" => {
            if args.iter().all(Value::is_string) {
                Ok(json!(
                    args.iter().map(|v| v.as_str().unwrap()).collect::<String>()
                ))
            } else if args.iter().all(Value::is_array) {
                Ok(Value::Array(
                    args.iter()
                        .flat_map(|v| v.as_array().unwrap().iter().cloned())
                        .collect(),
                ))
            } else {
                Err(error("concat requires text or lists of the same type"))
            }
        }
        "contains" => {
            arity(name, args, 2)?;
            if let Some(s) = args[0].as_str() {
                Ok(json!(s.contains(text(&args[1])?)))
            } else if let Some(a) = args[0].as_array() {
                Ok(json!(a.contains(&args[1])))
            } else {
                Err(error("contains requires text or a list"))
            }
        }
        "test" | "match" => {
            arity(name, args, 2)?;
            let value = text(&args[0])?;
            let regex = regex::RegexBuilder::new(text(&args[1])?)
                .size_limit(1_000_000)
                .build()
                .map_err(|e| error(format!("Invalid or unsupported regular expression: {e}")))?;
            if name == "test" {
                Ok(json!(regex.is_match(value)))
            } else {
                Ok(json!(
                    regex
                        .find_iter(value)
                        .map(|m| m.as_str())
                        .collect::<Vec<_>>()
                ))
            }
        }
        "replace" | "replaceAll" => {
            arity(name, args, 3)?;
            let (value, pattern, replacement) = (text(&args[0])?, text(&args[1])?, text(&args[2])?);
            let regex = regex::RegexBuilder::new(pattern)
                .size_limit(1_000_000)
                .build()
                .map_err(|e| error(format!("Invalid or unsupported regular expression: {e}")))?;
            Ok(json!(
                if name == "replace" {
                    regex.replacen(value, 1, replacement)
                } else {
                    regex.replace_all(value, replacement)
                }
                .into_owned()
            ))
        }
        "lower" | "upper" => {
            arity(name, args, 1)?;
            let s = text(&args[0])?;
            Ok(json!(if name == "lower" {
                s.to_lowercase()
            } else {
                s.to_uppercase()
            }))
        }
        "round" => {
            if !(1..=2).contains(&args.len()) {
                return Err(error("round requires one or two arguments"));
            }
            let n = num(&args[0])?;
            let places = if args.len() == 2 { num(&args[1])? } else { 0.0 };
            if places.fract() != 0.0 || !(-308.0..=308.0).contains(&places) {
                return Err(error(
                    "round precision must be an integer between -308 and 308",
                ));
            }
            let factor = 10f64.powi(places as i32);
            number((n * factor).round() / factor)
        }
        "floor" | "ceil" | "abs" | "sqrt" | "cbrt" | "exp" | "ln" | "log10" | "log2" | "sign" => {
            arity(name, args, 1)?;
            let n = num(&args[0])?;
            number(match name {
                "floor" => n.floor(),
                "ceil" => n.ceil(),
                "abs" => n.abs(),
                "sqrt" => n.sqrt(),
                "cbrt" => n.cbrt(),
                "exp" => n.exp(),
                "ln" => n.ln(),
                "log10" => n.log10(),
                "log2" => n.log2(),
                _ => {
                    if n == 0.0 {
                        0.0
                    } else {
                        n.signum()
                    }
                }
            })
        }
        "pi" | "e" => {
            arity(name, args, 0)?;
            number(if name == "pi" {
                std::f64::consts::PI
            } else {
                std::f64::consts::E
            })
        }
        "toNumber" => {
            arity(name, args, 1)?;
            let n = match &args[0] {
                Value::Bool(value) => {
                    if *value {
                        1.0
                    } else {
                        0.0
                    }
                }
                Value::String(value) => value
                    .trim()
                    .parse()
                    .map_err(|_| error("Cannot convert text to a number"))?,
                value => num(value)?,
            };
            number(n)
        }
        "min" | "max" | "sum" | "mean" | "median" => {
            let mut nums = Vec::new();
            for value in args {
                if let Some(items) = value.as_array() {
                    for item in items {
                        nums.push(num(item)?);
                    }
                } else {
                    nums.push(num(value)?);
                }
            }
            if nums.is_empty() {
                return Err(error(format!("{name} requires at least one number")));
            }
            let total: f64 = nums.iter().sum();
            number(match name {
                "sum" => total,
                "mean" => total / nums.len() as f64,
                "min" => nums.iter().copied().fold(f64::INFINITY, f64::min),
                "max" => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                _ => {
                    nums.sort_by(f64::total_cmp);
                    let mid = nums.len() / 2;
                    if nums.len() % 2 == 0 {
                        (nums[mid - 1] + nums[mid]) / 2.0
                    } else {
                        nums[mid]
                    }
                }
            })
        }
        "parseDate" => {
            arity(name, args, 1)?;
            Ok(json!(date(&args[0])?.to_string()))
        }
        "dateAdd" => {
            arity(name, args, 3)?;
            let d = date(&args[0])?;
            let n = num(&args[1])?;
            if n.fract() != 0.0 || n.abs() > i32::MAX as f64 {
                return Err(error("Date amount must be a whole number"));
            }
            let n = n as i32;
            let result = match text(&args[2])? {
                "days" => d.checked_add_signed(Duration::days(n as i64)),
                "weeks" => d.checked_add_signed(Duration::weeks(n as i64)),
                "months" => {
                    if n >= 0 {
                        d.checked_add_months(Months::new(n as u32))
                    } else {
                        d.checked_sub_months(Months::new(n.unsigned_abs()))
                    }
                }
                "years" => {
                    let months = n.checked_mul(12).ok_or_else(|| error("Date overflow"))?;
                    if months >= 0 {
                        d.checked_add_months(Months::new(months as u32))
                    } else {
                        d.checked_sub_months(Months::new(months.unsigned_abs()))
                    }
                }
                _ => return Err(error("Unsupported date unit")),
            }
            .ok_or_else(|| error("Date overflow"))?;
            Ok(json!(result.to_string()))
        }
        "dateBetween" => {
            arity(name, args, 3)?;
            let days = date(&args[0])?
                .signed_duration_since(date(&args[1])?)
                .num_days();
            match text(&args[2])? {
                "days" => Ok(json!(days)),
                "weeks" => Ok(json!(days / 7)),
                _ => Err(error("Unsupported dateBetween unit")),
            }
        }
        "formatDate" => {
            arity(name, args, 2)?;
            let format = text(&args[1])?;
            let format = match format {
                "YYYY-MM-DD" => "%Y-%m-%d",
                "YYYY" => "%Y",
                "MM/DD/YYYY" => "%m/%d/%Y",
                "DD/MM/YYYY" => "%d/%m/%Y",
                _ => return Err(error("Unsupported date format")),
            };
            Ok(json!(date(&args[0])?.format(format).to_string()))
        }
        _ => Err(error(format!("Unsupported function: {name}"))),
    }
}
