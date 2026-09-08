//! 应用层事件（实施方案第十一节，FFI 回调载体）。
//!
//! 每个事件 = 事件号（常量）+ JSON 负载；经 lt-ffi 的
//! `lt_set_event_callback` 推给上层。

pub const EVT_DEVICE_LIST: i32 = 1;
pub const EVT_CONN_STATE: i32 = 2;
pub const EVT_PAIR_REQUEST: i32 = 3;
pub const EVT_TRANSFER_REQUEST: i32 = 4;
pub const EVT_TASK_STATE: i32 = 5;
pub const EVT_TASK_PROGRESS: i32 = 6;
pub const EVT_TASK_SUMMARY: i32 = 7;
pub const EVT_ERROR: i32 = 8;

/// 应用层事件（统一枚举，序列化后交 FFI）。
#[derive(Debug, Clone)]
pub enum LtEvent {
    /// 设备列表变化：`{devices:[…]}`
    DeviceList { devices_json: String },
    /// 连接状态：`{uuid, name, state: connected|disconnected, conn_id, transport, err}`
    ConnState {
        uuid: String,
        name: String,
        state: String,
        conn_id: u64,
        transport: String,
        err: Option<i32>,
    },
    /// 配对请求：`{pair_id, uuid, name, code, is_initiator}`
    PairRequest {
        pair_id: u64,
        uuid: String,
        name: String,
        code: String,
        is_initiator: bool,
    },
    /// 入站传输请求：`{req_id, uuid, name, file_count, total_size}`
    TransferRequest {
        req_id: u64,
        uuid: String,
        name: String,
        file_count: u32,
        total_size: u64,
    },
    /// 任务状态：`{task_id, incoming, state}`
    TaskState {
        task_id: u64,
        incoming: bool,
        state: String,
    },
    /// 任务进度：`{task_id, incoming, rel_path, done, total, rate_bps, eta_secs}`
    TaskProgress {
        task_id: u64,
        incoming: bool,
        rel_path: Option<String>,
        done: u64,
        total: u64,
        rate_bps: u64,
        eta_secs: u64,
    },
    /// 任务汇总：`{task_id, incoming, ok, failed, avg_rate_bps, duration_ms, total_size}`
    TaskSummary {
        task_id: u64,
        incoming: bool,
        ok: u32,
        failed: u32,
        avg_rate_bps: u64,
        duration_ms: u64,
        total_size: u64,
    },
    /// 错误：`{task_id, code, message}`
    Error {
        task_id: Option<u64>,
        code: i32,
        message: String,
    },
}

impl LtEvent {
    pub fn id(&self) -> i32 {
        match self {
            LtEvent::DeviceList { .. } => EVT_DEVICE_LIST,
            LtEvent::ConnState { .. } => EVT_CONN_STATE,
            LtEvent::PairRequest { .. } => EVT_PAIR_REQUEST,
            LtEvent::TransferRequest { .. } => EVT_TRANSFER_REQUEST,
            LtEvent::TaskState { .. } => EVT_TASK_STATE,
            LtEvent::TaskProgress { .. } => EVT_TASK_PROGRESS,
            LtEvent::TaskSummary { .. } => EVT_TASK_SUMMARY,
            LtEvent::Error { .. } => EVT_ERROR,
        }
    }

    /// JSON 负载（上层解析展示）。
    pub fn payload_json(&self) -> String {
        match self {
            LtEvent::DeviceList { devices_json } => {
                let devices: serde_json::Value =
                    serde_json::from_str(devices_json).unwrap_or(serde_json::json!([]));
                serde_json::json!({ "devices": devices }).to_string()
            }
            LtEvent::ConnState {
                uuid,
                name,
                state,
                conn_id,
                transport,
                err,
            } => serde_json::json!({
                "uuid": uuid,
                "name": name,
                "state": state,
                "conn_id": conn_id,
                "transport": transport,
                "err": err,
            })
            .to_string(),
            LtEvent::PairRequest {
                pair_id,
                uuid,
                name,
                code,
                is_initiator,
            } => serde_json::json!({
                "pair_id": pair_id,
                "uuid": uuid,
                "name": name,
                "code": code,
                "is_initiator": is_initiator,
            })
            .to_string(),
            LtEvent::TransferRequest {
                req_id,
                uuid,
                name,
                file_count,
                total_size,
            } => serde_json::json!({
                "req_id": req_id,
                "uuid": uuid,
                "name": name,
                "file_count": file_count,
                "total_size": total_size,
            })
            .to_string(),
            LtEvent::TaskState {
                task_id,
                incoming,
                state,
            } => serde_json::json!({
                "task_id": task_id,
                "incoming": incoming,
                "state": state,
            })
            .to_string(),
            LtEvent::TaskProgress {
                task_id,
                incoming,
                rel_path,
                done,
                total,
                rate_bps,
                eta_secs,
            } => serde_json::json!({
                "task_id": task_id,
                "incoming": incoming,
                "rel_path": rel_path,
                "done": done,
                "total": total,
                "rate_bps": rate_bps,
                "eta_secs": eta_secs,
            })
            .to_string(),
            LtEvent::TaskSummary {
                task_id,
                incoming,
                ok,
                failed,
                avg_rate_bps,
                duration_ms,
                total_size,
            } => serde_json::json!({
                "task_id": task_id,
                "incoming": incoming,
                "ok": ok,
                "failed": failed,
                "avg_rate_bps": avg_rate_bps,
                "duration_ms": duration_ms,
                "total_size": total_size,
            })
            .to_string(),
            LtEvent::Error {
                task_id,
                code,
                message,
            } => serde_json::json!({
                "task_id": task_id,
                "code": code,
                "message": message,
            })
            .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_payloads() {
        let ev = LtEvent::TaskProgress {
            task_id: 7,
            incoming: false,
            rel_path: Some("a/b.txt".into()),
            done: 100,
            total: 200,
            rate_bps: 50,
            eta_secs: 2,
        };
        assert_eq!(ev.id(), EVT_TASK_PROGRESS);
        let v: serde_json::Value = serde_json::from_str(&ev.payload_json()).unwrap();
        assert_eq!(v["task_id"], 7);
        assert_eq!(v["rel_path"], "a/b.txt");

        let dl = LtEvent::DeviceList {
            devices_json: r#"[{"uuid":"x"}]"#.into(),
        };
        let v2: serde_json::Value = serde_json::from_str(&dl.payload_json()).unwrap();
        assert_eq!(v2["devices"][0]["uuid"], "x");
    }
}
