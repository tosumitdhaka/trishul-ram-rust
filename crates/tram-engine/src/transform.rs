// SPDX-License-Identifier: Apache-2.0
//! In-memory-only compiled expression and stateless transform interpreter.
//! No filesystem handles or dynamically loaded plugins are accepted.
use std::{borrow::Cow, collections::BTreeMap};
use tram_config::{BinaryOp, Expression, Transform, UnaryOp};
use tram_model::{Datum, RecordEnvelope};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformError {
    MissingField(String),
    UnsupportedType,
    Overflow,
    InvalidCondition,
    Collision(String),
}
pub fn truthy(v: &Datum) -> Result<bool, TransformError> {
    match v {
        Datum::Null => Ok(false),
        Datum::Boolean(v) => Ok(*v),
        Datum::Signed(v) => Ok(*v != 0),
        Datum::Unsigned(v) => Ok(*v != 0),
        Datum::BigInteger(v) => Ok(v.as_str() != "0"),
        Datum::Float(_) | Datum::Decimal(_) | Datum::Timestamp(_) | Datum::Bytes(_) => {
            Err(TransformError::UnsupportedType)
        }
        Datum::String(v) => Ok(!v.is_empty()),
        Datum::Array(v) => Ok(!v.is_empty()),
        Datum::Object(v) => Ok(!v.is_empty()),
    }
}
fn numeric(v: &Datum) -> Result<i128, TransformError> {
    match v {
        Datum::Signed(v) => Ok(i128::from(*v)),
        Datum::Unsigned(v) => Ok(i128::from(*v)),
        Datum::BigInteger(v) => v
            .as_str()
            .parse()
            .map_err(|_| TransformError::UnsupportedType),
        _ => Err(TransformError::UnsupportedType),
    }
}
fn number_out(value: i128) -> Result<Datum, TransformError> {
    if let Ok(v) = i64::try_from(value) {
        return Ok(Datum::Signed(v));
    }
    if let Ok(v) = u64::try_from(value) {
        return Ok(Datum::Unsigned(v));
    }
    Datum::bigint(value.to_string()).map_err(|_| TransformError::Overflow)
}
fn compare(a: &Datum, b: &Datum) -> Result<std::cmp::Ordering, TransformError> {
    match (a, b) {
        (Datum::String(x), Datum::String(y)) => Ok(x.cmp(y)),
        (Datum::Boolean(x), Datum::Boolean(y)) => Ok(x.cmp(y)),
        _ => Ok(numeric(a)?.cmp(&numeric(b)?)),
    }
}
// Borrowed field and literal operands never allocate temporary Datum trees.
// Only arithmetic/comparison results own their (bounded scalar) value.
// AddField materializes a clone under its caller's pre-acquired growth budget.
fn evaluate_view<'a>(
    expr: &'a Expression,
    row: &'a BTreeMap<String, Datum>,
) -> Result<Cow<'a, Datum>, TransformError> {
    match expr {
        Expression::Literal(v) => Ok(Cow::Borrowed(v)),
        Expression::Field(key) => row
            .get(key)
            .map(Cow::Borrowed)
            .ok_or_else(|| TransformError::MissingField(key.clone())),
        Expression::Unary { op, expr } => {
            let inner = evaluate_view(expr, row)?;
            let value = match op {
                UnaryOp::Not => Datum::Boolean(!truthy(inner.as_ref())?),
                UnaryOp::Negative => number_out(
                    numeric(inner.as_ref())?
                        .checked_neg()
                        .ok_or(TransformError::Overflow)?,
                )?,
            };
            Ok(Cow::Owned(value))
        }
        Expression::Binary { op, left, right } => {
            if *op == BinaryOp::And {
                let a = evaluate_view(left, row)?;
                if !truthy(a.as_ref())? {
                    return Ok(Cow::Owned(Datum::Boolean(false)));
                }
                let b = evaluate_view(right, row)?;
                return Ok(Cow::Owned(Datum::Boolean(truthy(b.as_ref())?)));
            }
            if *op == BinaryOp::Or {
                let a = evaluate_view(left, row)?;
                if truthy(a.as_ref())? {
                    return Ok(Cow::Owned(Datum::Boolean(true)));
                }
                let b = evaluate_view(right, row)?;
                return Ok(Cow::Owned(Datum::Boolean(truthy(b.as_ref())?)));
            }
            let a = evaluate_view(left, row)?;
            let b = evaluate_view(right, row)?;
            let output = match op {
                BinaryOp::Add => number_out(
                    numeric(a.as_ref())?
                        .checked_add(numeric(b.as_ref())?)
                        .ok_or(TransformError::Overflow)?,
                )?,
                BinaryOp::Sub => number_out(
                    numeric(a.as_ref())?
                        .checked_sub(numeric(b.as_ref())?)
                        .ok_or(TransformError::Overflow)?,
                )?,
                BinaryOp::Mul => number_out(
                    numeric(a.as_ref())?
                        .checked_mul(numeric(b.as_ref())?)
                        .ok_or(TransformError::Overflow)?,
                )?,
                BinaryOp::Eq => Datum::Boolean(equality(a.as_ref(), b.as_ref())?),
                BinaryOp::Ne => Datum::Boolean(!equality(a.as_ref(), b.as_ref())?),
                BinaryOp::Lt => Datum::Boolean(compare(a.as_ref(), b.as_ref())?.is_lt()),
                BinaryOp::Le => Datum::Boolean(compare(a.as_ref(), b.as_ref())?.is_le()),
                BinaryOp::Gt => Datum::Boolean(compare(a.as_ref(), b.as_ref())?.is_gt()),
                BinaryOp::Ge => Datum::Boolean(compare(a.as_ref(), b.as_ref())?.is_ge()),
                BinaryOp::And | BinaryOp::Or => unreachable!("short-circuited above"),
            };
            Ok(Cow::Owned(output))
        }
    }
}
fn condition_is_true(
    expr: &Expression,
    row: &BTreeMap<String, Datum>,
) -> Result<bool, TransformError> {
    let value = evaluate_view(expr, row)?;
    truthy(value.as_ref())
}
pub fn evaluate(expr: &Expression, row: &BTreeMap<String, Datum>) -> Result<Datum, TransformError> {
    Ok(evaluate_view(expr, row)?.into_owned())
}
fn equality(a: &Datum, b: &Datum) -> Result<bool, TransformError> {
    match (a, b) {
        (Datum::Boolean(_), Datum::Signed(_) | Datum::Unsigned(_) | Datum::BigInteger(_))
        | (Datum::Signed(_) | Datum::Unsigned(_) | Datum::BigInteger(_), Datum::Boolean(_)) => {
            Ok(false)
        } // Do not coerce bool to integer at the P1 boundary.
        (
            Datum::Signed(_) | Datum::Unsigned(_) | Datum::BigInteger(_),
            Datum::Signed(_) | Datum::Unsigned(_) | Datum::BigInteger(_),
        ) => Ok(numeric(a)? == numeric(b)?),
        (Datum::Null, Datum::Null) => Ok(true),
        (Datum::Null, _) | (_, Datum::Null) => Ok(false),
        (Datum::Boolean(x), Datum::Boolean(y)) => Ok(x == y),
        (Datum::String(x), Datum::String(y)) => Ok(x == y),
        (Datum::String(_), _) | (_, Datum::String(_)) => Ok(false),
        _ => Err(TransformError::UnsupportedType),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordDisposition {
    Retained,
    FilteredGlobal,
    FilteredForSink { slot: usize },
}
pub fn apply(
    record: &mut RecordEnvelope,
    transforms: &[Transform],
) -> Result<RecordDisposition, TransformError> {
    for transform in transforms {
        match transform {
            Transform::Rename(mapping) => {
                for (from, to) in mapping {
                    if record.data.contains_key(from) && record.data.contains_key(to) {
                        return Err(TransformError::Collision(to.clone()));
                    }
                }
                for (from, to) in mapping {
                    if let Some(value) = record.data.remove(from) {
                        record.data.insert(to.clone(), value);
                    }
                }
            }
            Transform::AddField(expressions) => {
                for (field, expr) in expressions {
                    // Values added earlier in the same map are visible.
                    let value = evaluate_view(expr, &record.data)?.into_owned();
                    record.data.insert(field.clone(), value);
                }
            }
            Transform::Filter(condition) => {
                if !condition_is_true(condition, &record.data)? {
                    return Ok(RecordDisposition::FilteredGlobal);
                }
            }
            Transform::Drop(fields) => {
                for field in fields {
                    record.data.remove(field);
                }
            }
        }
    }
    Ok(RecordDisposition::Retained)
}
pub fn branch_condition(
    record: &RecordEnvelope,
    condition: Option<&Expression>,
    slot: usize,
) -> Result<RecordDisposition, TransformError> {
    if let Some(expr) = condition {
        if !condition_is_true(expr, &record.data)? {
            return Ok(RecordDisposition::FilteredForSink { slot });
        }
    }
    Ok(RecordDisposition::Retained)
}
#[cfg(test)]
mod tests {
    use super::*;
    use tram_model::{BranchId, Provenance, RecordId, RunId, SourceUnitId};
    fn sample() -> RecordEnvelope {
        let mut rec = RecordEnvelope::new(Provenance {
            run_id: RunId::new("test-run").unwrap(),
            record_id: RecordId::new("record-1").unwrap(),
            source_unit_id: SourceUnitId::new("unit-1").unwrap(),
            source_plugin: "local".into(),
            ingested_at: None,
        });
        rec.data.insert("old_id".into(), Datum::String("A".into()));
        rec.data.insert("metric".into(), Datum::Signed(12));
        rec
    }
    fn field(key: &str) -> Expression {
        Expression::Field(key.into())
    }
    fn literal(n: i64) -> Expression {
        Expression::Literal(Datum::Signed(n))
    }
    fn binary(op: BinaryOp, a: Expression, b: Expression) -> Expression {
        Expression::Binary {
            op,
            left: Box::new(a),
            right: Box::new(b),
        }
    }
    #[test]
    fn r2_m5_large_field_comparison_and_short_circuit_borrow_owned_values() {
        let large = "q".repeat(3 * 1024 * 1024);
        let row = BTreeMap::from([("payload".into(), Datum::String(large.clone()))]);
        let field = Expression::Field("payload".into());
        let literal = Expression::Literal(Datum::String(large));
        assert!(matches!(
            evaluate_view(&field, &row).unwrap(),
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            evaluate_view(&literal, &row).unwrap(),
            Cow::Borrowed(_)
        ));
        let equals = binary(BinaryOp::Eq, field.clone(), literal);
        assert_eq!(
            evaluate_view(&equals, &row).unwrap().as_ref(),
            &Datum::Boolean(true)
        );
        let false_and_missing = binary(
            BinaryOp::And,
            Expression::Literal(Datum::Boolean(false)),
            Expression::Field("missing".into()),
        );
        assert_eq!(
            evaluate_view(&false_and_missing, &row).unwrap().as_ref(),
            &Datum::Boolean(false)
        );
    }
    #[test]
    fn comp_07_11_four_transforms_and_independent_fanout() {
        let mut a = sample();
        let steps = vec![
            Transform::Rename(vec![("old_id".into(), "cell_id".into())]),
            Transform::AddField(vec![(
                "double_metric".into(),
                binary(BinaryOp::Mul, field("metric"), literal(2)),
            )]),
            Transform::Filter(binary(BinaryOp::Ge, field("metric"), literal(10))),
            Transform::Drop(vec!["metric".into()]),
        ];
        assert_eq!(apply(&mut a, &steps), Ok(RecordDisposition::Retained));
        assert_eq!(a.data.get("double_metric"), Some(&Datum::Signed(24)));
        assert_eq!(a.data.get("metric"), None);
        let original = a.clone();
        let b = a.fork_for_branch(BranchId::new("B").unwrap());
        let mut b = b;
        assert_eq!(
            apply(
                &mut b,
                &[Transform::AddField(vec![(
                    "tag".into(),
                    Expression::Literal(Datum::String("secondary".into()))
                )])]
            ),
            Ok(RecordDisposition::Retained)
        );
        assert!(!a.data.contains_key("tag"));
        assert_eq!(b.data.get("tag"), Some(&Datum::String("secondary".into())));
        assert_eq!(original.data, a.data);
    }
    #[test]
    fn comp_08_sequential_expression_and_overflow() {
        let mut record = sample();
        let expr = vec![Transform::AddField(vec![
            (
                "first".into(),
                binary(BinaryOp::Add, field("metric"), literal(1)),
            ),
            (
                "second".into(),
                binary(BinaryOp::Mul, field("first"), literal(2)),
            ),
        ])];
        assert_eq!(apply(&mut record, &expr), Ok(RecordDisposition::Retained));
        assert_eq!(record.data.get("second"), Some(&Datum::Signed(26)));
        assert_eq!(
            evaluate(&field("absent"), &record.data),
            Err(TransformError::MissingField("absent".into()))
        );
        let big = Expression::Literal(Datum::bigint(i128::MAX.to_string()).unwrap());
        assert_eq!(
            evaluate(&binary(BinaryOp::Add, big, literal(1)), &record.data),
            Err(TransformError::Overflow)
        );
    }
    #[test]
    fn comp_09_python_boolean_and_comparison_short_circuit() {
        let row = sample().data;
        let pred = binary(BinaryOp::Ge, field("metric"), literal(10));
        let not = Expression::Unary {
            op: UnaryOp::Not,
            expr: Box::new(pred.clone()),
        };
        assert_eq!(evaluate(&not, &row), Ok(Datum::Boolean(false)));
        let short = binary(
            BinaryOp::And,
            Expression::Literal(Datum::Boolean(false)),
            field("absent"),
        );
        assert_eq!(evaluate(&short, &row), Ok(Datum::Boolean(false)));
        let branch = binary(BinaryOp::Or, pred, field("absent"));
        assert_eq!(evaluate(&branch, &row), Ok(Datum::Boolean(true)));
        let missing = binary(
            BinaryOp::And,
            Expression::Literal(Datum::Boolean(true)),
            field("absent"),
        );
        assert_eq!(
            evaluate(&missing, &row),
            Err(TransformError::MissingField("absent".into()))
        );
    }
    #[test]
    fn comp_10_drop_rename_missing_and_branch_filter() {
        let mut record = sample();
        assert_eq!(
            apply(
                &mut record,
                &[
                    Transform::Drop(vec!["missing".into()]),
                    Transform::Rename(vec![("absent".into(), "other".into())])
                ]
            ),
            Ok(RecordDisposition::Retained)
        );
        let pred = binary(BinaryOp::Lt, field("metric"), literal(10));
        assert_eq!(
            branch_condition(&record, Some(&pred), 1),
            Ok(RecordDisposition::FilteredForSink { slot: 1 })
        );
    }
}
