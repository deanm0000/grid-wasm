use arrow_array::Array;
use arrow_ipc::reader::StreamReader;
use arrow_schema::{DataType, Field, Schema};
use std::io::Cursor;
use std::sync::Arc;

use crate::types::{ContentAlign, GridCell, GridColumn};

pub const ROW_ID_COL: &str = "__row_id__";

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

    pub fn num_rows(&self) -> usize {
        self.row_count
    }

    pub fn num_columns(&self) -> usize {
        self.column_count
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
                    | DataType::Float64 => ("number".to_string(), 0.8),
                    DataType::Boolean => ("boolean".to_string(), 0.5),
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
        DataType::Timestamp(_, _) => {
            use arrow_array::types::*;
            let val = as_primitive_array::<TimestampMicrosecondType>(array).value(row);
            GridCell::Text {
                data: val.to_string(),
                display_data: None,
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
        if abs >= 1e9 || (abs < 0.001 && abs > 0.0) {
            format!("{:.6e}", val)
        } else {
            let s = format!("{:.6}", val);
            if s.contains('.') {
                let trimmed = s.trim_end_matches('0').trim_end_matches('.');
                trimmed.to_string()
            } else {
                s
            }
        }
    }
}
