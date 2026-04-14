use crate::types::NumberFormat;

pub fn format_number(value: f64, fmt: &NumberFormat) -> String {
    match fmt {
        NumberFormat::Accounting { decimals } => {
            let (_, num) = format_accounting_parts(value, *decimals);
            num
        }
        NumberFormat::Currency { symbol, decimals } => format_currency(value, symbol, *decimals),
        NumberFormat::Percent { decimals } => format_percent(value, *decimals),
        NumberFormat::Decimal { decimals } => format_decimal(value, *decimals),
        NumberFormat::Integer => format_integer(value),
        NumberFormat::Date { format } => format_date(value, format),
        NumberFormat::DateTime { format } => format_date(value, format),
    }
}

/// For accounting format, returns (symbol, number_string) to draw separately.
/// symbol is always "$", number_string is e.g. "1,234.56" or "(1,234.56)".
pub fn format_accounting_parts(value: f64, decimals: u32) -> (&'static str, String) {
    let num = format_with_commas(value.abs(), decimals);
    let number_str = if value < 0.0 {
        format!("({})", num)
    } else {
        num
    };
    ("$", number_str)
}

pub fn is_accounting(fmt: &NumberFormat) -> Option<u32> {
    match fmt {
        NumberFormat::Accounting { decimals } => Some(*decimals),
        _ => None,
    }
}

fn format_currency(value: f64, symbol: &str, decimals: u32) -> String {
    let abs = value.abs();
    let formatted = format_with_commas(abs, decimals);
    if value < 0.0 {
        format!("-{}{}", symbol, formatted)
    } else {
        format!("{}{}", symbol, formatted)
    }
}

fn format_percent(value: f64, decimals: u32) -> String {
    let pct = value * 100.0;
    format!("{:.prec$}%", pct, prec = decimals as usize)
}

fn format_decimal(value: f64, decimals: u32) -> String {
    format_with_commas(value, decimals)
}

fn format_integer(value: f64) -> String {
    let rounded = value.round() as i64;
    format_int_with_commas(rounded)
}

fn format_date(value: f64, fmt: &str) -> String {
    // Detect the scale of the value to handle Arrow's different date/time units:
    //   Date32 stores days since epoch      (values ~0–100,000)
    //   Date64 stores milliseconds          (values ~0–1e12)
    //   Timestamp (us) stores microseconds  (values ~0–1e15)
    //   Timestamp (ns) stores nanoseconds   (values ~0–1e18)
    //   Unix seconds                        (values ~0–2e9)
    let secs = if value.abs() < 1e8 {
        // Likely days (Date32) → convert to seconds
        value as i64 * 86_400
    } else if value.abs() < 1e11 {
        // Likely Unix seconds — use as-is
        value as i64
    } else if value.abs() < 1e14 {
        // Likely milliseconds (Date64) → convert to seconds
        (value / 1_000.0) as i64
    } else if value.abs() < 1e17 {
        // Likely microseconds → convert to seconds
        (value / 1_000_000.0) as i64
    } else {
        // Likely nanoseconds → convert to seconds
        (value / 1_000_000_000.0) as i64
    };
    match chrono::DateTime::from_timestamp(secs, 0) {
        Some(dt) => dt.format(fmt).to_string(),
        None => value.to_string(),
    }
}

fn format_with_commas(value: f64, decimals: u32) -> String {
    let formatted = format!("{:.prec$}", value.abs(), prec = decimals as usize);
    let parts: Vec<&str> = formatted.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts.get(1);

    let with_commas = add_commas(int_part);
    let sign = if value < 0.0 { "-" } else { "" };

    match dec_part {
        Some(d) if decimals > 0 => format!("{}{}.{}", sign, with_commas, d),
        _ => format!("{}{}", sign, with_commas),
    }
}

fn format_int_with_commas(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let s = value.abs().to_string();
    format!("{}{}", sign, add_commas(&s))
}

fn add_commas(s: &str) -> String {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len <= 3 {
        return s.to_string();
    }
    let mut result = String::with_capacity(len + len / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}
