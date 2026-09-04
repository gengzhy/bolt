//! lt-file：文件处理引擎（实施方案第八节）。
//!
//! - [`identity`] 文件身份（相对路径 + 大小 + 修改时间），断点续传匹配依据；
//! - [`ranges`] 已收字节区间集合（支持乱序到达、区间合并）；
//! - [`traverse`] 文件夹递归遍历（跳过符号链接/特殊文件，生成跳过清单）；
//! - [`reader`] memmap2 分片读取 + BLAKE3 流式哈希（支持 100GB 级单文件）；
//! - [`writer`] 分片写入 .tmp 临时文件、校验后落盘、同名策略；
//! - [`resume_store`] 断点缓存持久化；
//! - [`disk`] 磁盘空间预检（总量 × 1.05 + 512MB）。

pub mod disk;
pub mod identity;
pub mod ranges;
pub mod reader;
pub mod resume_store;
pub mod traverse;
pub mod writer;

pub use identity::FileIdentity;
pub use ranges::RangeSet;
pub use traverse::{SkippedEntry, TransferItem, TraverseResult};
