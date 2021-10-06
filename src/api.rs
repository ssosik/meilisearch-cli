use crate::date::DateRange;
use crate::document;
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use color_eyre::Report;
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::convert::TryInto;
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

// Provides the generated 'parse()' method on Filter struct
use pest::{iterators as pest_iterators, Parser};
// Provides the Parser deriver, grammer autogeneration, and Rules
use pest_derive::Parser;

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

    pub fn process_filter(&mut self, input: String) {
        // If the supplied string doesn't parse with our expected grammer, just return
        let mut expr = match Filter::parse(Rule::expression, input.as_str()) {
            Ok(f) => f,
            Err(_) => return,
        };
        let expr = expr.next().unwrap();
        // String to set on self.filter
        let mut filter = String::from("");
        // Iterate over each inner piece of the parsed expression and build the
        // filter string to set on the meilisearch query
        let mut curr_comparator: Option<Rule> = None;
        for token in expr.into_inner() {
            // TODO add support for subexpressions in parens
            // TODO add support for single-quoted tags to enable tags with spaces
            match token.as_rule() {
                Rule::comparator => match token.into_inner().next().unwrap().as_rule() {
                    Rule::gt => curr_comparator = Some(Rule::gt),
                    Rule::lt => curr_comparator = Some(Rule::lt),
                    _ => unreachable!(),
                },
                Rule::date => {
                    filter.push_str("date ");
                    let range: DateRange = match token.into_inner().next() {
                        Some(r) => match r.try_into() {
                            Ok(r) => r,
                            Err(e) => {
                                filter.push_str(&format!("Date err {:?}", e));
                                continue;
                            }
                        },
                        None => {
                            filter.push_str("none");
                            continue;
                        }
                    };
                    match curr_comparator {
                        Some(c) => match c {
                            Rule::gt => filter.push_str(&format!("> {} ", range.start.timestamp())),
                            Rule::lt => filter.push_str(&format!("< {} ", range.end.timestamp())),
                            _ => unreachable!(),
                        },
                        None => filter.push_str(&format!(
                            "> {} AND date < {}",
                            range.start.timestamp(),
                            range.end.timestamp()
                        )),
                    };
                    curr_comparator = None; // Reset comparator
                }
                Rule::duration => {
                    filter.push_str("date ");
                    let t = token.into_inner().next().unwrap();
                    let dur_fn = match t.as_rule() {
                        Rule::hour_duration => |n| Duration::hours(n),
                        Rule::day_duration => |n| Duration::days(n),
                        Rule::week_duration => |n| Duration::weeks(n),
                        Rule::month_duration => |n| Duration::days(n * 30),
                        Rule::year_duration => |n| Duration::days(n * 365),
                        _ => unreachable!(),
                    };
                    let v = t
                        .into_inner()
                        .next()
                        .unwrap()
                        .as_str()
                        .parse::<i64>()
                        .unwrap();
                    let ts = Local::now().checked_sub_signed(dur_fn(v)).unwrap();
                    match curr_comparator {
                        Some(c) => match c {
                            Rule::gt => filter.push_str(&format!("> {} ", ts.timestamp())),
                            Rule::lt => filter.push_str(&format!("< {} ", ts.timestamp())),
                            _ => unreachable!(),
                        },
                        None => filter.push_str(&format!("> {}", ts.timestamp())),
                    };
                    curr_comparator = None; // Reset comparator
                }
                Rule::tag => {
                    filter.push_str("tags = ");
                    filter.push_str(token.as_str());
                }
                Rule::not_tag => {
                    filter.push_str("tags != ");
                    for inner in token.into_inner() {
                        filter.push_str(inner.as_str());
                    }
                }
                Rule::operator => match token.into_inner().next().unwrap().as_rule() {
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

type PestPair<'a> = pest_iterators::Pair<'a, Rule>;
impl TryFrom<PestPair<'_>> for DateRange {
    type Error = Report;

    fn try_from(item: PestPair<'_>) -> Result<Self, Self::Error> {
        let (start, end) = match item.as_rule() {
            Rule::year_month_day => {
                let mut item = item.into_inner();
                let y = item.next().unwrap().as_str().parse::<i32>().unwrap();
                let m = item.next().unwrap().as_str().parse::<u32>().unwrap();
                let d = item.next().unwrap().as_str().parse::<u32>().unwrap();
                (
                    // Start date
                    DateTime::<Utc>::from_utc(NaiveDate::from_ymd(y, m, d).and_hms(0, 0, 0), Utc),
                    // End date
                    DateTime::<Utc>::from_utc(
                        NaiveDate::from_ymd(y, m, d).and_hms(23, 59, 59),
                        Utc,
                    ),
                )
            }
            Rule::year_month => {
                let mut item = item.into_inner();
                let y = item.next().unwrap().as_str().parse::<i32>().unwrap();
                let m = item.next().unwrap().as_str().parse::<u32>().unwrap();
                (
                    // Start date
                    DateTime::<Utc>::from_utc(NaiveDate::from_ymd(y, m, 1).and_hms(0, 0, 0), Utc),
                    // End date
                    DateTime::<Utc>::from_utc(
                        match m {
                            12 => NaiveDate::from_ymd(y + 1, 1, 1),
                            _ => NaiveDate::from_ymd(y, m + 1, 1),
                        }
                        .pred()
                        .and_hms(23, 59, 59),
                        Utc,
                    ),
                )
            }
            Rule::year => {
                let y = item.as_str().parse::<i32>().unwrap();
                (
                    // Start date
                    DateTime::<Utc>::from_utc(NaiveDate::from_ymd(y, 1, 1).and_hms(0, 0, 0), Utc),
                    // End date
                    DateTime::<Utc>::from_utc(
                        NaiveDate::from_ymd(y, 12, 31).and_hms(23, 59, 59),
                        Utc,
                    ),
                )
            }
            e => return Err(eyre!("Unexpected match item {:?}", e)),
        };
        Ok(DateRange { start, end })
    }
}
