use serde::{Deserialize, Serialize};

/// Non-novel source: short drama, video, RSS feed, etc.
/// Uses article-style rules instead of novel book source rules.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ArticleSource {
    pub source_name: String,
    pub source_url: String,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub source_icon: Option<String>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub source_comment: Option<String>,
    pub enabled: bool,
    pub enabled_cookie_jar: bool,
    pub enable_js: bool,
    pub load_with_base_url: bool,
    #[serde(deserialize_with = "deser_flex_number")]
    pub article_style: i32,
    #[serde(deserialize_with = "deser_opt_flex_number")]
    pub concurrent_rate: Option<i32>,
    #[serde(deserialize_with = "deser_flex_number")]
    pub custom_order: i32,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub single_url: Option<String>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub sort_url: Option<String>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub js_lib: Option<String>,
    #[serde(deserialize_with = "deser_opt_flex_i64")]
    pub last_update_time: Option<i64>,
    #[serde(default)]
    pub rule_articles: Option<serde_json::Value>,
    #[serde(default)]
    pub rule_content: Option<serde_json::Value>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub rule_image: Option<String>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub rule_link: Option<String>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub rule_next_page: Option<String>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub rule_pub_date: Option<String>,
    #[serde(deserialize_with = "deser_opt_string_or_bool")]
    pub rule_title: Option<String>,
}

fn deser_opt_string_or_bool<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    match v {
        None | Some(serde_json::Value::Null) | Some(serde_json::Value::Bool(false)) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Bool(true)) => Ok(Some("true".to_string())),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        _ => Ok(None),
    }
}

fn deser_flex_number<'de, D>(d: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Number(n) => n.as_i64().map(|i| i as i32).ok_or_else(|| de::Error::custom("not a valid i32")),
        serde_json::Value::String(s) => s.parse::<i32>().map_err(de::Error::custom),
        _ => Err(de::Error::custom("expected number or string")),
    }
}

fn deser_opt_flex_number<'de, D>(d: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    match v {
        None => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(n.as_i64().map(|i| i as i32)),
        Some(serde_json::Value::String(s)) => s.parse::<i32>().map(Some).map_err(de::Error::custom),
        _ => Ok(None),
    }
}
fn deser_opt_flex_i64<'de, D>(d: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    match v {
        None => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(n.as_i64()),
        Some(serde_json::Value::String(s)) => s.parse::<i64>().map(Some).map_err(de::Error::custom),
        _ => Ok(None),
    }
}

/// Multi-category article discovery rules.
/// Keys are category indices; values are RuleSet JSON objects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ArticleRules {
    #[serde(flatten)]
    pub categories: std::collections::HashMap<String, ArticleCategoryRule>,
}

/// Rule set for a single article category.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ArticleCategoryRule {
    pub title: Option<String>,
    pub url: Option<String>,
}

/// Content extraction rule for article sources.
/// The content field maps episode indices to HTML content.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ArticleContentRule {
    #[serde(flatten)]
    pub episodes: std::collections::HashMap<String, String>,
}

/// Metadata for a single article/video item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleItem {
    pub title: String,
    pub url: String,
    pub image_url: Option<String>,
    pub pub_date: Option<String>,
    pub description: Option<String>,
}
