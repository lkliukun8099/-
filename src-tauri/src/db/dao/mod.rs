//! DAO 层：一表（或一组紧耦合表）一个 DAO。
//!
//! 所有 DAO 以 `&Connection` 注入，便于单元测试使用内存库（架构 §9-9）。

pub mod attribution;
pub mod kg;
pub mod log;
pub mod mastery;
pub mod mistake;
pub mod model;
pub mod review;
pub mod variant;

use serde::Serialize;

/// 通用分页结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

impl<T: Serialize> Page<T> {
    pub fn new(items: Vec<T>, total: i64, page: i64, page_size: i64) -> Self {
        Self {
            items,
            total,
            page,
            page_size,
        }
    }
}
