//! Import and export of plans. Export writes a portable JSON spec; import reads that JSON, a
//! bare JSON array of items, or a CSV with a header row.
//!
//! CSV columns are matched by name, case-insensitive:
//! - name: `slug`, `id`, `name` or `file` (file extensions are dropped)
//! - title: `title` or `label`
//! - description: `subject`, `prompt`, `description` or `text` (required)

use crate::error::{msg, AppResult};
use crate::store::Project;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpecItem {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(alias = "prompt", alias = "description")]
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub format: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub idea: String,
    #[serde(default)]
    pub style_brief: String,
    #[serde(default)]
    pub shared_tail: String,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub gate: Option<String>,
    pub items: Vec<SpecItem>,
}

pub const FORMAT: &str = "eitri-spec/1";

pub fn export(project: &Project, items: Vec<SpecItem>) -> Spec {
    Spec {
        format: FORMAT.into(),
        name: project.name.clone(),
        idea: project.idea.clone(),
        style_brief: project.style_brief.clone(),
        shared_tail: project.shared_tail.clone(),
        size: Some(project.size.clone()),
        background: Some(project.background.clone()),
        gate: Some(project.gate.clone()),
        items,
    }
}

pub fn parse(text: &str, file_name: &str) -> AppResult<Spec> {
    let t = text.trim_start_matches('\u{feff}').trim();
    if t.starts_with('{') {
        let spec: Spec =
            serde_json::from_str(t).map_err(|e| msg(format!("Not a valid spec file: {e}")))?;
        return check(spec);
    }
    if t.starts_with('[') {
        let items: Vec<SpecItem> =
            serde_json::from_str(t).map_err(|e| msg(format!("Not a valid list of images: {e}")))?;
        return check(bare(items));
    }
    if file_name.to_lowercase().ends_with(".csv") || t.contains(',') {
        return check(bare(parse_csv(t)?));
    }
    Err(msg("Import a .json spec or a .csv with a header row."))
}

fn bare(items: Vec<SpecItem>) -> Spec {
    Spec {
        format: FORMAT.into(),
        name: String::new(),
        idea: String::new(),
        style_brief: String::new(),
        shared_tail: String::new(),
        size: None,
        background: None,
        gate: None,
        items,
    }
}

fn check(mut spec: Spec) -> AppResult<Spec> {
    spec.items.retain(|i| !i.subject.trim().is_empty());
    if spec.items.is_empty() {
        return Err(msg("The file has no image descriptions."));
    }
    for (n, it) in spec.items.iter_mut().enumerate() {
        if it.slug.trim().is_empty() {
            it.slug = if it.title.trim().is_empty() {
                format!("image-{}", n + 1)
            } else {
                it.title.clone()
            };
        }
        if let Some(stem) = it
            .slug
            .rsplit_once('.')
            .filter(|(_, ext)| ext.len() <= 4)
            .map(|(s, _)| s.to_string())
        {
            it.slug = stem;
        }
    }
    Ok(spec)
}

/// RFC 4180 CSV: quoted fields, doubled quotes, newlines inside quotes.
pub fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' => {}
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    rows
}

fn parse_csv(text: &str) -> AppResult<Vec<SpecItem>> {
    let rows = csv_rows(text);
    let (header, body) = rows.split_first().ok_or_else(|| msg("The CSV is empty."))?;
    let find = |names: &[&str]| {
        header
            .iter()
            .position(|h| names.contains(&h.trim().to_lowercase().as_str()))
    };
    let subject = find(&["subject", "prompt", "description", "text"])
        .ok_or_else(|| msg("The CSV needs a column named subject, prompt or description."))?;
    let slug = find(&["slug", "id", "name", "file"]);
    let title = find(&["title", "label"]);
    let get =
        |r: &Vec<String>, i: Option<usize>| i.and_then(|i| r.get(i)).cloned().unwrap_or_default();
    Ok(body
        .iter()
        .map(|r| SpecItem {
            slug: get(r, slug),
            title: get(r, title),
            subject: get(r, Some(subject)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_with_quotes_and_newlines() {
        let csv = "id,file,prompt\nJPN-m-001,JPN-m-001.png,\"A man, 29.\nBuild: \"\"sturdy\"\"\"\nJPN-f-001,JPN-f-001.png,A woman\n";
        let spec = parse(csv, "portraits.csv").unwrap();
        assert_eq!(spec.items.len(), 2);
        assert_eq!(spec.items[0].slug, "JPN-m-001");
        assert_eq!(spec.items[0].subject, "A man, 29.\nBuild: \"sturdy\"");
    }

    #[test]
    fn json_spec_and_bare_array() {
        let s = parse(r#"{"format":"eitri-spec/1","shared_tail":"flat","items":[{"slug":"a","subject":"x"}]}"#, "a.json").unwrap();
        assert_eq!(s.shared_tail, "flat");
        let b = parse(
            r#"[{"title":"Red potion","prompt":"a red potion"},{"subject":""}]"#,
            "b.json",
        )
        .unwrap();
        assert_eq!(b.items.len(), 1);
        assert_eq!(b.items[0].slug, "Red potion");
    }

    #[test]
    fn csv_needs_a_description_column() {
        assert!(parse("a,b\n1,2\n", "x.csv").is_err());
    }
}
