//! build.rs：用 cbindgen 生成 C 头文件 `include/lt_api.h`。

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(&crate_dir).join("include");
    std::fs::create_dir_all(&out_dir).ok();

    let config = cbindgen::Config {
        language: cbindgen::Language::C,
        header: Some(
            "#include <stdint.h>\n\
             /* 事件回调：event_id 见 EVT_* 定义，payload_json 为 UTF-8 JSON 字符串 */\n\
             typedef void (*LtEventCallback)(int32_t event_id, const char *payload_json);\n\
             /* 事件号（与 lt-task events 常量一致） */\n\
             #define EVT_DEVICE_LIST 1\n\
             #define EVT_CONN_STATE 2\n\
             #define EVT_PAIR_REQUEST 3\n\
             #define EVT_TRANSFER_REQUEST 4\n\
             #define EVT_TASK_STATE 5\n\
             #define EVT_TASK_PROGRESS 6\n\
             #define EVT_TASK_SUMMARY 7\n\
             #define EVT_ERROR 8"
                .to_string(),
        ),
        ..Default::default()
    };

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .with_include_guard("LT_API_H")
        .generate()
    {
        Ok(bindings) => {
            bindings.write_to_file(out_dir.join("lt_api.h"));
        }
        Err(e) => {
            // 头文件生成失败不阻塞编译（CI 会单独校验）
            println!("cargo:warning=cbindgen failed: {e}");
        }
    }
}
