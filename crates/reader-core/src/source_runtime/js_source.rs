use crate::dto::{BookDetail, BookItem, ChapterItem};
use crate::error::ReaderCoreError;
use crate::parser::js::{eval_source_function, eval_source_function_value, JsSourceArg};
use serde_json::Value;

pub struct JsSourceRuntime {
    file_name: String,
    content: String,
}

impl JsSourceRuntime {
    pub fn new(file_name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            file_name: file_name.into(),
            content: content.into(),
        }
    }

    pub fn search(&self, keyword: &str, page: i32) -> Result<Vec<BookItem>, ReaderCoreError> {
        let raw = self.call_first(
            &["search"],
            &[
                JsSourceArg::String(keyword.to_string()),
                JsSourceArg::Int(page),
            ],
        )?;
        serde_json::from_value(expect_json(raw, "search")?)
            .map_err(|err| js_shape_error(&self.file_name, "search", err))
    }

    pub fn book_info(&self, book_url: &str) -> Result<BookDetail, ReaderCoreError> {
        let raw = self.call_first(&["bookInfo"], &[JsSourceArg::String(book_url.to_string())])?;
        let mut detail: BookDetail = serde_json::from_value(expect_json(raw, "bookInfo")?)
            .map_err(|err| js_shape_error(&self.file_name, "bookInfo", err))?;
        if detail
            .book_url
            .as_deref()
            .map_or(true, |value| value.trim().is_empty())
        {
            detail.book_url = Some(book_url.to_string());
        }
        Ok(detail)
    }

    pub fn chapter_list(&self, toc_url: &str) -> Result<Vec<ChapterItem>, ReaderCoreError> {
        let raw = self.call_first(
            &["chapterList", "toc"],
            &[JsSourceArg::String(toc_url.to_string())],
        )?;
        serde_json::from_value(expect_json(raw, "chapterList")?)
            .map_err(|err| js_shape_error(&self.file_name, "chapterList", err))
    }

    pub fn chapter_content(&self, chapter_url: &str) -> Result<String, ReaderCoreError> {
        let raw = self.call_first(
            &["chapterContent", "content"],
            &[JsSourceArg::String(chapter_url.to_string())],
        )?;
        Ok(match serde_json::from_str::<Value>(&raw) {
            Ok(Value::String(value)) => value,
            Ok(value) => serde_json::to_string(&value)?,
            Err(_) => raw,
        })
    }

    pub fn explore(&self, page: i32, category: &str) -> Result<Value, ReaderCoreError> {
        let raw = self.call_first(
            &["explore"],
            &[
                JsSourceArg::Int(page),
                JsSourceArg::String(category.to_string()),
            ],
        )?;
        match serde_json::from_str::<Value>(&raw) {
            Ok(value) => Ok(value),
            Err(_) => Ok(Value::String(raw)),
        }
    }

    pub fn call_function(
        &self,
        function_name: &str,
        args: &[JsSourceArg],
    ) -> Result<Value, ReaderCoreError> {
        eval_source_function_value(&self.content, function_name, args).map_err(|err| {
            ReaderCoreError::Message(format!(
                "JS 书源执行失败 [{}::{function_name}]: {err}",
                self.file_name
            ))
        })
    }

    fn call_first(&self, names: &[&str], args: &[JsSourceArg]) -> Result<String, ReaderCoreError> {
        for name in names {
            if !has_js_function(&self.content, name) {
                continue;
            }
            return eval_source_function(&self.content, name, args).map_err(|err| {
                ReaderCoreError::Message(format!(
                    "JS 书源执行失败 [{}::{name}]: {err}",
                    self.file_name
                ))
            });
        }
        Err(ReaderCoreError::Message(format!(
            "JS 书源缺少函数 [{}]: {}",
            self.file_name,
            names.join("/")
        )))
    }
}

fn expect_json(raw: String, function_name: &str) -> Result<Value, ReaderCoreError> {
    serde_json::from_str::<Value>(&raw).map_err(|err| {
        ReaderCoreError::Message(format!(
            "{function_name} 返回值不是可解析 JSON: {err}; raw={raw}"
        ))
    })
}

fn js_shape_error(file_name: &str, function_name: &str, err: serde_json::Error) -> ReaderCoreError {
    ReaderCoreError::Message(format!(
        "JS 书源返回结构不兼容 [{}::{function_name}]: {err}",
        file_name
    ))
}

fn has_js_function(content: &str, name: &str) -> bool {
    let pattern = format!(
        r"(async\s+function\s+{0}\b|function\s+{0}\b|(const|let|var)\s+{0}\s*=)",
        regex::escape(name)
    );
    regex::Regex::new(&pattern)
        .map(|re| re.is_match(content))
        .unwrap_or(false)
}
