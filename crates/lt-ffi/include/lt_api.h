#include <stdint.h>
/* 事件回调：event_id 见 EVT_* 定义，payload_json 为 UTF-8 JSON 字符串 */
typedef void (*LtEventCallback)(int32_t event_id, const char *payload_json);
/* 事件号（与 lt-task events 常量一致） */
#define EVT_DEVICE_LIST 1
#define EVT_CONN_STATE 2
#define EVT_PAIR_REQUEST 3
#define EVT_TRANSFER_REQUEST 4
#define EVT_TASK_STATE 5
#define EVT_TASK_PROGRESS 6
#define EVT_TASK_SUMMARY 7
#define EVT_ERROR 8

#ifndef LT_API_H
#define LT_API_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * 初始化 LocalTransfer（引擎 + 发现）。`data_dir` 为 NULL 用系统默认目录。
 * 返回 0 成功 / 负错误码。
 */
int lt_init(const char *data_dir);

/**
 * 关闭全部（停发现、停引擎）。幂等。
 */
void lt_shutdown(void);

/**
 * 版本字符串（静态，不需释放）。
 */
const char *lt_version(void);

/**
 * 注册事件回调（覆盖旧回调，传 NULL 清除）。回调在专用分发线程执行。
 */
void lt_set_event_callback(void (*cb)(int event_id, const char *payload_json));

/**
 * 合并更新配置（JSON 片段）。
 */
int lt_set_config(const char *json);

/**
 * 读取全量配置（JSON）。调用方 [`lt_free_string`] 释放。
 */
char *lt_get_config(void);

/**
 * 本机证书指纹（冒号分隔，可用于屏幕比对）。
 */
char *lt_get_local_fingerprint(void);

/**
 * 本机设备信息 JSON（uuid/name/dt/qport/tport/ver/stealth）。
 * Android 侧由 Kotlin 取此信息注册 NSD 服务（Rust 在安卓不跑 mDNS）。
 * 调用方 [`lt_free_string`] 释放。
 */
char *lt_get_local_info(void);

int lt_start_discovery(void);

int lt_stop_discovery(void);

int lt_probe_network(void);

/**
 * 设备列表（JSON 数组）。
 */
char *lt_get_devices(void);

/**
 * 手动添加设备（直连场景）。
 */
int lt_add_manual_device(const char *ip, uint16_t port);

/**
 * 连接设备（异步，结果经 EVT_CONN_STATE / EVT_ERROR）。
 */
int lt_connect(const char *uuid);

/**
 * 直连任意地址。
 */
int lt_connect_addr(const char *ip, uint16_t port);

int lt_disconnect(const char *uuid);

/**
 * 配对应答（pair_id 来自 EVT_PAIR_REQUEST）。
 */
int lt_respond_pair(uint64_t pair_id, int accept);

/**
 * 传输请求应答（req_id 来自 EVT_TRANSFER_REQUEST）。
 */
int lt_respond_transfer(uint64_t req_id, int accept);

/**
 * 发送文件（paths_json 为字符串数组的 JSON）。
 * 成功返回 0 且 `out_task_id` 写入任务 ID。
 */
int lt_send_files(const char *uuid, const char *paths_json, uint64_t *out_task_id);

int lt_cancel_task(uint64_t task_id);

/**
 * 任务列表（JSON 数组）。
 */
char *lt_get_tasks(void);

/**
 * 清除已结束的任务记录。
 */
int lt_clear_records(void);

/**
 * 清理临时缓存（临时文件 + 断点记录）。
 */
int lt_clear_temp_cache(void);

/**
 * Kotlin NsdManager 发现结果注入（JSON）。
 */
int lt_nsd_inject_device(const char *json);

/**
 * Kotlin 侧服务丢失。
 */
int lt_nsd_remove_device(const char *uuid);

/**
 * 释放本库返回的字符串。
 */
void lt_free_string(char *ptr);

#endif  /* LT_API_H */
