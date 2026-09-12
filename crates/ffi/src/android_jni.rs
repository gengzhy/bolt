//! Android JNI 桥（仅 `target_os = "android"` 编译）。
//!
//! JVM 解析 native 方法只认 `Java_...` 符号或 `RegisterNatives` 注册，
//! 而本 crate 导出的是裸 `bt_*` C 符号（供 cli/Tauri 用），故这里在
//! `JNI_OnLoad` 中把 `xin/cosmos/bolt/ffi/Native` 的 28 个 external fun
//! 按 `include/bt_api.h` 语义一一注册到本模块的实现上。
//!
//! 事件回调：Kotlin 侧传 `(Int, String?) -> Unit`（Function2 对象），
//! 这里保存全局引用并安装 C 蹦床；Rust 事件分发线程（bt-ffi-events）
//! 经 `attach_current_thread` 回调 Kotlin，由 Kotlin 侧再转主线程。

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Mutex, OnceLock};

use jni::errors::{Error as JniError, ThrowRuntimeExAndDefault};
use jni::objects::{Global, JLongArray, JObject, JString, JValue};
use jni::sys::{jint, jlong, JNI_VERSION_1_6};
use jni::{jni_sig, jni_str, Env, EnvUnowned, JavaVM, NativeMethod, Outcome};

/// Android 进程的 JavaVM（JNI_OnLoad 时记录，供回调蹦床 attach 线程）。
static JVM: OnceLock<JavaVM> = OnceLock::new();
/// Kotlin `(Int, String?) -> Unit` 回调对象的全局引用。
static EVENT_CB: Mutex<Option<Global<JObject<'static>>>> = Mutex::new(None);

/// 注册表项宏：name/sig 与 Kotlin `Native.kt` 的 external fun 一一对应
/// （签名以 javap 反编译 `Native.class` 的 descriptor 为准）。
macro_rules! method {
    ($name:literal, $sig:literal, $f:ident) => {
        // Safety: fn_ptr 指向与 sig 匹配的 extern "system" 实现。
        unsafe { NativeMethod::from_raw_parts(jni_str!($name), jni_str!($sig), $f as *mut c_void) }
    };
}

const METHODS: &[NativeMethod<'static>] = &[
    method!("btInit", "(Ljava/lang/String;)I", java_btInit),
    method!("btShutdown", "()V", java_btShutdown),
    method!("btVersion", "()Ljava/lang/String;", java_btVersion),
    method!(
        "btSetEventCallback",
        "(Lkotlin/jvm/functions/Function2;)V",
        java_btSetEventCallback
    ),
    method!("btSetConfig", "(Ljava/lang/String;)I", java_btSetConfig),
    method!("btGetConfig", "()Ljava/lang/String;", java_btGetConfig),
    method!(
        "btGetLocalFingerprint",
        "()Ljava/lang/String;",
        java_btGetLocalFingerprint
    ),
    method!(
        "btGetLocalInfo",
        "()Ljava/lang/String;",
        java_btGetLocalInfo
    ),
    method!("btStartDiscovery", "()I", java_btStartDiscovery),
    method!("btStopDiscovery", "()I", java_btStopDiscovery),
    method!("btProbeNetwork", "()I", java_btProbeNetwork),
    method!("btGetDevices", "()Ljava/lang/String;", java_btGetDevices),
    method!(
        "btAddManualDevice",
        "(Ljava/lang/String;I)I",
        java_btAddManualDevice
    ),
    method!(
        "btNsdInjectDevice",
        "(Ljava/lang/String;)I",
        java_btNsdInjectDevice
    ),
    method!(
        "btNsdRemoveDevice",
        "(Ljava/lang/String;)I",
        java_btNsdRemoveDevice
    ),
    method!("btConnect", "(Ljava/lang/String;)I", java_btConnect),
    method!(
        "btConnectAddr",
        "(Ljava/lang/String;I)I",
        java_btConnectAddr
    ),
    method!("btDisconnect", "(Ljava/lang/String;)I", java_btDisconnect),
    method!("btRespondPair", "(JI)I", java_btRespondPair),
    method!("btRespondTransfer", "(JI)I", java_btRespondTransfer),
    method!(
        "btSendFiles",
        "(Ljava/lang/String;Ljava/lang/String;[J)I",
        java_btSendFiles
    ),
    method!("btCancelTask", "(J)I", java_btCancelTask),
    method!("btGetTasks", "()Ljava/lang/String;", java_btGetTasks),
    method!("btClearRecords", "()I", java_btClearRecords),
    method!("btClearTempCache", "()I", java_btClearTempCache),
    method!("btFreeString", "(J)V", java_btFreeString),
];

/// ART 加载 .so 时回调：注册 native 方法。
#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    // Safety: ART 传入合法 VM 指针。
    let vm = unsafe { JavaVM::from_raw(vm) };
    let _ = JVM.set(vm.clone());
    let ok = vm.with_top_local_frame(|env: &mut Env| -> jni::errors::Result<()> {
        let class = env.find_class(jni_str!("xin/cosmos/bolt/ffi/Native"))?;
        // Safety: METHODS 每项 fn_ptr 都与 sig 匹配。
        unsafe { env.register_native_methods(&class, METHODS) }
    });
    if ok.is_err() {
        return jni::sys::JNI_ERR;
    }
    JNI_VERSION_1_6
}

// ================= 事件回调蹦床 =================

/// C 侧事件回调蹦床：在 Rust 事件分发线程执行，attach 后转调 Kotlin。
extern "C" fn event_trampoline(id: c_int, payload: *const c_char) {
    let Some(vm) = JVM.get() else { return };
    let payload = if payload.is_null() {
        None
    } else {
        // Safety: 核心库保证 payload 在回调期间有效。
        unsafe { CStr::from_ptr(payload) }
            .to_str()
            .ok()
            .map(str::to_owned)
    };
    let guard = EVENT_CB.lock().unwrap();
    let Some(cb) = guard.as_ref() else {
        return;
    };
    let _ = vm.attach_current_thread(|env: &mut Env| -> jni::errors::Result<()> {
        let js: JObject = match &payload {
            Some(s) => env.new_string(s)?.into(),
            None => JObject::null(),
        };
        // Kotlin `(Int, String?) -> Unit` 编译为 kotlin.jvm.functions.Function2，
        // 其 invoke 的 JVM 描述符是装箱形态 `(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;`，
        // 传原始 `(ILjava/lang/String;)V` 会 NoSuchMethodError，事件被静默丢弃。
        let boxed_id = env.new_object(
            jni_str!("java/lang/Integer"),
            jni_sig!(sig = "(I)V"),
            &[JValue::Int(id)],
        )?;
        env.call_method(
            cb.as_ref(),
            jni_str!("invoke"),
            jni_sig!(sig = "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"),
            &[JValue::Object(&boxed_id), JValue::Object(&js)],
        )?;
        Ok(())
    });
}

// ================= 工具 =================

/// JString → CString；null 返回 None（C 侧拿 NULL 指针）。
fn jstring_to_cstring<'local>(
    env: &mut Env<'local>,
    js: JString<'local>,
) -> Result<Option<CString>, JniError> {
    if js.is_null() {
        return Ok(None);
    }
    let s: String = js.mutf8_chars(env)?.into();
    Ok(Some(CString::new(s).map_err(|_| JniError::JavaException)?))
}

/// C 字符串 → JString；`owned` 为真时随转随释放（bt_get_* 系列）。
fn cstring_to_jstring<'local>(
    env: &mut Env<'local>,
    ptr: *const c_char,
    owned: bool,
) -> Result<JString<'local>, JniError> {
    let text = if ptr.is_null() {
        String::new()
    } else {
        // Safety: 核心库返回 NUL 结尾 UTF-8（或 NULL）。
        unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .unwrap_or("")
            .to_owned()
    };
    if owned && !ptr.is_null() {
        crate::bt_free_string(ptr as *mut c_char);
    }
    env.new_string(&text)
}

// ================= 生命周期 =================

#[no_mangle]
pub extern "system" fn java_btInit<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    data_dir: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let dir = jstring_to_cstring(env, data_dir)?;
        Ok(crate::bt_init(
            dir.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btShutdown<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) {
    let _ = env
        .with_env(|_env| -> jni::errors::Result<()> {
            // 先摘回调，避免核心关闭后事件线程仍持有全局引用。
            *EVENT_CB.lock().unwrap() = None;
            crate::bt_shutdown();
            Ok(())
        })
        .into_outcome();
}

#[no_mangle]
pub extern "system" fn java_btVersion<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::bt_version(), false)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_btSetEventCallback<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    cb: JObject<'local>,
) {
    let _ = env
        .with_env(|env| -> jni::errors::Result<()> {
            if cb.is_null() {
                *EVENT_CB.lock().unwrap() = None;
                crate::bt_set_event_callback(None);
            } else {
                let global = env.new_global_ref(cb)?;
                *EVENT_CB.lock().unwrap() = Some(global);
                crate::bt_set_event_callback(Some(event_trampoline));
            }
            Ok(())
        })
        .into_outcome();
}

// ================= 配置 =================

#[no_mangle]
pub extern "system" fn java_btSetConfig<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    json: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, json)?;
        Ok(crate::bt_set_config(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btGetConfig<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::bt_get_config(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_btGetLocalFingerprint<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::bt_get_local_fingerprint(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_btGetLocalInfo<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::bt_get_local_info(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

// ================= 发现 =================

macro_rules! passthrough_i32 {
    ($name:ident, $core:path) => {
        #[no_mangle]
        pub extern "system" fn $name<'local>(
            mut env: EnvUnowned<'local>,
            _this: JObject<'local>,
        ) -> jint {
            env.with_env(|_env| -> jni::errors::Result<jint> { Ok($core()) })
                .resolve::<ThrowRuntimeExAndDefault>()
        }
    };
}

passthrough_i32!(java_btStartDiscovery, crate::bt_start_discovery);
passthrough_i32!(java_btStopDiscovery, crate::bt_stop_discovery);
passthrough_i32!(java_btProbeNetwork, crate::bt_probe_network);
passthrough_i32!(java_btClearRecords, crate::bt_clear_records);
passthrough_i32!(java_btClearTempCache, crate::bt_clear_temp_cache);

#[no_mangle]
pub extern "system" fn java_btGetDevices<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::bt_get_devices(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_btAddManualDevice<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    ip: JString<'local>,
    port: jint,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, ip)?;
        Ok(crate::bt_add_manual_device(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
            port as u16,
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btNsdInjectDevice<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    json: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, json)?;
        Ok(crate::bt_nsd_inject_device(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btNsdRemoveDevice<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    uuid: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, uuid)?;
        Ok(crate::bt_nsd_remove_device(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// ================= 连接 / 配对 =================

#[no_mangle]
pub extern "system" fn java_btConnect<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    uuid: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, uuid)?;
        Ok(crate::bt_connect(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btConnectAddr<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    ip: JString<'local>,
    port: jint,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, ip)?;
        Ok(crate::bt_connect_addr(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
            port as u16,
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btDisconnect<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    uuid: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, uuid)?;
        Ok(crate::bt_disconnect(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btRespondPair<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    pair_id: jlong,
    accept: jint,
) -> jint {
    env.with_env(|_env| -> jni::errors::Result<jint> {
        Ok(crate::bt_respond_pair(pair_id as u64, accept))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_btRespondTransfer<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    req_id: jlong,
    accept: jint,
) -> jint {
    env.with_env(|_env| -> jni::errors::Result<jint> {
        Ok(crate::bt_respond_transfer(req_id as u64, accept))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// ================= 任务 =================

#[no_mangle]
pub extern "system" fn java_btSendFiles<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    uuid: JString<'local>,
    paths_json: JString<'local>,
    out_task_id: JLongArray<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let uuid = jstring_to_cstring(env, uuid)?;
        let paths = jstring_to_cstring(env, paths_json)?;
        let mut task_id: u64 = 0;
        let code = crate::bt_send_files(
            uuid.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
            paths.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
            &mut task_id,
        );
        if !out_task_id.is_null() {
            out_task_id.set_region(env, 0, &[task_id as jlong])?;
        }
        Ok(code)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

macro_rules! passthrough_jlong {
    ($name:ident, $core:path) => {
        #[no_mangle]
        pub extern "system" fn $name<'local>(
            mut env: EnvUnowned<'local>,
            _this: JObject<'local>,
            task_id: jlong,
        ) -> jint {
            env.with_env(|_env| -> jni::errors::Result<jint> { Ok($core(task_id as u64)) })
                .resolve::<ThrowRuntimeExAndDefault>()
        }
    };
}

passthrough_jlong!(java_btCancelTask, crate::bt_cancel_task);

#[no_mangle]
pub extern "system" fn java_btGetTasks<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::bt_get_tasks(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

// ================= 内存 =================

#[no_mangle]
pub extern "system" fn java_btFreeString<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    ptr: jlong,
) {
    let _ = env
        .with_env(|_env| -> jni::errors::Result<()> {
            crate::bt_free_string(ptr as *mut c_char);
            Ok(())
        })
        .into_outcome();
}
