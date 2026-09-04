//! lt-task：任务调度与应用门面（实施方案第九节）。
//!
//! - [`events`] 应用层事件（EVT_*，FFI 回调载体）；
//! - [`task`] 任务记录与状态机（发送/接收入同一队列）；
//! - [`app`] [`App`] 门面：配置、身份、信任库、引擎、发现、任务、事件总线。

pub mod app;
pub mod events;
pub mod task;

pub use app::App;
pub use events::*;
pub use task::{Direction, TaskRecord, TaskState};
