//! In-memory DB execution for the tree-walking interpreter (`--mode interp`).
//!
//! `db.*` operations lower to `HirExpr::MethodCall` carrying an
//! [`HirDbQueryPlan`]. Under `--mode script` these codegen to real SQL; under
//! the interpreter we execute the same plan against a per-`Interpreter`
//! in-memory store so data-layer programs produce real input→output in the
//! default run mode.
//!
//! Return types mirror `typeck/builtins.rs` exactly (see
//! `docs/superpowers/specs/2026-06-03-interpreter-db-execution-design.md`):
//! `insert -> Result[Int]`, `get`/`find -> Result[Option[Record]]`,
//! `delete -> Result[Unit]`, `all`/`filter`/`where`/… -> `Result[List[Record]]`,
//! `count -> Result[Int]`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::eval::EvalError;
use crate::eval::Interpreter;
use crate::eval::expr::eval_expr;
use crate::eval::value::VoxValue;
use crate::hir::nodes::{HirDbPredicate, HirDbQueryPlan, HirDbTableOp};

/// One row is an ordered list of `(field, value)` pairs, matching
/// `VoxValue::Object`. Every row carries an auto-assigned `_id` field.
type Row = Vec<(String, VoxValue)>;

/// Per-table storage: rows plus the monotonic id counter.
#[derive(Debug, Clone, Default)]
pub struct DbTable {
    pub rows: Vec<Row>,
    pub next_id: i64,
}

/// In-memory database for one interpreter run. Tables are created lazily on
/// first insert/query.
#[derive(Debug, Clone, Default)]
pub struct DbStore {
    pub tables: BTreeMap<String, DbTable>,
}

impl DbStore {
    fn table_mut(&mut self, name: &str) -> &mut DbTable {
        self.tables.entry(name.to_string()).or_default()
    }
}

fn ok(v: VoxValue) -> VoxValue {
    VoxValue::Result(Ok(Box::new(v)))
}

fn row_to_object(row: &Row) -> VoxValue {
    VoxValue::Object(row.clone())
}

/// Three-way compare for the orderable `VoxValue` scalars (Int/Float/Decimal/
/// Str/Bool). Cross-numeric Int/Float compare as `f64`, consistent with
/// interpreter arithmetic promotion. Returns `None` for incomparable pairs.
fn vox_cmp(a: &VoxValue, b: &VoxValue) -> Option<Ordering> {
    use VoxValue::*;
    match (a, b) {
        (Int(x), Int(y)) => Some(x.cmp(y)),
        (Float(x), Float(y)) => x.partial_cmp(y),
        (Int(x), Float(y)) => (*x as f64).partial_cmp(y),
        (Float(x), Int(y)) => x.partial_cmp(&(*y as f64)),
        (Decimal(x), Decimal(y)) => Some(x.cmp(y)),
        (Str(x), Str(y)) => Some(x.cmp(y)),
        (Bool(x), Bool(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn field<'r>(row: &'r Row, name: &str) -> Option<&'r VoxValue> {
    row.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

/// Apply a single comparison operator (`gte`, `lt`, …) between a row field
/// value and the filter value.
fn apply_op(op: &str, have: &VoxValue, want: &VoxValue) -> bool {
    match op {
        "eq" => have == want,
        "neq" => have != want,
        "lt" => vox_cmp(have, want) == Some(Ordering::Less),
        "lte" => matches!(vox_cmp(have, want), Some(Ordering::Less | Ordering::Equal)),
        "gt" => vox_cmp(have, want) == Some(Ordering::Greater),
        "gte" => matches!(
            vox_cmp(have, want),
            Some(Ordering::Greater | Ordering::Equal)
        ),
        "contains" => match (have, want) {
            (VoxValue::Str(h), VoxValue::Str(w)) => h.contains(w.as_str()),
            (VoxValue::List(items), w) => items.iter().any(|it| it == w),
            _ => false,
        },
        // Unknown operator keyword → treat as equality (mirrors the lowering's
        // fallback in `parse_where_object_predicate`).
        _ => have == want,
    }
}

/// Take the next flattened predicate value, advancing the cursor.
fn take(vals: &[VoxValue], pos: &mut usize) -> Option<VoxValue> {
    let v = vals.get(*pos).cloned();
    if v.is_some() {
        *pos += 1;
    }
    v
}

/// Evaluate a predicate against a row, threading the flattened comparison values
/// (`vals`) positionally via `pos`. The order matches how the lowering's
/// `parse_where_object_predicate` flattened them (predicate DFS order), so
/// carrying `vals` on the plan lets the interpreter filter even *fused* chains
/// where the surface object is not on the executed node.
///
/// `And`/`Or` evaluate **every** part (no short-circuit) so the cursor stays
/// aligned regardless of which branches match. `is_null` consumes no value;
/// each comparison leaf consumes one; `in` consumes `arity`.
fn eval_predicate(pred: &HirDbPredicate, row: &Row, vals: &[VoxValue], pos: &mut usize) -> bool {
    match pred {
        HirDbPredicate::Eq { field: f }
        | HirDbPredicate::Neq { field: f }
        | HirDbPredicate::Lt { field: f }
        | HirDbPredicate::Lte { field: f }
        | HirDbPredicate::Gt { field: f }
        | HirDbPredicate::Gte { field: f }
        | HirDbPredicate::Contains { field: f } => {
            let want = take(vals, pos);
            match (want, field(row, f)) {
                (Some(w), Some(have)) => apply_op(predicate_op(pred), have, &w),
                _ => false,
            }
        }
        HirDbPredicate::In { field: f, arity } => {
            let have = field(row, f).cloned();
            let mut matched = false;
            for _ in 0..*arity {
                if let Some(w) = take(vals, pos)
                    && have.as_ref() == Some(&w)
                {
                    matched = true;
                }
            }
            matched
        }
        HirDbPredicate::IsNull { field: f } => {
            matches!(field(row, f), None | Some(VoxValue::Null))
        }
        HirDbPredicate::And(parts) => {
            let mut all = true;
            for p in parts {
                if !eval_predicate(p, row, vals, pos) {
                    all = false;
                }
            }
            all
        }
        HirDbPredicate::Or(parts) => {
            let mut any = false;
            for p in parts {
                if eval_predicate(p, row, vals, pos) {
                    any = true;
                }
            }
            any
        }
        HirDbPredicate::Not(inner) => !eval_predicate(inner, row, vals, pos),
    }
}

/// The operator keyword for a comparison-leaf predicate (for [`apply_op`]).
fn predicate_op(pred: &HirDbPredicate) -> &'static str {
    match pred {
        HirDbPredicate::Eq { .. } => "eq",
        HirDbPredicate::Neq { .. } => "neq",
        HirDbPredicate::Lt { .. } => "lt",
        HirDbPredicate::Lte { .. } => "lte",
        HirDbPredicate::Gt { .. } => "gt",
        HirDbPredicate::Gte { .. } => "gte",
        HirDbPredicate::Contains { .. } => "contains",
        _ => "eq",
    }
}

/// Execute a lowered DB plan against the interpreter's in-memory store. `args`
/// are the call-site argument values with their optional labels.
pub fn execute_db_plan(
    interp: &mut Interpreter,
    plan: &HirDbQueryPlan,
    args: Vec<(Option<String>, VoxValue)>,
) -> Result<VoxValue, EvalError> {
    match plan.op {
        HirDbTableOp::Insert => {
            let record = args
                .iter()
                .find_map(|(_, v)| match v {
                    VoxValue::Object(fields) => Some(fields.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let table = interp.db.table_mut(&plan.table);
            let id = table.next_id;
            table.next_id += 1;
            let mut row: Row = vec![("_id".to_string(), VoxValue::Int(id))];
            for (k, v) in record {
                if k != "_id" {
                    row.push((k, v));
                }
            }
            table.rows.push(row);
            Ok(ok(VoxValue::Int(id)))
        }
        HirDbTableOp::Get => {
            let id = args.first().map(|(_, v)| v.clone());
            let found = interp
                .db
                .table_mut(&plan.table)
                .rows
                .iter()
                .find(|r| field(r, "_id") == id.as_ref())
                .map(row_to_object);
            Ok(ok(VoxValue::Option(found.map(Box::new))))
        }
        HirDbTableOp::Delete => {
            let id = args.first().map(|(_, v)| v.clone());
            let table = interp.db.table_mut(&plan.table);
            table.rows.retain(|r| field(r, "_id") != id.as_ref());
            Ok(ok(VoxValue::Null))
        }
        HirDbTableOp::Count => {
            let n = interp.db.table_mut(&plan.table).rows.len() as i64;
            Ok(ok(VoxValue::Int(n)))
        }
        HirDbTableOp::All | HirDbTableOp::FilterRecord | HirDbTableOp::UnsafeQueryRawClause => {
            // Evaluate the plan-carried predicate values and limit first (both
            // borrow `interp` mutably via `eval_expr`), then snapshot rows and
            // filter / order / limit / project. The predicate values come from
            // the plan — not the surface args — so a *fused* chain
            // (`.where({..}).select(..)`) filters correctly even though its
            // `where` object is not on the executed node.
            // `UnsafeQueryRawClause` has no interpreter SQL analogue.
            let mut pred_vals: Vec<VoxValue> = Vec::with_capacity(plan.predicate_args.len());
            for a in &plan.predicate_args {
                pred_vals.push(eval_expr(interp, &a.value)?);
            }
            // limit: prefer the plan-carried value (survives fusion); else fall
            // back to a trailing Int surface arg when `has_limit` is set.
            let limit_n: Option<usize> = match &plan.limit_value {
                Some(e) => match eval_expr(interp, e)? {
                    VoxValue::Int(n) => Some(n.max(0) as usize),
                    _ => None,
                },
                None if plan.has_limit => args.iter().rev().find_map(|(_, v)| match v {
                    VoxValue::Int(n) => Some((*n).max(0) as usize),
                    _ => None,
                }),
                None => None,
            };

            let rows: Vec<Row> = interp.db.table_mut(&plan.table).rows.clone();
            let mut out: Vec<Row> = match &plan.predicate {
                Some(pred) => rows
                    .into_iter()
                    .filter(|r| {
                        let mut pos = 0;
                        eval_predicate(pred, r, &pred_vals, &mut pos)
                    })
                    .collect(),
                None => rows,
            };

            if let Some((by, ascending)) = &plan.order_by {
                out.sort_by(|a, b| {
                    let c = match (field(a, by), field(b, by)) {
                        (Some(x), Some(y)) => vox_cmp(x, y).unwrap_or(Ordering::Equal),
                        _ => Ordering::Equal,
                    };
                    if *ascending { c } else { c.reverse() }
                });
            }

            if let Some(n) = limit_n {
                out.truncate(n);
            }

            let projected: Vec<VoxValue> = match &plan.projection {
                Some(cols) => out
                    .iter()
                    .map(|r| {
                        let mut keep: Row = Vec::new();
                        if let Some(id) = r.iter().find(|(k, _)| k == "_id") {
                            keep.push(id.clone());
                        }
                        for c in cols {
                            if c != "_id"
                                && let Some(pair) = r.iter().find(|(k, _)| k == c)
                            {
                                keep.push(pair.clone());
                            }
                        }
                        VoxValue::Object(keep)
                    })
                    .collect(),
                None => out.iter().map(row_to_object).collect(),
            };
            Ok(ok(VoxValue::List(projected)))
        }
    }
}
