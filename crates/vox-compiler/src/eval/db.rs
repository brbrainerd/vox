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

use crate::eval::value::VoxValue;
use crate::eval::EvalError;
use crate::eval::Interpreter;
use crate::hir::nodes::{HirDbQueryPlan, HirDbTableOp};

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
        "gte" => matches!(vox_cmp(have, want), Some(Ordering::Greater | Ordering::Equal)),
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

/// Evaluate a `where`/`filter` surface object against a row. The object follows
/// the lowering's vocabulary (`parse_where_object_predicate`):
/// `{ and: [..] }`, `{ or: [..] }`, `{ not: {..} }`, `{ field: { op: val } }`,
/// `{ field: { is_null: _ } }`, `{ field: { in: [..] } }`, and the scalar
/// shorthand `{ field: val }` (equality). Reading values directly from the
/// surface object keeps the comparison values that the plan's predicate
/// structure does not carry.
fn matches_object(row: &Row, obj: &[(String, VoxValue)]) -> bool {
    obj.iter().all(|(key, spec)| match key.as_str() {
        "and" => match spec {
            VoxValue::List(items) => items.iter().all(|it| match it {
                VoxValue::Object(inner) => matches_object(row, inner),
                _ => false,
            }),
            _ => false,
        },
        "or" => match spec {
            VoxValue::List(items) => items.iter().any(|it| match it {
                VoxValue::Object(inner) => matches_object(row, inner),
                _ => false,
            }),
            _ => false,
        },
        "not" => match spec {
            VoxValue::Object(inner) => !matches_object(row, inner),
            _ => false,
        },
        field_name => {
            let have = match field(row, field_name) {
                Some(v) => v,
                None => return false,
            };
            match spec {
                // `{ field: { op: val } }` — single-key operator object.
                VoxValue::Object(op_fields) if op_fields.len() == 1 => {
                    let (op, op_val) = &op_fields[0];
                    match op.as_str() {
                        "is_null" => matches!(have, VoxValue::Null),
                        "in" => match op_val {
                            VoxValue::List(items) => items.iter().any(|it| it == have),
                            _ => false,
                        },
                        other => apply_op(other, have, op_val),
                    }
                }
                // `{ field: val }` — equality shorthand.
                scalar => have == scalar,
            }
        }
    })
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
        HirDbTableOp::All
        | HirDbTableOp::FilterRecord
        | HirDbTableOp::UnsafeQueryRawClause => {
            // Snapshot rows, filter by the surface `where`/`filter` object, then
            // apply projection / order_by / limit. The filter object is the
            // first object-valued arg on this call.
            // `UnsafeQueryRawClause` has no interpreter SQL analogue.
            let rows: Vec<Row> = interp.db.table_mut(&plan.table).rows.clone();
            let filter_obj = args.iter().find_map(|(_, v)| match v {
                VoxValue::Object(fields) => Some(fields.clone()),
                _ => None,
            });
            // A plan that carries a predicate but no surface filter object is a
            // *fused* query chain (`.where({..}).select(..)`) whose comparison
            // values live on an inner chain node we never executed. Rather than
            // silently return unfiltered rows (wrong data), fail loudly. Single
            // `.where(..)`/`.filter(..)` calls carry their object here and work.
            // Lifting this is the documented follow-on (carry predicate values
            // in the plan) — see the interpreter-db-execution spec.
            if plan.predicate.is_some() && filter_obj.is_none() {
                return Ok(VoxValue::Result(Err(crate::eval::value::err_str(
                    "interp db: fused query-chain predicates (e.g. \
                     `.where({..}).select(..)`) are not yet executable under \
                     --mode interp; run a single `.where`/`.filter` call, or use \
                     --mode script. See interpreter-db-execution spec."
                        .to_string(),
                ))));
            }
            let mut out: Vec<Row> = match &filter_obj {
                Some(obj) => rows
                    .into_iter()
                    .filter(|r| matches_object(r, obj))
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

            if plan.has_limit {
                if let Some((_, VoxValue::Int(n))) = args.last() {
                    let n = (*n).max(0) as usize;
                    out.truncate(n);
                }
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
                            if c != "_id" {
                                if let Some(pair) = r.iter().find(|(k, _)| k == c) {
                                    keep.push(pair.clone());
                                }
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
