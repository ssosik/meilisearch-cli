use crate::document;
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr; // Provides `width()` method on String

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ApiQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "q")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub sort: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "facetsDistribution")]
    pub facets_distribution: Option<Vec<String>>,
    #[serde(default)]
    pub limit: u32,
}

use pest::Parser; // Provides the generated 'parse()' method on Filter struct
use pest_derive::Parser; // Provides the Parser deriver, grammer autogeneration, and Rules

#[derive(Parser)]
#[grammar = "filter.pest"]
pub struct Filter;

impl ApiQuery {
    pub fn new() -> Self {
        ApiQuery {
            sort: Some(vec!["date:desc".to_owned()]),
            limit: 10000,
            ..Default::default()
        }
    }

    pub fn process_filter(&mut self, s: String) {
        // If the supplied string doesn't parse with our expected grammer, just return
        let mut expr = match Filter::parse(Rule::expression, s.as_str()) {
            Ok(f) => f,
            Err(_) => return,
        };
        let expr = expr.next().unwrap();
        // String to set on self.filter
        let mut filter = String::from("");
        // Iterate over each inner piece of the parsed expression and build the
        // filter string to set on the meilisearch query
        for t in expr.into_inner() {
            // TODO add support for subexpressions in parens
            // TODO add support for single-quoted tags to enable tags with spaces
            // TODO add support for dates, like:
            //  - 2019 : match all docs within date in the year
            //  - 2019-10 : match all docs within date in the year and month
            //  - 2019-10-30 : match all docs within date in the year, month and dat
            //  - 1h : match all docs within the past hour
            //  - 2d : match all docs within the 2 days
            //  - 3w : match all docs within the 3 weeks
            //  - 4m : match all docs within the 4 months
            //  - 5y : match all docs within the 5 years
            //  For all of the above, add '<' and '>' prefixed variants for
            //    older than and newer than constraints
            match t.as_rule() {
                Rule::tag => {
                    filter.push_str("tag=");
                    filter.push_str(t.as_str());
                }
                Rule::not_tag => {
                    filter.push_str("tag!=");
                    for i in t.into_inner() {
                        filter.push_str(i.as_str());
                    }
                }
                Rule::operator => match t.into_inner().next().unwrap().as_rule() {
                    Rule::and => {
                        filter.push_str(" AND ");
                    }
                    Rule::or => {
                        filter.push_str(" OR ");
                    }
                    _ => unreachable!(),
                },
                Rule::EOI => break,
                _ => unreachable!(),
            }
        }
        if filter.width() > 0 {
            self.filter = Some(filter);
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ApiResponse {
    pub hits: Vec<document::Document>,
    #[serde(rename = "nbHits")]
    pub num_hits: u32,
    #[serde(rename = "exhaustiveNbHits")]
    pub exhaustive_num_hits: bool,
    pub query: String,
    pub limit: u16,
    pub offset: u32,
    #[serde(rename = "processingTimeMs")]
    pub processing_time_ms: u32,
}
