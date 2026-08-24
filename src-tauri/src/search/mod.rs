use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    File,
    Folder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Comparison {
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Equal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SizeFilter {
    pub comparison: Comparison,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub terms: Vec<String>,
    pub extensions: Vec<String>,
    pub paths: Vec<String>,
    pub sizes: Vec<SizeFilter>,
    pub entry_type: Option<EntryType>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SearchParseError {
    #[error("Unvollständiges Anführungszeichen")]
    UnclosedQuote,
    #[error("Ungültiger Grössenfilter: {0}")]
    InvalidSize(String),
    #[error("Unbekannter Typ: {0}")]
    InvalidType(String),
    #[error("Leerer Filter: {0}")]
    EmptyFilter(String),
}

impl SearchQuery {
    pub fn parse(input: &str) -> Result<Self, SearchParseError> {
        let mut parsed = Self::default();
        for token in tokenize(input)? {
            let Some((key, value)) = token.split_once(':') else {
                if !token.is_empty() {
                    parsed.terms.push(token);
                }
                continue;
            };
            let key = key.to_ascii_lowercase();
            if value.trim().is_empty() {
                return Err(SearchParseError::EmptyFilter(key));
            }
            match key.as_str() {
                "ext" => parsed
                    .extensions
                    .push(value.trim_start_matches('.').to_ascii_lowercase()),
                "path" => parsed.paths.push(value.to_string()),
                "size" => parsed.sizes.push(parse_size_filter(value)?),
                "type" => {
                    parsed.entry_type = Some(match value.to_ascii_lowercase().as_str() {
                        "file" | "datei" => EntryType::File,
                        "folder" | "dir" | "ordner" => EntryType::Folder,
                        _ => return Err(SearchParseError::InvalidType(value.to_string())),
                    })
                }
                _ => parsed.terms.push(token),
            }
        }
        Ok(parsed)
    }

    pub fn to_sql(&self) -> (String, String, Vec<Value>) {
        let uses_fts = !self.terms.is_empty() || !self.paths.is_empty();
        let from = if uses_fts {
            "FROM entries e JOIN entries_fts ON entries_fts.rowid=e.id".to_string()
        } else {
            "FROM entries e".to_string()
        };
        let mut clauses = Vec::new();
        let mut values = Vec::new();
        if uses_fts {
            let mut expressions = Vec::new();
            for term in &self.terms {
                let value = fts_value(term);
                expressions.push(format!(
                    "(name:{value} OR full_path:{value} OR extension:{value})"
                ));
            }
            for path in &self.paths {
                expressions.push(format!("full_path:{}", fts_value(path)));
            }
            clauses.push("entries_fts MATCH ?".to_string());
            values.push(Value::Text(expressions.join(" AND ")));
        }
        for extension in &self.extensions {
            clauses.push("e.extension = ? COLLATE NOCASE".to_string());
            values.push(Value::Text(extension.clone()));
        }
        for size in &self.sizes {
            let operator = match size.comparison {
                Comparison::Greater => ">",
                Comparison::GreaterOrEqual => ">=",
                Comparison::Less => "<",
                Comparison::LessOrEqual => "<=",
                Comparison::Equal => "=",
            };
            clauses.push(format!(
                "CASE WHEN e.is_directory=1 THEN e.recursive_size ELSE e.size END {operator} ?"
            ));
            values.push(Value::Integer(size.bytes.min(i64::MAX as u64) as i64));
        }
        if let Some(entry_type) = &self.entry_type {
            clauses.push(format!(
                "e.is_directory={}",
                matches!(entry_type, EntryType::Folder) as u8
            ));
        }
        if clauses.is_empty() {
            clauses.push("1=1".to_string());
        }
        (from, clauses.join(" AND "), values)
    }
}

pub fn parse_bytes(value: &str) -> Result<u64, SearchParseError> {
    let normalized = value.trim().to_ascii_lowercase();
    let split = normalized
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != ',')
        .unwrap_or(normalized.len());
    let number = normalized[..split].replace(',', ".");
    let unit = normalized[split..].trim();
    if number.is_empty() {
        return Err(SearchParseError::InvalidSize(value.to_string()));
    }
    let numeric: f64 = number
        .parse()
        .map_err(|_| SearchParseError::InvalidSize(value.to_string()))?;
    if !numeric.is_finite() || numeric < 0.0 {
        return Err(SearchParseError::InvalidSize(value.to_string()));
    }
    let multiplier = match unit {
        "" | "b" => 1_f64,
        "kb" => 1024_f64,
        "mb" => 1024_f64.powi(2),
        "gb" => 1024_f64.powi(3),
        "tb" => 1024_f64.powi(4),
        _ => return Err(SearchParseError::InvalidSize(value.to_string())),
    };
    let result = numeric * multiplier;
    if result > u64::MAX as f64 {
        return Err(SearchParseError::InvalidSize(value.to_string()));
    }
    Ok(result.round() as u64)
}

fn parse_size_filter(value: &str) -> Result<SizeFilter, SearchParseError> {
    let (comparison, number) = if let Some(rest) = value.strip_prefix(">=") {
        (Comparison::GreaterOrEqual, rest)
    } else if let Some(rest) = value.strip_prefix("<=") {
        (Comparison::LessOrEqual, rest)
    } else if let Some(rest) = value.strip_prefix('>') {
        (Comparison::Greater, rest)
    } else if let Some(rest) = value.strip_prefix('<') {
        (Comparison::Less, rest)
    } else if let Some(rest) = value.strip_prefix('=') {
        (Comparison::Equal, rest)
    } else {
        (Comparison::Equal, value)
    };
    Ok(SizeFilter {
        comparison,
        bytes: parse_bytes(number)?,
    })
}

fn tokenize(input: &str) -> Result<Vec<String>, SearchParseError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in input.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if quoted {
        return Err(SearchParseError::UnclosedQuote);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

fn fts_value(value: &str) -> String {
    let cleaned = value.replace(['"', '(', ')', ':', '*'], " ");
    cleaned
        .split_whitespace()
        .map(|part| format!("\"{}\"*", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combined_query() {
        let query =
            SearchQuery::parse("blender ext:.blend size:>500mb type:file path:\"Meine Projekte\"")
                .unwrap();
        assert_eq!(query.terms, ["blender"]);
        assert_eq!(query.extensions, ["blend"]);
        assert_eq!(query.paths, ["Meine Projekte"]);
        assert_eq!(query.entry_type, Some(EntryType::File));
        assert_eq!(query.sizes[0].bytes, 500 * 1024 * 1024);
    }

    #[test]
    fn parses_all_units_and_decimal() {
        assert_eq!(parse_bytes("1b").unwrap(), 1);
        assert_eq!(parse_bytes("2kb").unwrap(), 2048);
        assert_eq!(parse_bytes("1.5mb").unwrap(), 1_572_864);
        assert_eq!(parse_bytes("1gb").unwrap(), 1_073_741_824);
        assert_eq!(parse_bytes("1tb").unwrap(), 1_099_511_627_776);
    }

    #[test]
    fn rejects_invalid_filters() {
        assert!(SearchQuery::parse("size:huge").is_err());
        assert!(SearchQuery::parse("type:banana").is_err());
        assert!(SearchQuery::parse("path:\"unfinished").is_err());
    }
}
