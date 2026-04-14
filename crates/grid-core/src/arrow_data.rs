use arrow_array::Array;
use arrow_ipc::reader::StreamReader;
use arrow_schema::{DataType, Field, Schema};
use std::io::Cursor;
use std::sync::Arc;

use crate::types::{ContentAlign, DateTruncation, GridCell, GridColumn};

pub const ROW_ID_COL: &str = "__row_id__";

/// Safely read a Timestamp array value as microseconds since epoch,
/// handling all four Arrow TimeUnit variants without panicking.
fn timestamp_to_micros(array: &std::sync::Arc<dyn arrow_array::Array>, row: usize, unit: &arrow_schema::TimeUnit) -> i64 {
    use arrow_array::cast::as_primitive_array;
    use arrow_array::types::*;
    use arrow_schema::TimeUnit;
    match unit {
        TimeUnit::Second      => as_primitive_array::<TimestampSecondType>(array).value(row) * 1_000_000,
        TimeUnit::Millisecond => as_primitive_array::<TimestampMillisecondType>(array).value(row) * 1_000,
        TimeUnit::Microsecond => as_primitive_array::<TimestampMicrosecondType>(array).value(row),
        TimeUnit::Nanosecond  => as_primitive_array::<TimestampNanosecondType>(array).value(row) / 1_000,
    }
}

/// Truncate a microsecond timestamp to the floor of the given granularity.
fn truncate_micros(val: i64, t: crate::types::DateTruncation) -> i64 {
    use crate::types::DateTruncation;
    const SEC:  i64 = 1_000_000;
    const MIN:  i64 = 60 * SEC;
    const HOUR: i64 = 3600 * SEC;
    const DAY:  i64 = 86_400 * SEC;
    const WEEK: i64 = 7 * DAY;
    match t {
        DateTruncation::Nanosecond | DateTruncation::Microsecond => val,
        DateTruncation::Millisecond => (val / 1_000) * 1_000,
        DateTruncation::Second  => (val / SEC) * SEC,
        DateTruncation::Minute  => (val / MIN) * MIN,
        DateTruncation::Hour    => (val / HOUR) * HOUR,
        DateTruncation::Day     => (val / DAY) * DAY,
        DateTruncation::Week    => {
            let offset = 3 * DAY; // epoch (1970-01-01) is Thursday; offset aligns to Monday
            ((val - offset) / WEEK) * WEEK + offset
        }
        DateTruncation::Month => {
            use chrono::{DateTime, Datelike, TimeZone, Utc};
            match DateTime::from_timestamp(val / SEC, 0) {
                Some(dt) => Utc.with_ymd_and_hms(dt.year(), dt.month(), 1, 0, 0, 0)
                    .unwrap().timestamp() * SEC,
                None => val,
            }
        }
        DateTruncation::Quarter => {
            use chrono::{DateTime, Datelike, TimeZone, Utc};
            match DateTime::from_timestamp(val / SEC, 0) {
                Some(dt) => {
                    let q = ((dt.month() - 1) / 3) * 3 + 1;
                    Utc.with_ymd_and_hms(dt.year(), q, 1, 0, 0, 0).unwrap().timestamp() * SEC
                }
                None => val,
            }
        }
        DateTruncation::Year => {
            use chrono::{DateTime, Datelike, TimeZone, Utc};
            match DateTime::from_timestamp(val / SEC, 0) {
                Some(dt) => Utc.with_ymd_and_hms(dt.year(), 1, 1, 0, 0, 0)
                    .unwrap().timestamp() * SEC,
                None => val,
            }
        }
    }
}

/// Values above this threshold are displayed in scientific notation.
const SCI_NOTATION_UPPER_THRESHOLD: f64 = 1e9;
/// Values below this threshold (and above zero) are displayed in scientific notation.
const SCI_NOTATION_LOWER_THRESHOLD: f64 = 0.001;
/// Number of decimal places used when displaying floats in fixed or scientific notation.
const FLOAT_DISPLAY_PRECISION: usize = 6;
/// Column width multiplier for numeric columns relative to the default width.
const NUMERIC_COL_WIDTH_RATIO: f64 = 0.8;
/// Column width multiplier for boolean columns relative to the default width.
const BOOLEAN_COL_WIDTH_RATIO: f64 = 0.5;
/// Microseconds per second, used for timestamp unit conversion.
const MICROS_PER_SECOND: i64 = 1_000_000;

fn make_row_id_schema(user_schema: &Schema) -> Arc<Schema> {
    let mut fields: Vec<Field> = user_schema.fields().iter().map(|f| (**f).clone()).collect();
    fields.push(Field::new(ROW_ID_COL, DataType::UInt64, false));
    Arc::new(Schema::new(fields))
}

fn append_row_ids(
    batch: &arrow_array::RecordBatch,
    start_id: u64,
    full_schema: &Arc<Schema>,
) -> Result<arrow_array::RecordBatch, String> {
    use arrow_array::builder::UInt64Builder;

    let n = batch.num_rows();
    let mut builder = UInt64Builder::with_capacity(n);
    for i in 0..n {
        builder.append_value(start_id + i as u64);
    }
    let id_array = Arc::new(builder.finish()) as Arc<dyn Array>;

    let mut columns: Vec<Arc<dyn Array>> = batch.columns().to_vec();
    columns.push(id_array);

    arrow_array::RecordBatch::try_new(full_schema.clone(), columns)
        .map_err(|e| format!("Failed to append row IDs: {}", e))
}

/// Stores Arrow RecordBatches and provides efficient cell access.
/// Optionally backed by a DataFusion SessionContext for query execution.
pub struct ArrowDataSource {
    batches: Vec<arrow_array::RecordBatch>,
    schema: Arc<Schema>,
    row_count: usize,
    column_count: usize,
    /// DataFusion context for future query support (aggregations, sorts, filters)
    df_ctx: Option<datafusion::execution::context::SessionContext>,
}

impl ArrowDataSource {
    /// Create from Arrow IPC stream bytes.
    pub fn from_ipc_stream(bytes: &[u8]) -> Result<Self, String> {
        let cursor = Cursor::new(bytes);
        let reader = StreamReader::try_new(cursor, None)
            .map_err(|e| format!("Failed to create IPC stream reader: {}", e))?;

        let user_schema = reader.schema();
        let column_count = user_schema.fields().len();
        let full_schema = make_row_id_schema(&user_schema);

        let mut batches = Vec::new();
        let mut row_offset: u64 = 0;
        for batch_result in reader {
            let batch = batch_result.map_err(|e| format!("Failed to read batch: {}", e))?;
            let n = batch.num_rows() as u64;
            let batch_with_ids = append_row_ids(&batch, row_offset, &full_schema)?;
            batches.push(batch_with_ids);
            row_offset += n;
        }

        let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();

        Ok(Self {
            batches,
            schema: user_schema,
            row_count,
            column_count,
            df_ctx: None,
        })
    }

    /// Create from a single RecordBatch.
    pub fn from_batch(batch: arrow_array::RecordBatch) -> Self {
        let schema = batch.schema();
        let row_count = batch.num_rows();
        let column_count = batch.num_columns();

        Self {
            batches: vec![batch],
            schema,
            row_count,
            column_count,
            df_ctx: None,
        }
    }

    /// Create from multiple RecordBatches that already include the __row_id__ column.
    /// `user_schema` is the schema WITHOUT __row_id__.
    pub fn from_batches_with_ids(
        batches: Vec<arrow_array::RecordBatch>,
        user_schema: Arc<Schema>,
    ) -> Result<Self, String> {
        let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();
        let column_count = user_schema.fields().len();

        Ok(Self {
            batches,
            schema: user_schema,
            row_count,
            column_count,
            df_ctx: None,
        })
    }

    /// Create from multiple RecordBatches (legacy path, no row IDs yet).
    pub fn from_batches(
        batches: Vec<arrow_array::RecordBatch>,
        schema: Arc<Schema>,
    ) -> Result<Self, String> {
        let user_schema = if schema.field_with_name(ROW_ID_COL).is_ok() {
            let fields: Vec<Field> = schema
                .fields()
                .iter()
                .filter(|f| f.name() != ROW_ID_COL)
                .map(|f| (**f).clone())
                .collect();
            Arc::new(Schema::new(fields))
        } else {
            schema.clone()
        };
        let column_count = user_schema.fields().len();
        let full_schema = make_row_id_schema(&user_schema);

        let batches = if schema.field_with_name(ROW_ID_COL).is_ok() {
            batches
        } else {
            let mut row_offset: u64 = 0;
            let mut result = Vec::with_capacity(batches.len());
            for batch in batches {
                let n = batch.num_rows() as u64;
                result.push(append_row_ids(&batch, row_offset, &full_schema)?);
                row_offset += n;
            }
            result
        };

        let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();

        Ok(Self {
            batches,
            schema: user_schema,
            row_count,
            column_count,
            df_ctx: None,
        })
    }

    /// Create from an array of JS objects (passed as serde_json::Value).
    /// Each object is a row, keys are column names.
    /// Inferred types: numbers -> Float64, booleans -> Boolean, everything else -> Utf8.
    pub fn from_json_objects(objects: &[serde_json::Value]) -> Result<Self, String> {
        if objects.is_empty() {
            let schema = Arc::new(Schema::empty());
            return Ok(Self {
                batches: Vec::new(),
                schema,
                row_count: 0,
                column_count: 0,
                df_ctx: None,
            });
        }

        // Collect all unique keys across all objects to determine schema
        let mut keys_order: Vec<String> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for obj in objects {
            if let Some(map) = obj.as_object() {
                for key in map.keys() {
                    if seen.insert(key.clone()) {
                        keys_order.push(key.clone());
                    }
                }
            }
        }

        if keys_order.is_empty() {
            let schema = Arc::new(Schema::empty());
            return Ok(Self {
                batches: Vec::new(),
                schema,
                row_count: 0,
                column_count: 0,
                df_ctx: None,
            });
        }

        // Infer types by scanning first non-null value per column
        let mut col_types: Vec<DataType> = vec![DataType::Utf8; keys_order.len()];
        for obj in objects {
            if let Some(map) = obj.as_object() {
                for (i, key) in keys_order.iter().enumerate() {
                    if let Some(val) = map.get(key) {
                        // Only infer if we haven't already found a non-null value
                        if col_types[i] == DataType::Utf8 {
                            col_types[i] = infer_json_type(val);
                        }
                    }
                }
            }
        }

        // Build schema
        let fields: Vec<Field> = keys_order
            .iter()
            .zip(col_types.iter())
            .map(|(name, dt)| Field::new(name, dt.clone(), true))
            .collect();
        let schema = Arc::new(Schema::new(fields));

        // Build arrays
        use arrow_array::builder::*;
        use arrow_array::*;

        let mut builders: Vec<Box<dyn std::any::Any>> = col_types
            .iter()
            .map(|dt| match dt {
                DataType::Float64 => Box::new(Float64Builder::with_capacity(objects.len()))
                    as Box<dyn std::any::Any>,
                DataType::Int64 => {
                    Box::new(Int64Builder::with_capacity(objects.len())) as Box<dyn std::any::Any>
                }
                DataType::Boolean => {
                    Box::new(BooleanBuilder::with_capacity(objects.len())) as Box<dyn std::any::Any>
                }
                _ => Box::new(StringBuilder::with_capacity(
                    objects.len() * 16,
                    objects.len(),
                )) as Box<dyn std::any::Any>,
            })
            .collect();

        for obj in objects {
            let map = obj.as_object();
            for (i, key) in keys_order.iter().enumerate() {
                let val = map.and_then(|m| m.get(key));
                match col_types[i] {
                    DataType::Float64 => {
                        let builder = builders[i]
                            .downcast_mut::<Float64Builder>()
                            .unwrap();
                        match val {
                            Some(serde_json::Value::Number(n)) => {
                                builder.append_value(n.as_f64().unwrap_or(0.0));
                            }
                            _ => builder.append_null(),
                        }
                    }
                    DataType::Int64 => {
                        let builder = builders[i]
                            .downcast_mut::<Int64Builder>()
                            .unwrap();
                        match val {
                            Some(serde_json::Value::Number(n)) => {
                                builder.append_value(n.as_i64().unwrap_or(0));
                            }
                            _ => builder.append_null(),
                        }
                    }
                    DataType::Boolean => {
                        let builder = builders[i]
                            .downcast_mut::<BooleanBuilder>()
                            .unwrap();
                        match val {
                            Some(serde_json::Value::Bool(b)) => builder.append_value(*b),
                            _ => builder.append_null(),
                        }
                    }
                    _ => {
                        let builder = builders[i]
                            .downcast_mut::<StringBuilder>()
                            .unwrap();
                        match val {
                            Some(v) => builder.append_value(json_value_to_string(v)),
                            _ => builder.append_null(),
                        }
                    }
                }
            }
        }

        // Finalize arrays
        let arrays: Vec<Arc<dyn Array>> = col_types
            .iter()
            .enumerate()
            .map(|(i, dt)| match dt {
                DataType::Float64 => {
                    let builder = builders[i]
                        .downcast_mut::<Float64Builder>()
                        .unwrap();
                    Arc::new(builder.finish()) as Arc<dyn Array>
                }
                DataType::Int64 => {
                    let builder = builders[i]
                        .downcast_mut::<Int64Builder>()
                        .unwrap();
                    Arc::new(builder.finish()) as Arc<dyn Array>
                }
                DataType::Boolean => {
                    let builder = builders[i]
                        .downcast_mut::<BooleanBuilder>()
                        .unwrap();
                    Arc::new(builder.finish()) as Arc<dyn Array>
                }
                _ => {
                    let builder = builders[i]
                        .downcast_mut::<StringBuilder>()
                        .unwrap();
                    Arc::new(builder.finish()) as Arc<dyn Array>
                }
            })
            .collect();

        let user_batch =
            RecordBatch::try_new(schema.clone(), arrays).map_err(|e| e.to_string())?;

        let column_count = schema.fields().len();
        let row_count = user_batch.num_rows();
        let full_schema = make_row_id_schema(&schema);
        let batch = append_row_ids(&user_batch, 0, &full_schema)?;

        Ok(Self {
            batches: vec![batch],
            schema,
            row_count,
            column_count,
            df_ctx: None,
        })
    }

    /// Initialize DataFusion context and register the data as a table.
    /// This enables future query execution (aggregations, sorts, filters).
    pub fn init_datafusion(&mut self, table_name: &str) -> Result<(), String> {
        use datafusion::datasource::MemTable;
        use datafusion::execution::context::SessionContext;

        let ctx = SessionContext::new();
        let full_schema = make_row_id_schema(&self.schema);
        let mem_table = MemTable::try_new(
            full_schema,
            vec![self.batches.clone()],
        )
        .map_err(|e| format!("Failed to create MemTable: {}", e))?;

        ctx.register_table(table_name, Arc::new(mem_table))
            .map_err(|e| format!("Failed to register table: {}", e))?;

        self.df_ctx = Some(ctx);
        Ok(())
    }

    /// Execute a SQL query against the registered data.
    /// Returns a new ArrowDataSource with the query results.
    /// Requires the `datafusion-sql` feature.
    #[cfg(feature = "datafusion-sql")]
    pub async fn execute_query(&self, sql: &str) -> Result<Self, String> {
        let ctx = self
            .df_ctx
            .as_ref()
            .ok_or_else(|| "DataFusion not initialized. Call init_datafusion first.".to_string())?;

        let df = ctx
            .sql(sql)
            .await
            .map_err(|e| format!("SQL execution failed: {}", e))?;

        let batches = df
            .collect()
            .await
            .map_err(|e| format!("Failed to collect results: {}", e))?;

        if batches.is_empty() {
            return Err("Query returned no results".to_string());
        }

        let schema = batches[0].schema();
        Self::from_batches(batches, schema)
    }

    /// Execute a SQL query against the registered data.
    /// Returns a new ArrowDataSource with the query results.
    /// Requires the `datafusion-sql` feature to be enabled on this crate.
    /// Without it, this method returns an error.
    #[cfg(not(feature = "datafusion-sql"))]
    pub async fn execute_query(&self, _sql: &str) -> Result<Self, String> {
        Err("SQL support not compiled in. Enable the 'datafusion-sql' feature.".to_string())
    }

    /// Perform a GROUP BY + aggregate query using DataFusion's DataFrame API.
    /// Returns a new ArrowDataSource with the aggregated result.
    pub async fn group_by(
        &self,
        group_keys: &[crate::types::DateGroupKey],
        aggregations: &[(String, Vec<crate::types::AggregateFunction>)],
    ) -> Result<Self, String> {
        use crate::types::AggregateFunction;
        use datafusion::datasource::MemTable;
        use datafusion::execution::context::SessionContext;
        use datafusion::functions_aggregate::expr_fn::{avg, count, max, min, sum};
        use datafusion::prelude::*;

        if group_keys.is_empty() {
            return Err("No group keys specified".to_string());
        }

        let user_schema = self.schema.clone();
        let ctx = SessionContext::new();
        let table = MemTable::try_new(
            make_row_id_schema(&user_schema),
            vec![self.batches.clone()],
        )
        .map_err(|e| format!("Failed to create MemTable: {}", e))?;

        ctx.register_table("data", Arc::new(table))
            .map_err(|e| format!("Failed to register table: {}", e))?;

        let df = ctx.table("data").await
            .map_err(|e| format!("Failed to read table: {}", e))?;

        // Build group expressions — date columns use date_trunc(), others use col()
        let group_exprs: Vec<Expr> = group_keys.iter().map(|key| {
            match key.truncation {
                None => col(key.arrow_name.as_str()),
                Some(t) => {
                    use datafusion::functions::datetime::expr_fn::date_trunc;
                    date_trunc(lit(t.precision()), col(key.arrow_name.as_str()))
                        .alias(key.result_name())
                }
            }
        }).collect();

        let mut agg_exprs: Vec<Expr> = Vec::new();
        for (col_name, agg_fns) in aggregations {
            for agg_fn in agg_fns {
                let alias = agg_fn.alias(col_name);
                let expr = match agg_fn {
                    AggregateFunction::Count => count(lit(1u8)).alias(alias),
                    AggregateFunction::Sum => sum(col(col_name.as_str())).alias(alias),
                    AggregateFunction::Min => min(col(col_name.as_str())).alias(alias),
                    AggregateFunction::Max => max(col(col_name.as_str())).alias(alias),
                    AggregateFunction::Mean => avg(col(col_name.as_str())).alias(alias),
                };
                agg_exprs.push(expr);
            }
        }

        if agg_exprs.is_empty() {
            return Err("No aggregations specified".to_string());
        }

        let grouped = df.aggregate(group_exprs, agg_exprs)
            .map_err(|e| format!("Aggregate failed: {}", e))?;

        let batches = grouped.collect().await
            .map_err(|e| format!("Failed to collect grouped result: {}", e))?;

        if batches.is_empty() {
            return Err("Group by returned no results".to_string());
        }

        let result_schema = batches[0].schema();
        Self::from_batches(batches, result_schema)
    }

    /// A filter predicate for a single group key column.
    /// `result_col` is the aliased result name (e.g. "created_at_month").
    /// `key` is the DateGroupKey that produced it (used to reconstruct the date_trunc expr).
    /// `display_value` is the string from the aggregate row — parsed back to a timestamp to build
    /// an equality filter on the original source column.
    pub(crate) fn build_filter_expr(
        key: &crate::types::DateGroupKey,
        display_value: &str,
        source_schema: &arrow_schema::Schema,
    ) -> Option<datafusion::prelude::Expr> {
        use datafusion::prelude::*;
        use datafusion::functions::datetime::expr_fn::date_trunc;
        use datafusion::scalar::ScalarValue;
        use arrow_schema::DataType;

        let source_dtype = source_schema.field_with_name(&key.arrow_name)
            .map(|f| f.data_type().clone())
            .unwrap_or(DataType::Utf8);

        if key.truncation.is_some() {
            let ts_micros = Self::parse_display_as_ts_micros(display_value)?;
            let ts_nanos = ts_micros.checked_mul(1_000)?;
            let ts_lit = lit(ScalarValue::TimestampNanosecond(Some(ts_nanos), None));
            let lhs = date_trunc(lit(key.truncation.unwrap().precision()), col(key.arrow_name.as_str()));
            Some(lhs.eq(ts_lit))
        } else {
            // No truncation: direct equality using the column's actual type.
            let rhs = match &source_dtype {
                DataType::Boolean => {
                    lit(display_value.eq_ignore_ascii_case("true"))
                }
                DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 => {
                    let v = display_value.parse::<i64>().ok()?;
                    lit(v)
                }
                DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => {
                    let v = display_value.parse::<u64>().ok()?;
                    lit(v)
                }
                DataType::Float32 | DataType::Float64 => {
                    let v = display_value.parse::<f64>().ok()?;
                    lit(v)
                }
                DataType::Timestamp(_, _) => {
                    let ts_micros = Self::parse_display_as_ts_micros(display_value)?;
                    let utc: Arc<str> = Arc::from("UTC");
                    lit(ScalarValue::TimestampMicrosecond(Some(ts_micros), Some(utc)))
                }
                _ => lit(display_value),
            };
            Some(col(key.arrow_name.as_str()).eq(rhs))
        }
    }

    fn parse_display_as_ts_micros(display_value: &str) -> Option<i64> {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(display_value) {
            return Some(dt.timestamp_micros());
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(display_value, "%Y-%m-%d %H:%M:%S") {
            return Some(dt.and_utc().timestamp_micros());
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(display_value, "%Y-%m-%d") {
            return Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_micros());
        }
        None
    }

    /// Filter source data to rows matching the given group key values, then run a sub-level
    /// GROUP BY + aggregate. Returns the sub-level aggregated ArrowDataSource sorted by
    /// the first group key ascending.
    pub async fn filter_and_group(
        &self,
        filters: &[(crate::types::DateGroupKey, String)],
        group_keys: &[crate::types::DateGroupKey],
        aggregations: &[(String, Vec<crate::types::AggregateFunction>)],
    ) -> Result<Self, String> {
        use crate::types::AggregateFunction;
        use datafusion::datasource::MemTable;
        use datafusion::execution::context::SessionContext;
        use datafusion::functions_aggregate::expr_fn::{avg, count, max, min, sum};
        use datafusion::prelude::*;

        let user_schema = self.schema.clone();
        let ctx = SessionContext::new();
        let table = MemTable::try_new(
            make_row_id_schema(&user_schema),
            vec![self.batches.clone()],
        ).map_err(|e| format!("MemTable error: {}", e))?;
        ctx.register_table("data", Arc::new(table))
            .map_err(|e| format!("Register error: {}", e))?;

        let mut df = ctx.table("data").await
            .map_err(|e| format!("Table error: {}", e))?;

        for (key, value) in filters {
            if let Some(pred) = Self::build_filter_expr(key, value, &user_schema) {
                df = df.filter(pred).map_err(|e| format!("Filter error: {}", e))?;
            }
        }

        let group_exprs: Vec<Expr> = group_keys.iter().map(|key| {
            match key.truncation {
                None => col(key.arrow_name.as_str()),
                Some(t) => {
                    use datafusion::functions::datetime::expr_fn::date_trunc;
                    date_trunc(lit(t.precision()), col(key.arrow_name.as_str()))
                        .alias(key.result_name())
                }
            }
        }).collect();

        let mut agg_exprs: Vec<Expr> = Vec::new();
        for (col_name, agg_fns) in aggregations {
            for agg_fn in agg_fns {
                let alias = agg_fn.alias(col_name);
                let expr = match agg_fn {
                    AggregateFunction::Count => count(lit(1u8)).alias(alias),
                    AggregateFunction::Sum => sum(col(col_name.as_str())).alias(alias),
                    AggregateFunction::Min => min(col(col_name.as_str())).alias(alias),
                    AggregateFunction::Max => max(col(col_name.as_str())).alias(alias),
                    AggregateFunction::Mean => avg(col(col_name.as_str())).alias(alias),
                };
                agg_exprs.push(expr);
            }
        }

        let grouped = df.aggregate(group_exprs.clone(), agg_exprs)
            .map_err(|e| format!("Aggregate error: {}", e))?;

        let sorted = if let Some(first) = group_exprs.first() {
            grouped.sort(vec![first.clone().sort(true, true)])
                .map_err(|e| format!("Sort error: {}", e))?
        } else {
            grouped
        };

        let batches = sorted.collect().await
            .map_err(|e| format!("Collect error: {}", e))?;
        if batches.is_empty() {
            return Err("No results".to_string());
        }
        let schema = batches[0].schema();
        Self::from_batches(batches, schema)
    }

    /// Filter source data to raw rows matching the given group key values.
    /// Returns the matching rows sorted by the first group key ascending.
    pub async fn filter_raw(
        &self,
        filters: &[(crate::types::DateGroupKey, String)],
    ) -> Result<Self, String> {
        use datafusion::datasource::MemTable;
        use datafusion::execution::context::SessionContext;
        use datafusion::prelude::*;

        let user_schema = self.schema.clone();
        let ctx = SessionContext::new();
        let table = MemTable::try_new(
            make_row_id_schema(&user_schema),
            vec![self.batches.clone()],
        ).map_err(|e| format!("MemTable error: {}", e))?;
        ctx.register_table("data", Arc::new(table))
            .map_err(|e| format!("Register error: {}", e))?;

        let mut df = ctx.table("data").await
            .map_err(|e| format!("Table error: {}", e))?;

        for (key, value) in filters {
            if let Some(pred) = Self::build_filter_expr(key, value, &user_schema) {
                df = df.filter(pred).map_err(|e| format!("Filter error: {}", e))?;
            }
        }

        if !filters.is_empty() {
            let first_key = &filters[0].0;
            let sort_expr = match first_key.truncation {
                None => col(first_key.arrow_name.as_str()).sort(true, true),
                Some(t) => {
                    use datafusion::functions::datetime::expr_fn::date_trunc;
                    date_trunc(lit(t.precision()), col(first_key.arrow_name.as_str()))
                        .sort(true, true)
                }
            };
            df = df.sort(vec![sort_expr]).map_err(|e| format!("Sort error: {}", e))?;
        }

        let batches = df.collect().await
            .map_err(|e| format!("Collect error: {}", e))?;
        if batches.is_empty() {
            return Self::from_batches(vec![], user_schema);
        }
        let schema = batches[0].schema();
        Self::from_batches(batches, schema)
    }

    /// Return a sub-datasource containing only rows where column `col_idx` has the
    /// given display value. Pure in-memory scan — no DataFusion query needed.
    pub fn rows_matching_column_value(&self, col_idx: usize, value: &str) -> Self {
        let mut row_indices: Vec<usize> = Vec::new();
        for row in 0..self.row_count {
            if self.get_cell_raw_text(col_idx, row) == value {
                row_indices.push(row);
            }
        }
        let mut sub_batches: Vec<arrow_array::RecordBatch> = Vec::new();
        let mut global_row = 0usize;
        for batch in &self.batches {
            let batch_len = batch.num_rows();
            let mut mask = Vec::with_capacity(batch_len);
            let mut idx_iter = row_indices.iter().peekable();
            while idx_iter.peek().map(|&&i| i < global_row).unwrap_or(false) {
                idx_iter.next();
            }
            for local_row in 0..batch_len {
                let global = global_row + local_row;
                if idx_iter.peek().copied() == Some(&global) {
                    mask.push(true);
                    idx_iter.next();
                } else {
                    mask.push(false);
                }
            }
            global_row += batch_len;
            let bool_arr = arrow_array::BooleanArray::from(mask);
            if let Ok(filtered) = arrow::compute::filter_record_batch(batch, &bool_arr) {
                if filtered.num_rows() > 0 {
                    sub_batches.push(filtered);
                }
            }
        }
        if sub_batches.is_empty() {
            return Self::from_batches(vec![], self.schema.clone()).unwrap_or_else(|_| Self {
                batches: vec![], schema: self.schema.clone(), row_count: 0, column_count: 0, df_ctx: None,
            });
        }
        let schema = sub_batches[0].schema();
        Self::from_batches(sub_batches, schema).unwrap_or_else(|_| Self {
            batches: vec![], schema: self.schema.clone(), row_count: 0, column_count: 0, df_ctx: None,
        })
    }

    /// Partition this data source by the unique display values of column `col_idx`.
    /// Returns a vec of `(display_value, sub_datasource)` pairs — one entry per unique value,
    /// in the order they first appear.  Each sub-datasource contains only the rows for that value.
    pub fn partition_by_column(&self, col_idx: usize) -> Vec<(String, Self)> {
        let n = self.row_count;
        if n == 0 || col_idx >= self.column_count {
            return Vec::new();
        }

        // Walk rows once, collecting row indices grouped by their display value.
        let mut order: Vec<String> = Vec::new();
        let mut groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();

        for row in 0..n {
            let val = self.get_cell_raw_text(col_idx, row);
            let entry = groups.entry(val.clone());
            if matches!(entry, std::collections::hash_map::Entry::Vacant(_)) {
                order.push(val.clone());
            }
            entry.or_default().push(row);
        }

        // Build one sub-ArrowDataSource per group using Arrow's `filter` kernel.
        let mut result: Vec<(String, Self)> = Vec::new();
        for key_val in order {
            let row_indices = match groups.get(&key_val) {
                Some(v) => v,
                None => continue,
            };

            // Build a boolean mask for this group across all batches.
            let mut sub_batches: Vec<arrow_array::RecordBatch> = Vec::new();
            let mut global_row = 0usize;
            for batch in &self.batches {
                let batch_len = batch.num_rows();
                let mut mask = Vec::with_capacity(batch_len);
                let mut idx_iter = row_indices.iter().peekable();
                // Skip indices before this batch
                while idx_iter.peek().map(|&&i| i < global_row).unwrap_or(false) {
                    idx_iter.next();
                }
                for local_row in 0..batch_len {
                    let global = global_row + local_row;
                    if idx_iter.peek().copied() == Some(&global) {
                        mask.push(true);
                        idx_iter.next();
                    } else {
                        mask.push(false);
                    }
                }
                global_row += batch_len;

                let bool_arr = arrow_array::BooleanArray::from(mask);
                if let Ok(filtered) = arrow::compute::filter_record_batch(batch, &bool_arr) {
                    if filtered.num_rows() > 0 {
                        sub_batches.push(filtered);
                    }
                }
            }

            if sub_batches.is_empty() { continue; }
            let schema = sub_batches[0].schema();
            if let Ok(sub_ds) = Self::from_batches(sub_batches, schema) {
                result.push((key_val, sub_ds));
            }
        }

        result
    }

    /// Extract all non-null values in a date/datetime column as microseconds since epoch.
    /// Handles Date32 (days), Date64 (ms), Timestamp (various units).
    pub fn column_as_micros(&self, col: usize) -> Vec<i64> {
        use arrow_array::cast::as_primitive_array;
        use arrow_array::types::*;
        use arrow_schema::DataType;

        if col >= self.column_count { return Vec::new(); }
        let dtype = self.schema.field(col).data_type().clone();
        let mut result = Vec::new();

        macro_rules! collect_as_micros {
            ($ty:ty, $factor:expr) => {{
                for batch in &self.batches {
                    if col >= batch.num_columns() { continue; }
                    let arr = as_primitive_array::<$ty>(batch.column(col));
                    for i in 0..arr.len() {
                        if !arr.is_null(i) {
                            result.push(arr.value(i) as i64 * $factor);
                        }
                    }
                }
            }};
        }

        match dtype {
            DataType::Date32 => collect_as_micros!(Date32Type, 86_400_000_000),
            DataType::Date64 => collect_as_micros!(Date64Type, 1_000),
            DataType::Timestamp(arrow_schema::TimeUnit::Second, _) =>
                collect_as_micros!(TimestampSecondType, 1_000_000),
            DataType::Timestamp(arrow_schema::TimeUnit::Millisecond, _) =>
                collect_as_micros!(TimestampMillisecondType, 1_000),
            DataType::Timestamp(arrow_schema::TimeUnit::Microsecond, _) =>
                collect_as_micros!(TimestampMicrosecondType, 1),
            DataType::Timestamp(arrow_schema::TimeUnit::Nanosecond, _) => {
                for batch in &self.batches {
                    if col >= batch.num_columns() { continue; }
                    let arr = as_primitive_array::<TimestampNanosecondType>(batch.column(col));
                    for i in 0..arr.len() {
                        if !arr.is_null(i) {
                            result.push(arr.value(i) / 1_000);
                        }
                    }
                }
            }
            // String columns storing ISO 8601 datetimes (e.g. "2022-01-01T00:00:00Z")
            DataType::Utf8 | DataType::LargeUtf8 => {
                use chrono::DateTime;
                for batch in &self.batches {
                    if col >= batch.num_columns() { continue; }
                    let arr = if matches!(dtype, DataType::LargeUtf8) {
                        use arrow_array::cast::as_largestring_array;
                        as_largestring_array(batch.column(col)).iter()
                            .filter_map(|v| v)
                            .filter_map(|s| DateTime::parse_from_rfc3339(s).ok())
                            .for_each(|dt| result.push(dt.timestamp_micros()));
                        continue;
                    } else {
                        use arrow_array::cast::as_string_array;
                        as_string_array(batch.column(col)).iter()
                            .filter_map(|v| v)
                            .filter_map(|s| DateTime::parse_from_rfc3339(s).ok())
                            .for_each(|dt| result.push(dt.timestamp_micros()));
                        continue;
                    };
                    let _ = arr;
                }
            }
            _ => {}
        }
        result
    }

    /// Infer useful date truncation levels for a column.
    /// Returns (available_truncations, is_estimate).
    /// is_estimate = true means we assumed sorted order for the smallest-tick calculation.
    pub fn infer_date_truncations(&self, col: usize) -> (Vec<crate::types::DateTruncation>, bool) {
        use crate::types::DateTruncation;

        let values = self.column_as_micros(col);
        if values.is_empty() {
            return (DateTruncation::all_coarsest_first().to_vec(), true);
        }

        let min_val = values[0];
        let max_val = *values.last().unwrap();

        // HIGH-END: exclude truncations where trunc(first) == trunc(last)
        // — the whole dataset falls within a single bucket of that granularity.
        let dont_offer_coarse: Vec<DateTruncation> = DateTruncation::all_coarsest_first()
            .iter()
            .filter(|&&t| truncate_micros(min_val, t) == truncate_micros(max_val, t))
            .copied()
            .collect();

        // LOW-END: smallest non-zero diff between consecutive values (assumes sorted).
        let smallest_tick: Option<i64> = values.windows(2)
            .filter_map(|w| {
                let d = (w[1] - w[0]).abs();
                if d > 0 { Some(d) } else { None }
            })
            .min();

        let dont_offer_fine: Vec<DateTruncation> = if let Some(tick) = smallest_tick {
            DateTruncation::all_coarsest_first()
                .iter()
                .filter(|&&t| t.duration_micros() <= tick)
                .copied()
                .collect()
        } else {
            Vec::new()
        };

        let available: Vec<DateTruncation> = DateTruncation::all_coarsest_first()
            .iter()
            .filter(|t| !dont_offer_coarse.contains(t) && !dont_offer_fine.contains(t))
            .copied()
            .collect();

        (available, true) // always an estimate (assumes sorted)
    }

    pub fn num_rows(&self) -> usize {
        self.row_count
    }

    pub fn num_columns(&self) -> usize {
        self.column_count
    }

    /// Compute min and max for a numeric column by index (user column, not __row_id__).
    pub fn column_min_max(&self, col: usize) -> Option<(f64, f64)> {
        use arrow_array::cast::as_primitive_array;
        use arrow_array::types::*;
        use arrow_schema::DataType;

        if col >= self.column_count { return None; }
        let dtype = self.schema.field(col).data_type().clone();

        let mut global_min = f64::INFINITY;
        let mut global_max = f64::NEG_INFINITY;

        macro_rules! scan_numeric {
            ($ty:ty) => {{
                for batch in &self.batches {
                    if col >= batch.num_columns() { continue; }
                    let arr = as_primitive_array::<$ty>(batch.column(col));
                    for i in 0..arr.len() {
                        if !arr.is_null(i) {
                            let v = arr.value(i) as f64;
                            if v < global_min { global_min = v; }
                            if v > global_max { global_max = v; }
                        }
                    }
                }
            }};
        }

        match dtype {
            DataType::Int8   => scan_numeric!(Int8Type),
            DataType::Int16  => scan_numeric!(Int16Type),
            DataType::Int32  => scan_numeric!(Int32Type),
            DataType::Int64  => scan_numeric!(Int64Type),
            DataType::UInt8  => scan_numeric!(UInt8Type),
            DataType::UInt16 => scan_numeric!(UInt16Type),
            DataType::UInt32 => scan_numeric!(UInt32Type),
            DataType::UInt64 => scan_numeric!(UInt64Type),
            DataType::Float32 => scan_numeric!(Float32Type),
            DataType::Float64 => scan_numeric!(Float64Type),
            _ => return None,
        }

        if global_min.is_infinite() { None } else { Some((global_min, global_max)) }
    }

    /// Compute approximate percentile bounds for a numeric column.
    /// Returns (p_low, p_high) where p_low is the `low_pct` percentile value
    /// and p_high is the `high_pct` percentile value (e.g. 0.05 and 0.95).
    pub fn column_percentiles(&self, col: usize, low_pct: f64, high_pct: f64) -> Option<(f64, f64)> {
        use arrow_array::cast::as_primitive_array;
        use arrow_array::types::*;
        use arrow_schema::DataType;

        if col >= self.column_count { return None; }
        let dtype = self.schema.field(col).data_type().clone();

        let mut values: Vec<f64> = Vec::new();

        macro_rules! collect_numeric {
            ($ty:ty) => {{
                for batch in &self.batches {
                    if col < batch.num_columns() {
                        let arr = as_primitive_array::<$ty>(batch.column(col));
                        for i in 0..arr.len() {
                            if !arr.is_null(i) {
                                values.push(arr.value(i) as f64);
                            }
                        }
                    }
                }
            }};
        }

        match dtype {
            DataType::Int8   => collect_numeric!(Int8Type),
            DataType::Int16  => collect_numeric!(Int16Type),
            DataType::Int32  => collect_numeric!(Int32Type),
            DataType::Int64  => collect_numeric!(Int64Type),
            DataType::UInt8  => collect_numeric!(UInt8Type),
            DataType::UInt16 => collect_numeric!(UInt16Type),
            DataType::UInt32 => collect_numeric!(UInt32Type),
            DataType::UInt64 => collect_numeric!(UInt64Type),
            DataType::Float32 => collect_numeric!(Float32Type),
            DataType::Float64 => collect_numeric!(Float64Type),
            _ => return None,
        }

        if values.is_empty() { return None; }

        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = values.len();
        let low_idx  = ((n as f64 * low_pct).floor() as usize).min(n - 1);
        let high_idx = ((n as f64 * high_pct).ceil() as usize).min(n - 1);
        Some((values[low_idx], values[high_idx]))
    }

    /// Get unique sorted string values for a column (for value picker UI).
    /// Returns up to `limit` values; is_truncated = true if more exist.
    pub fn column_unique_values(&self, col: usize, limit: usize) -> (Vec<String>, bool) {
        use std::collections::BTreeSet;

        let mut seen = BTreeSet::new();
        let mut truncated = false;

        for batch in &self.batches {
            if col >= batch.num_columns() { continue; }
            let array = batch.column(col);
            for row in 0..array.len() {
                if array.is_null(row) { continue; }
                let s = format_cell_display(array, row, self.schema.field(col).data_type());
                if seen.len() < limit {
                    seen.insert(s);
                } else {
                    truncated = true;
                    break;
                }
            }
            if truncated { break; }
        }

        (seen.into_iter().collect(), truncated)
    }

    pub fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    /// Sort the data by a column using DataFusion's DataFrame API.
    /// Returns a new ArrowDataSource with sorted data (row IDs are preserved).
    pub async fn sort_by_column(&self, col_index: usize, ascending: bool) -> Result<Self, String> {
        use datafusion::datasource::MemTable;
        use datafusion::execution::context::SessionContext;
        use datafusion::prelude::*;

        let col_name = self.column_name(col_index);
        if col_name.is_empty() {
            return Err(format!("Column index {} out of bounds", col_index));
        }

        // Register with full schema (including __row_id__)
        let full_schema = make_row_id_schema(&self.schema);

        let ctx = SessionContext::new();
        let table = MemTable::try_new(
            full_schema.clone(),
            vec![self.batches.clone()],
        )
        .map_err(|e| format!("Failed to create MemTable: {}", e))?;

        ctx.register_table("data", Arc::new(table))
            .map_err(|e| format!("Failed to register table: {}", e))?;

        let df = ctx.table("data").await
            .map_err(|e| format!("Failed to read table: {}", e))?;

        let sorted = df.sort(vec![
            col(col_name).sort(ascending, false),
        ])
        .map_err(|e| format!("Sort failed: {}", e))?;

        let batches = sorted.collect().await
            .map_err(|e| format!("Failed to collect sorted results: {}", e))?;

        if batches.is_empty() {
            return Err("Sort returned no results".to_string());
        }

        Self::from_batches_with_ids(batches, self.schema.clone())
    }

    /// Given a row ID (from __row_id__), find its current display row index.
    /// Returns None if not found.
    pub fn find_row_by_id(&self, row_id: u64) -> Option<usize> {
        use arrow_array::cast::as_primitive_array;
        use arrow_array::types::UInt64Type;

        let id_col_idx = self.column_count; // __row_id__ is always last
        let mut global_row = 0usize;

        for batch in &self.batches {
            if id_col_idx >= batch.num_columns() {
                global_row += batch.num_rows();
                continue;
            }
            let arr = batch.column(id_col_idx);
            let ids = as_primitive_array::<UInt64Type>(arr);
            for local_row in 0..batch.num_rows() {
                if ids.value(local_row) == row_id {
                    return Some(global_row + local_row);
                }
            }
            global_row += batch.num_rows();
        }
        None
    }

    /// Get the __row_id__ value for a given display row index.
    pub fn get_row_id(&self, row: usize) -> Option<u64> {
        use arrow_array::cast::as_primitive_array;
        use arrow_array::types::UInt64Type;

        let (batch, local_row) = self.find_batch_row(row);
        let id_col_idx = self.column_count;
        if id_col_idx >= batch.num_columns() {
            return None;
        }
        let arr = batch.column(id_col_idx);
        let ids = as_primitive_array::<UInt64Type>(arr);
        Some(ids.value(local_row))
    }

    /// Get the GridColumn definitions derived from the Arrow schema.
    pub fn to_grid_columns(&self, default_width: f64) -> Vec<GridColumn> {
        self.schema
            .fields()
            .iter()
            .map(|field| {
                let (icon_hint, width_mult) = match field.data_type() {
                    DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float16
                    | DataType::Float32
                    | DataType::Float64 => ("number".to_string(), NUMERIC_COL_WIDTH_RATIO),
                    DataType::Boolean => ("boolean".to_string(), BOOLEAN_COL_WIDTH_RATIO),
                    DataType::Utf8 | DataType::LargeUtf8 => ("string".to_string(), 1.0),
                    _ => ("string".to_string(), 1.0),
                };

                GridColumn {
                    title: field.name().clone(),
                    width: default_width * width_mult,
                    group: None,
                    icon: Some(icon_hint),
                    id: None,
                }
            })
            .collect()
    }

    /// Get a cell's raw value as a plain string for clipboard export.
    /// Numbers returned as raw floats, dates as ISO strings, strings as-is.
    pub fn get_cell_raw_text(&self, col: usize, row: usize) -> String {
        use arrow_array::cast::*;
        use arrow_array::types::*;

        if col >= self.column_count || row >= self.row_count {
            return String::new();
        }
        let (batch, local_row) = self.find_batch_row(row);
        if col >= batch.num_columns() {
            return String::new();
        }
        let array = batch.column(col);
        if array.is_null(local_row) {
            return String::new();
        }
        let field = self.schema.field(col);
        match field.data_type() {
            DataType::Int8 => as_primitive_array::<Int8Type>(array).value(local_row).to_string(),
            DataType::Int16 => as_primitive_array::<Int16Type>(array).value(local_row).to_string(),
            DataType::Int32 => as_primitive_array::<Int32Type>(array).value(local_row).to_string(),
            DataType::Int64 => as_primitive_array::<Int64Type>(array).value(local_row).to_string(),
            DataType::UInt8 => as_primitive_array::<UInt8Type>(array).value(local_row).to_string(),
            DataType::UInt16 => as_primitive_array::<UInt16Type>(array).value(local_row).to_string(),
            DataType::UInt32 => as_primitive_array::<UInt32Type>(array).value(local_row).to_string(),
            DataType::UInt64 => as_primitive_array::<UInt64Type>(array).value(local_row).to_string(),
            DataType::Float32 => as_primitive_array::<Float32Type>(array).value(local_row).to_string(),
            DataType::Float64 => as_primitive_array::<Float64Type>(array).value(local_row).to_string(),
            DataType::Boolean => as_boolean_array(array).value(local_row).to_string(),
            DataType::Utf8 => as_string_array(array).value(local_row).to_string(),
            DataType::LargeUtf8 => as_largestring_array(array).value(local_row).to_string(),
            DataType::Date32 => {
                use arrow::temporal_conversions::date32_to_datetime;
                let val = as_primitive_array::<Date32Type>(array).value(local_row);
                date32_to_datetime(val)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| val.to_string())
            }
            DataType::Date64 => {
                use arrow::temporal_conversions::date64_to_datetime;
                let val = as_primitive_array::<Date64Type>(array).value(local_row);
                date64_to_datetime(val)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| val.to_string())
            }
            DataType::Timestamp(unit, _) => {
                let micros = timestamp_to_micros(array, local_row, unit);
                let secs = micros / MICROS_PER_SECOND;
                let nsecs = ((micros % MICROS_PER_SECOND) * 1000).unsigned_abs() as u32;
                chrono::DateTime::from_timestamp(secs, nsecs)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                    .unwrap_or_else(|| micros.to_string())
            }
            _ => format!("{:?}", array.slice(local_row, 1)),
        }
    }

    /// Get a cell value as a GridCell.
    pub fn get_cell(&self, col: usize, row: usize) -> GridCell {
        if col >= self.column_count || row >= self.row_count {
            return GridCell::loading();
        }

        let (batch, local_row) = self.find_batch_row(row);

        if col >= batch.num_columns() {
            return GridCell::loading();
        }

        let array = batch.column(col);
        if array.is_null(local_row) {
            return GridCell::Text {
                data: String::new(),
                display_data: Some(String::new()),
                content_align: None,
            };
        }

        let field = self.schema.field(col);
        format_cell(array, local_row, field.data_type())
    }

    /// Get a cell's display string directly.
    pub fn get_cell_display(&self, col: usize, row: usize) -> String {
        if col >= self.column_count || row >= self.row_count {
            return String::new();
        }

        let (batch, local_row) = self.find_batch_row(row);
        if col >= batch.num_columns() {
            return String::new();
        }

        let array = batch.column(col);
        if array.is_null(local_row) {
            return String::new();
        }

        let field = self.schema.field(col);
        format_cell_display(array, local_row, field.data_type())
    }

    /// Get the column name.
    pub fn column_name(&self, col: usize) -> &str {
        if col < self.column_count {
            self.schema.field(col).name()
        } else {
            ""
        }
    }

    /// Get the column data type.
    pub fn column_type(&self, col: usize) -> &DataType {
        if col < self.column_count {
            self.schema.field(col).data_type()
        } else {
            &DataType::Utf8
        }
    }

    /// Find which batch and local row index a global row maps to.
    fn find_batch_row(&self, row: usize) -> (&arrow_array::RecordBatch, usize) {
        let mut remaining = row;
        for batch in &self.batches {
            if remaining < batch.num_rows() {
                return (batch, remaining);
            }
            remaining -= batch.num_rows();
        }
        let last = self.batches.last().unwrap();
        (last, last.num_rows() - 1)
    }

    /// Get raw bytes of the data as Arrow IPC stream (for transfer).
    /// Write user columns only (strips __row_id__).
    pub fn to_ipc_stream(&self) -> Result<Vec<u8>, String> {
        use arrow_ipc::writer::StreamWriter;
        let mut buf = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buf, &self.schema)
                .map_err(|e| format!("Failed to create IPC writer: {}", e))?;
            for batch in &self.batches {
                // Project to user columns only (drop __row_id__ which is last)
                let user_batch = batch
                    .project(&(0..self.column_count).collect::<Vec<_>>())
                    .map_err(|e| format!("Failed to project batch: {}", e))?;
                writer
                    .write(&user_batch)
                    .map_err(|e| format!("Failed to write batch: {}", e))?;
            }
            writer
                .finish()
                .map_err(|e| format!("Failed to finish IPC writer: {}", e))?;
        }
        Ok(buf)
    }
}

/// Infer Arrow DataType from a JSON value.
fn infer_json_type(val: &serde_json::Value) -> DataType {
    match val {
        serde_json::Value::Number(n) => {
            if n.is_i64() {
                DataType::Int64
            } else {
                DataType::Float64
            }
        }
        serde_json::Value::Bool(_) => DataType::Boolean,
        _ => DataType::Utf8,
    }
}

/// Convert a JSON value to a string representation.
fn json_value_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => val.to_string(),
    }
}

/// Format a cell value as a GridCell (with kind tag for JS).
fn format_cell(array: &Arc<dyn Array>, row: usize, dtype: &DataType) -> GridCell {
    use arrow_array::cast::*;
    use arrow_array::types::*;

    match dtype {
        DataType::Int8 => {
            let val = as_primitive_array::<Int8Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(val.to_string()),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::Int16 => {
            let val = as_primitive_array::<Int16Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(val.to_string()),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::Int32 => {
            let val = as_primitive_array::<Int32Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(val.to_string()),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::Int64 => {
            let val = as_primitive_array::<Int64Type>(array).value(row);
            let display = format_int64(val);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(display),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::UInt8 => {
            let val = as_primitive_array::<UInt8Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(val.to_string()),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::UInt16 => {
            let val = as_primitive_array::<UInt16Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(val.to_string()),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::UInt32 => {
            let val = as_primitive_array::<UInt32Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(val.to_string()),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::UInt64 => {
            let val = as_primitive_array::<UInt64Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(val.to_string()),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::Float32 => {
            let val = as_primitive_array::<Float32Type>(array).value(row);
            GridCell::Number {
                data: Some(val as f64),
                display_data: Some(format_float(val as f64)),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::Float64 => {
            let val = as_primitive_array::<Float64Type>(array).value(row);
            GridCell::Number {
                data: Some(val),
                display_data: Some(format_float(val)),
                content_align: Some(ContentAlign::Right),
            }
        }
        DataType::Boolean => {
            let val = as_boolean_array(array).value(row);
            GridCell::Text {
                data: if val {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
                display_data: None,
                content_align: Some(ContentAlign::Center),
            }
        }
        DataType::Utf8 => {
            let val = as_string_array(array).value(row);
            GridCell::Text {
                data: val.to_string(),
                display_data: None,
                content_align: None,
            }
        }
        DataType::LargeUtf8 => {
            let val = as_largestring_array(array).value(row);
            GridCell::Text {
                data: val.to_string(),
                display_data: None,
                content_align: None,
            }
        }
        DataType::Date32 => {
            use arrow::temporal_conversions::date32_to_datetime;
            let val = as_primitive_array::<Date32Type>(array).value(row);
            let display = date32_to_datetime(val)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| val.to_string());
            GridCell::Text {
                data: display,
                display_data: None,
                content_align: None,
            }
        }
        DataType::Date64 => {
            use arrow::temporal_conversions::date64_to_datetime;
            let val = as_primitive_array::<Date64Type>(array).value(row);
            let display = date64_to_datetime(val)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| val.to_string());
            GridCell::Text {
                data: display,
                display_data: None,
                content_align: None,
            }
        }
        DataType::Timestamp(unit, _) => {
            let micros = timestamp_to_micros(array, row, unit);
            let secs = micros / MICROS_PER_SECOND;
            let nsecs = ((micros % MICROS_PER_SECOND) * 1000).unsigned_abs() as u32;
            let display = chrono::DateTime::from_timestamp(secs, nsecs)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| micros.to_string());
            GridCell::Number {
                data: Some(micros as f64),
                display_data: Some(display),
                content_align: None,
            }
        }
        _ => {
            // Fallback: use Debug representation
            GridCell::Text {
                data: format!("{:?}", array.slice(row, 1)),
                display_data: None,
                content_align: None,
            }
        }
    }
}

/// Format a cell as a plain display string.
fn format_cell_display(array: &Arc<dyn Array>, row: usize, dtype: &DataType) -> String {
    use arrow_array::cast::*;
    use arrow_array::types::*;

    match dtype {
        DataType::Int8 => as_primitive_array::<Int8Type>(array).value(row).to_string(),
        DataType::Int16 => as_primitive_array::<Int16Type>(array).value(row).to_string(),
        DataType::Int32 => as_primitive_array::<Int32Type>(array).value(row).to_string(),
        DataType::Int64 => format_int64(as_primitive_array::<Int64Type>(array).value(row)),
        DataType::UInt8 => as_primitive_array::<UInt8Type>(array).value(row).to_string(),
        DataType::UInt16 => as_primitive_array::<UInt16Type>(array).value(row).to_string(),
        DataType::UInt32 => as_primitive_array::<UInt32Type>(array).value(row).to_string(),
        DataType::UInt64 => as_primitive_array::<UInt64Type>(array).value(row).to_string(),
        DataType::Float32 => {
            format_float(as_primitive_array::<Float32Type>(array).value(row) as f64)
        }
        DataType::Float64 => format_float(as_primitive_array::<Float64Type>(array).value(row)),
        DataType::Boolean => {
            if as_boolean_array(array).value(row) {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        DataType::Utf8 => as_string_array(array).value(row).to_string(),
        DataType::LargeUtf8 => as_largestring_array(array).value(row).to_string(),
        _ => format!("{:?}", array.slice(row, 1)),
    }
}

fn format_int64(val: i64) -> String {
    if val < 0 {
        format!("-{}", format_int64(-val))
    } else if val >= 1000 {
        let s = val.to_string();
        let mut result = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        result.chars().rev().collect()
    } else {
        val.to_string()
    }
}

fn format_float(val: f64) -> String {
    if val.is_nan() {
        "NaN".to_string()
    } else if val.is_infinite() {
        if val > 0.0 {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        }
    } else {
        let abs = val.abs();
        if abs >= SCI_NOTATION_UPPER_THRESHOLD || (abs < SCI_NOTATION_LOWER_THRESHOLD && abs > 0.0) {
            format!("{:.prec$e}", val, prec = FLOAT_DISPLAY_PRECISION)
        } else {
            let s = format!("{:.prec$}", val, prec = FLOAT_DISPLAY_PRECISION);
            if s.contains('.') {
                let trimmed = s.trim_end_matches('0').trim_end_matches('.');
                trimmed.to_string()
            } else {
                s
            }
        }
    }
}
