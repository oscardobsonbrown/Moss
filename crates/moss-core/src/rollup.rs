use crate::{EngineError, error};
use serde_json::{Value, json};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollupOp {
    Original,
    Unique,
    Count,
    CountValues,
    CountEmpty,
    PercentEmpty,
    Sum,
    Average,
    Min,
    Max,
}
pub fn aggregate(op: RollupOp, values: &[Value]) -> Result<Value, EngineError> {
    let empty = |v: &&Value| {
        v.is_null()
            || v.as_str().is_some_and(str::is_empty)
            || v.as_array().is_some_and(Vec::is_empty)
    };
    match op {
        RollupOp::Original => Ok(json!(values)),
        RollupOp::Unique => {
            let mut result = Vec::new();
            for v in values {
                if !result.contains(v) {
                    result.push(v.clone());
                }
            }
            Ok(json!(result))
        }
        RollupOp::Count => Ok(json!(values.len())),
        RollupOp::CountValues => Ok(json!(values.iter().filter(|v| !empty(v)).count())),
        RollupOp::CountEmpty => Ok(json!(values.iter().filter(empty).count())),
        RollupOp::PercentEmpty => {
            if values.is_empty() {
                Ok(Value::Null)
            } else {
                Ok(json!(
                    values.iter().filter(empty).count() as f64 / values.len() as f64
                ))
            }
        }
        RollupOp::Sum | RollupOp::Average | RollupOp::Min | RollupOp::Max => {
            let nums = values
                .iter()
                .filter(|v| !v.is_null())
                .map(|v| v.as_f64().ok_or_else(|| error("Rollup requires numbers")))
                .collect::<Result<Vec<_>, _>>()?;
            if nums.is_empty() {
                return Ok(Value::Null);
            }
            let sum: f64 = nums.iter().sum();
            let result = match op {
                RollupOp::Sum => sum,
                RollupOp::Average => sum / nums.len() as f64,
                RollupOp::Min => nums.iter().copied().fold(f64::INFINITY, f64::min),
                _ => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            };
            serde_json::Number::from_f64(result)
                .map(Value::Number)
                .ok_or_else(|| error("Non-finite rollup"))
        }
    }
}
