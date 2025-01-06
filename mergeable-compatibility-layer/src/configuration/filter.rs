use serde::{Deserialize, Serialize};

use crate::configuration::basics::{BooleanClause, ChainedAndOrIncludeExcludeClause};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "do")]
#[serde(deny_unknown_fields)]
pub enum Filter {
    /// supported events 'pull_request.*', 'pull_request_review.*'
    #[serde(rename = "author")]
    Author(Box<TheAuthor>),
    /// supported events 'pull_request.*', 'pull_request_review.*'
    #[serde(rename = "repository")]
    Repository(Box<TheRepository>),
    /// supported events 'pull_request.*', 'pull_request_review.*', issues.*'
    #[serde(rename = "payload")]
    Payload(ThePayload),
    #[serde(rename = "and")]
    And(FilterAnd),
    #[serde(rename = "or")]
    Or(FilterOr),
    #[serde(rename = "not")]
    Not(FilterNot),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilterAnd(FilterStack);

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilterOr(FilterStack);

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilterNot(FilterStack);

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FilterStack {
    filter: Vec<Filter>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TheAuthor {
    #[serde(flatten)]
    filter: ChainedAndOrIncludeExcludeClause,
    #[serde(rename = "team")]
    #[serde(skip_serializing_if = "Option::is_none")]
    team: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TheRepository {
    #[serde(rename = "visibility")]
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<String>,
    #[serde(rename = "name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<ChainedAndOrIncludeExcludeClause>,
    #[serde(rename = "topics")]
    #[serde(skip_serializing_if = "Option::is_none")]
    topics: Option<ChainedAndOrIncludeExcludeClause>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThePayload {
    // TODO proper handling
    #[serde(skip_serializing_if = "Option::is_none")]
    pull_request: Option<serde_yaml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<serde_yaml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sender: Option<serde_yaml::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum X {
    #[serde(rename = "boolean")]
    Boolean(BooleanClause),
}
