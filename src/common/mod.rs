pub mod logs_data;
pub mod logs_parser;
pub mod logs_filters;
pub mod logs_subscribe;
pub mod logs_events;
pub mod error;

pub type AnyResult<T> = anyhow::Result<T>;

