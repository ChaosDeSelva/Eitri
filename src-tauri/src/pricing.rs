//! Cost per call, from the usage block the API returns. Prices are USD per million tokens
//! and are built in. They only drive spend reporting; OpenAI's own dashboard is the bill of record.

use crate::openai::Usage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Price {
    pub text_in: f64,
    pub image_in: f64,
    pub output: f64,
}

pub type PriceTable = BTreeMap<String, Price>;

pub fn defaults() -> PriceTable {
    let mut t = PriceTable::new();
    let image = Price {
        text_in: 5.0,
        image_in: 10.0,
        output: 40.0,
    };
    t.insert("gpt-image-1".into(), image.clone());
    t.insert("gpt-image-1.5".into(), image.clone());
    t.insert(
        "gpt-image-1-mini".into(),
        Price {
            text_in: 2.0,
            image_in: 2.5,
            output: 8.0,
        },
    );
    t.insert(
        "gpt-4.1-mini".into(),
        Price {
            text_in: 0.4,
            image_in: 0.4,
            output: 1.6,
        },
    );
    t.insert(
        "gpt-4o-mini".into(),
        Price {
            text_in: 0.15,
            image_in: 0.15,
            output: 0.6,
        },
    );
    t.insert(
        "gpt-5-mini".into(),
        Price {
            text_in: 0.25,
            image_in: 0.25,
            output: 2.0,
        },
    );
    t.insert(
        "gemini-2.5-flash-image".into(),
        Price {
            text_in: 0.3,
            image_in: 0.3,
            output: 30.0,
        },
    );
    t.insert(
        "gemini-2.5-flash".into(),
        Price {
            text_in: 0.3,
            image_in: 0.3,
            output: 2.5,
        },
    );
    t.insert(
        "gemini-2.5-flash-lite".into(),
        Price {
            text_in: 0.1,
            image_in: 0.1,
            output: 0.4,
        },
    );
    t.insert(
        "gemini-2.5-pro".into(),
        Price {
            text_in: 1.25,
            image_in: 1.25,
            output: 10.0,
        },
    );
    t.insert("*".into(), image);
    t
}

pub fn cost(table: &PriceTable, model: &str, u: &Usage) -> f64 {
    let p = table
        .get(model)
        .or_else(|| {
            table
                .iter()
                .filter(|(k, _)| model.starts_with(k.as_str()))
                .max_by_key(|(k, _)| k.len())
                .map(|(_, v)| v)
        })
        .or_else(|| table.get("*"))
        .cloned()
        .unwrap_or(Price {
            text_in: 5.0,
            image_in: 10.0,
            output: 40.0,
        });
    (p.text_in * u.input_text as f64
        + p.image_in * u.input_image as f64
        + p.output * u.output as f64)
        / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_image_input_separately() {
        let t = defaults();
        let u = Usage {
            input_text: 500,
            input_image: 1000,
            output: 1056,
        };
        let c = cost(&t, "gpt-image-1", &u);
        assert!((c - (0.0025 + 0.01 + 0.04224)).abs() < 1e-9);
    }

    #[test]
    fn falls_back_to_prefix_then_wildcard() {
        let t = defaults();
        let u = Usage {
            input_text: 0,
            input_image: 0,
            output: 1_000_000,
        };
        assert_eq!(cost(&t, "gpt-image-1-2026-01-01", &u), 40.0);
        assert_eq!(cost(&t, "some-future-model", &u), 40.0);
    }
}
