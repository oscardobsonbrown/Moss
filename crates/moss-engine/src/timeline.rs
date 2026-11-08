use crate::{EngineError, error};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
}
/// `before` blocks `after`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub before: String,
    pub after: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftMode {
    Overlap,
    MaintainGap,
    None,
}
fn add(date: NaiveDate, days: i64) -> Result<NaiveDate, EngineError> {
    date.checked_add_signed(Duration::days(days))
        .ok_or_else(|| error("Date overflow"))
}
fn weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}
/// Returns all tasks, preserving their order. The changed task retains its duration.
pub fn shift_dependencies(
    tasks: &[Task],
    edges: &[Dependency],
    changed_id: &str,
    new_start: NaiveDate,
    mode: ShiftMode,
    avoid_weekends: bool,
) -> Result<Vec<Task>, EngineError> {
    let mut index = BTreeMap::new();
    for (i, task) in tasks.iter().enumerate() {
        if task.start > task.end {
            return Err(error("Task start is after end"));
        }
        if index.insert(task.id.clone(), i).is_some() {
            return Err(error("Duplicate task ID"));
        }
    }
    let changed = *index
        .get(changed_id)
        .ok_or_else(|| error("Unknown changed task"))?;
    let mut incoming = vec![Vec::new(); tasks.len()];
    let mut outgoing = vec![Vec::new(); tasks.len()];
    let mut seen = BTreeSet::new();
    for edge in edges {
        let a = *index
            .get(&edge.before)
            .ok_or_else(|| error("Unknown dependency task"))?;
        let b = *index
            .get(&edge.after)
            .ok_or_else(|| error("Unknown dependency task"))?;
        if !seen.insert((a, b)) {
            return Err(error("Duplicate dependency"));
        }
        incoming[b].push(a);
        outgoing[a].push(b);
    }
    let mut degree: Vec<_> = incoming.iter().map(Vec::len).collect();
    let mut queue: VecDeque<_> = degree
        .iter()
        .enumerate()
        .filter_map(|(i, &d)| (d == 0).then_some(i))
        .collect();
    let mut order = Vec::new();
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &j in &outgoing[i] {
            degree[j] -= 1;
            if degree[j] == 0 {
                queue.push_back(j);
            }
        }
    }
    if order.len() != tasks.len() {
        return Err(error("Dependency cycle"));
    }
    let mut result = tasks.to_vec();
    let duration = tasks[changed]
        .end
        .signed_duration_since(tasks[changed].start)
        .num_days();
    result[changed].start = new_start;
    result[changed].end = add(new_start, duration)?;
    if mode == ShiftMode::None {
        return Ok(result);
    }
    let mut moved = vec![false; tasks.len()];
    moved[changed] = new_start != tasks[changed].start;
    for i in order {
        if i == changed || !incoming[i].iter().any(|&p| moved[p]) {
            continue;
        }
        let duration = tasks[i]
            .end
            .signed_duration_since(tasks[i].start)
            .num_days();
        let mut candidate: Option<NaiveDate> = None;
        for &p in &incoming[i] {
            let gap = match mode {
                ShiftMode::Overlap => 1,
                ShiftMode::MaintainGap => tasks[i]
                    .start
                    .signed_duration_since(tasks[p].end)
                    .num_days()
                    .max(1),
                ShiftMode::None => unreachable!(),
            };
            let target = add(result[p].end, gap)?;
            candidate = Some(candidate.map_or(target, |d| d.max(target)));
        }
        let mut start = candidate.unwrap();
        if mode == ShiftMode::Overlap {
            start = start.max(tasks[i].start);
        }
        if avoid_weekends && start != tasks[i].start {
            for _ in 0..7 {
                if !weekend(start) && !weekend(add(start, duration)?) {
                    break;
                }
                start = add(start, 1)?;
            }
            if weekend(start) || weekend(add(start, duration)?) {
                return Err(error("Cannot avoid weekends for task duration"));
            }
        }
        result[i].start = start;
        result[i].end = add(start, duration)?;
        moved[i] = start != tasks[i].start;
    }
    Ok(result)
}
