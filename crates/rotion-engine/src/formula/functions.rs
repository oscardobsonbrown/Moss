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
fn values(args: &[Value]) -> &[Value] {
    if args.len() == 1 {
        args[0].as_array().map(Vec::as_slice).unwrap_or(args)
    } else {
        args
    }
}

pub fn call(name: &str, args: &[Value]) -> Result<Value, EngineError> {
    match name {
        "empty" => {
            arity(name, args, 1)?;
            Ok(json!(
                args[0].is_null()
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
        "replace" | "replaceAll" => {
            arity(name, args, 3)?;
            let (s, from, to) = (text(&args[0])?, text(&args[1])?, text(&args[2])?);
            Ok(json!(if name == "replace" {
                s.replacen(from, to, 1)
            } else {
                s.replace(from, to)
            }))
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
        "round" | "floor" | "ceil" | "abs" => {
            arity(name, args, 1)?;
            let n = num(&args[0])?;
            number(match name {
                "round" => n.round(),
                "floor" => n.floor(),
                "ceil" => n.ceil(),
                _ => n.abs(),
            })
        }
        "min" | "max" | "sum" | "mean" => {
            let nums = values(args)
                .iter()
                .map(num)
                .collect::<Result<Vec<_>, _>>()?;
            if nums.is_empty() {
                return Err(error(format!("{name} requires at least one number")));
            }
            let total: f64 = nums.iter().sum();
            number(match name {
                "sum" => total,
                "mean" => total / nums.len() as f64,
                "min" => nums.iter().copied().fold(f64::INFINITY, f64::min),
                _ => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
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
