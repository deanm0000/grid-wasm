use crate::types::{ColumnInput, DataStyle, HeaderStyle};

#[derive(Debug, Clone)]
pub struct LeafColumn {
    pub display_index: usize,
    pub arrow_index: usize,
    pub arrow_name: String,
    pub display_name: String,
    pub width: f64,
    pub is_resizable: bool,
    pub header_style: Option<HeaderStyle>,
    pub data_style: Option<DataStyle>,
    pub parent_titles: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HeaderSpan {
    pub title: String,
    pub first_leaf: usize,
    pub last_leaf: usize,
    pub style: Option<HeaderStyle>,
}

#[derive(Debug, Clone)]
pub struct ResolvedColumns {
    pub leaves: Vec<LeafColumn>,
    pub header_levels: Vec<Vec<HeaderSpan>>,
    pub max_depth: usize,
}

impl ResolvedColumns {
    pub fn leaf_by_arrow_index(&self, arrow_idx: usize) -> Option<&LeafColumn> {
        self.leaves.iter().find(|l| l.arrow_index == arrow_idx)
    }

    pub fn leaf_by_display_index(&self, disp_idx: usize) -> Option<&LeafColumn> {
        self.leaves.get(disp_idx)
    }

    pub fn swap_leaves(&mut self, a: usize, b: usize) {
        if a >= self.leaves.len() || b >= self.leaves.len() || a == b {
            return;
        }
        self.leaves.swap(a, b);
        self.leaves[a].display_index = a;
        self.leaves[b].display_index = b;
        self.recompute_header_levels();
    }

    pub fn recompute_header_levels(&mut self) {
        if self.max_depth <= 1 {
            self.header_levels.clear();
            return;
        }
        let num_levels = self.max_depth - 1;
        let mut levels: Vec<Vec<HeaderSpan>> = vec![Vec::new(); num_levels];

        for level in 0..num_levels {
            let mut spans: Vec<HeaderSpan> = Vec::new();
            for (i, leaf) in self.leaves.iter().enumerate() {
                let title = leaf.parent_titles.get(level).cloned().unwrap_or_default();
                let can_merge = spans.last().map_or(false, |s: &HeaderSpan| s.title == title);
                if can_merge {
                    spans.last_mut().unwrap().last_leaf = i;
                } else {
                    spans.push(HeaderSpan {
                        title,
                        first_leaf: i,
                        last_leaf: i,
                        style: leaf.header_style.clone(),
                    });
                }
            }
            levels[level] = spans;
        }
        self.header_levels = levels;
    }
}

pub fn normalize_columns(
    columns: Option<&[ColumnInput]>,
    overrides: Option<&[ColumnInput]>,
    data_columns: &[String],
) -> Result<Vec<ColumnInput>, String> {
    if columns.is_some() && overrides.is_some() {
        return Err("Cannot specify both 'columns' and 'columnOverrides'".to_string());
    }

    if let Some(cols) = columns {
        return Ok(cols.to_vec());
    }

    if let Some(ovrs) = overrides {
        let override_map: std::collections::HashMap<&str, &ColumnInput> = ovrs
            .iter()
            .filter_map(|c| c.name.as_deref().map(|n| (n, c)))
            .collect();

        let result: Vec<ColumnInput> = data_columns
            .iter()
            .map(|name| {
                if let Some(ovr) = override_map.get(name.as_str()) {
                    (*ovr).clone()
                } else {
                    ColumnInput {
                        name: Some(name.clone()),
                        display: Some(name.clone()),
                        init_width: None,
                        is_resizable: true,
                        header_style: None,
                        data_style: None,
                        children: None,
                        agg_func: None,
                        group_by: None,
                        group_by_truncation: None,
                    }
                }
            })
            .collect();
        return Ok(result);
    }

    Ok(data_columns
        .iter()
        .map(|name| ColumnInput {
            name: Some(name.clone()),
            display: Some(name.clone()),
            init_width: None,
            is_resizable: true,
            header_style: None,
            data_style: None,
            children: None,
        agg_func: None,
        group_by: None,
        group_by_truncation: None,
        })
        .collect())
}

fn measure_depth(input: &ColumnInput) -> usize {
    match &input.children {
        Some(children) if !children.is_empty() => {
            1 + children.iter().map(|c| measure_depth(c)).max().unwrap_or(0)
        }
        _ => 1,
    }
}

fn validate_uniform_depth(inputs: &[ColumnInput]) -> Result<usize, String> {
    if inputs.is_empty() {
        return Ok(1);
    }

    let depths: Vec<usize> = inputs.iter().map(|c| measure_depth(c)).collect();
    let first = depths[0];
    for (i, &d) in depths.iter().enumerate() {
        if d != first {
            let name = inputs[i]
                .display
                .as_deref()
                .or(inputs[i].name.as_deref())
                .unwrap_or("(unnamed)");
            return Err(format!(
                "Column '{}' has depth {} but expected {}. All columns must have the same nesting depth.",
                name, d, first
            ));
        }
    }
    Ok(first)
}

pub fn resolve_columns(
    inputs: &[ColumnInput],
    data_columns: &[String],
    default_width: f64,
    border_width: f64,
) -> Result<ResolvedColumns, String> {
    let depth = validate_uniform_depth(inputs)?;

    let col_index_map: std::collections::HashMap<&str, usize> = data_columns
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    let mut leaves = Vec::new();
    let mut header_levels: Vec<Vec<HeaderSpan>> = vec![Vec::new(); depth.saturating_sub(1)];

    collect_leaves(
        inputs,
        &col_index_map,
        default_width,
        border_width,
        0,
        depth,
        &mut leaves,
        &mut header_levels,
        &Vec::new(),
    )?;

    Ok(ResolvedColumns {
        leaves,
        header_levels,
        max_depth: depth,
    })
}

fn collect_leaves(
    inputs: &[ColumnInput],
    col_index_map: &std::collections::HashMap<&str, usize>,
    default_width: f64,
    border_width: f64,
    level: usize,
    max_depth: usize,
    leaves: &mut Vec<LeafColumn>,
    header_levels: &mut Vec<Vec<HeaderSpan>>,
    ancestor_titles: &Vec<String>,
) -> Result<(), String> {
    for input in inputs {
        match &input.children {
            Some(children) if !children.is_empty() => {
                let first_leaf = leaves.len();

                let parent_title = input
                    .display
                    .clone()
                    .or_else(|| input.name.clone())
                    .unwrap_or_default();

                let mut child_ancestors = ancestor_titles.clone();
                child_ancestors.push(parent_title);

                let parent_width = input.init_width;
                let children_have_widths = children.iter().all(|c| c.init_width.is_some());

                let child_default = if !children_have_widths {
                    if let Some(pw) = parent_width {
                        let n = children.len() as f64;
                        let total_borders = (n - 1.0) * border_width;
                        (pw - total_borders) / n
                    } else {
                        default_width
                    }
                } else {
                    default_width
                };

                collect_leaves(
                    children,
                    col_index_map,
                    child_default,
                    border_width,
                    level + 1,
                    max_depth,
                    leaves,
                    header_levels,
                    &child_ancestors,
                )?;

                let last_leaf = leaves.len().saturating_sub(1);

                if level < header_levels.len() {
                    header_levels[level].push(HeaderSpan {
                        title: input
                            .display
                            .clone()
                            .or_else(|| input.name.clone())
                            .unwrap_or_default(),
                        first_leaf,
                        last_leaf,
                        style: input.header_style.clone(),
                    });
                }
            }
            _ => {
                let name = input.name.clone().unwrap_or_default();
                if let Some(&arrow_idx) = col_index_map.get(name.as_str()) {
                    let display_index = leaves.len();
                    leaves.push(LeafColumn {
                        display_index,
                        arrow_index: arrow_idx,
                        arrow_name: name.clone(),
                        display_name: input
                            .display
                            .clone()
                            .unwrap_or_else(|| name.clone()),
                        width: input.init_width.unwrap_or(default_width),
                        is_resizable: input.is_resizable,
                        header_style: input.header_style.clone(),
                        data_style: input.data_style.clone(),
                        parent_titles: ancestor_titles.clone(),
                    });

                    for lvl in level..header_levels.len() {
                        header_levels[lvl].push(HeaderSpan {
                            title: String::new(),
                            first_leaf: display_index,
                            last_leaf: display_index,
                            style: None,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}
