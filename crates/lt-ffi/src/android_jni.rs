//! Android JNI 桥（仅 `target_os = "android"` 编译）。
//!
//! JVM 解析 native 方法只认 `Java_...` 符号或 `RegisterNatives` 注册，
//! 而本 crate 导出的是裸 `lt_*` C 符号（供 lt-cli/Tauri 用），故这里在
//! `JNI_OnLoad` 中把 `com/lt/transfer/ffi/Native` 的 28 个 external fun
//! 按 `include/lt_api.h` 语义一一注册到本模块的实现上。
//!
//! 事件回调：Kotlin 侧传 `(Int, String?) -> Unit`（Function2 对象），
//! 这里保存全局引用并安装 C 蹦床；Rust 事件分发线程（lt-ffi-events）
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
    method!("ltInit", "(Ljava/lang/String;)I", java_ltInit),
    method!("ltShutdown", "()V", java_ltShutdown),
    method!("ltVersion", "()Ljava/lang/String;", java_ltVersion),
    method!(
        "ltSetEventCallback",
        "(Lkotlin/jvm/functions/Function2;)V",
        java_ltSetEventCallback
    ),
    method!("ltSetConfig", "(Ljava/lang/String;)I", java_ltSetConfig),
    method!("ltGetConfig", "()Ljava/lang/String;", java_ltGetConfig),
    method!(
        "ltGetLocalFingerprint",
        "()Ljava/lang/String;",
        java_ltGetLocalFingerprint
    ),
    method!(
        "ltGetLocalInfo",
        "()Ljava/lang/String;",
        java_ltGetLocalInfo
    ),
    method!("ltStartDiscovery", "()I", java_ltStartDiscovery),
    method!("ltStopDiscovery", "()I", java_ltStopDiscovery),
    method!("ltProbeNetwork", "()I", java_ltProbeNetwork),
    method!("ltGetDevices", "()Ljava/lang/String;", java_ltGetDevices),
    method!(
        "ltAddManualDevice",
        "(Ljava/lang/String;I)I",
        java_ltAddManualDevice
    ),
    method!(
        "ltNsdInjectDevice",
        "(Ljava/lang/String;)I",
        java_ltNsdInjectDevice
    ),
    method!(
        "ltNsdRemoveDevice",
        "(Ljava/lang/String;)I",
        java_ltNsdRemoveDevice
    ),
    method!("ltConnect", "(Ljava/lang/String;)I", java_ltConnect),
    method!(
        "ltConnectAddr",
        "(Ljava/lang/String;I)I",
        java_ltConnectAddr
    ),
    method!("ltDisconnect", "(Ljava/lang/String;)I", java_ltDisconnect),
    method!("ltRespondPair", "(JI)I", java_ltRespondPair),
    method!("ltRespondTransfer", "(JI)I", java_ltRespondTransfer),
    method!(
        "ltSendFiles",
        "(Ljava/lang/String;Ljava/lang/String;[J)I",
        java_ltSendFiles
    ),
    method!("ltCancelTask", "(J)I", java_ltCancelTask),
    method!("ltGetTasks", "()Ljava/lang/String;", java_ltGetTasks),
    method!("ltClearRecords", "()I", java_ltClearRecords),
    method!("ltClearTempCache", "()I", java_ltClearTempCache),
    method!("ltFreeString", "(J)V", java_ltFreeString),
];

/// ART 加载 .so 时回调：注册 native 方法。
#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    // Safety: ART 传入合法 VM 指针。
    let vm = unsafe { JavaVM::from_raw(vm) };
    let _ = JVM.set(vm.clone());
    let ok = vm.with_top_local_frame(|env: &mut Env| -> jni::errors::Result<()> {
        let class = env.find_class(jni_str!("com/lt/transfer/ffi/Native"))?;
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

/// C 字符串 → JString；`owned` 为真时随转随释放（lt_get_* 系列）。
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
        crate::lt_free_string(ptr as *mut c_char);
    }
    env.new_string(&text)
}

// ================= 生命周期 =================

#[no_mangle]
pub extern "system" fn java_ltInit<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    data_dir: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let dir = jstring_to_cstring(env, data_dir)?;
        Ok(crate::lt_init(
            dir.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltShutdown<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) {
    let _ = env
        .with_env(|_env| -> jni::errors::Result<()> {
            // 先摘回调，避免核心关闭后事件线程仍持有全局引用。
            *EVENT_CB.lock().unwrap() = None;
            crate::lt_shutdown();
            Ok(())
        })
        .into_outcome();
}

#[no_mangle]
pub extern "system" fn java_ltVersion<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::lt_version(), false)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_ltSetEventCallback<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    cb: JObject<'local>,
) {
    let _ = env
        .with_env(|env| -> jni::errors::Result<()> {
            if cb.is_null() {
                *EVENT_CB.lock().unwrap() = None;
                crate::lt_set_event_callback(None);
            } else {
                let global = env.new_global_ref(cb)?;
                *EVENT_CB.lock().unwrap() = Some(global);
                crate::lt_set_event_callback(Some(event_trampoline));
            }
            Ok(())
        })
        .into_outcome();
}

// ================= 配置 =================

#[no_mangle]
pub extern "system" fn java_ltSetConfig<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    json: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, json)?;
        Ok(crate::lt_set_config(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltGetConfig<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::lt_get_config(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_ltGetLocalFingerprint<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::lt_get_local_fingerprint(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_ltGetLocalInfo<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::lt_get_local_info(), true)
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

passthrough_i32!(java_ltStartDiscovery, crate::lt_start_discovery);
passthrough_i32!(java_ltStopDiscovery, crate::lt_stop_discovery);
passthrough_i32!(java_ltProbeNetwork, crate::lt_probe_network);
passthrough_i32!(java_ltClearRecords, crate::lt_clear_records);
passthrough_i32!(java_ltClearTempCache, crate::lt_clear_temp_cache);

#[no_mangle]
pub extern "system" fn java_ltGetDevices<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::lt_get_devices(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

#[no_mangle]
pub extern "system" fn java_ltAddManualDevice<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    ip: JString<'local>,
    port: jint,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, ip)?;
        Ok(crate::lt_add_manual_device(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
            port as u16,
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltNsdInjectDevice<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    json: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, json)?;
        Ok(crate::lt_nsd_inject_device(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltNsdRemoveDevice<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    uuid: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, uuid)?;
        Ok(crate::lt_nsd_remove_device(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// ================= 连接 / 配对 =================

#[no_mangle]
pub extern "system" fn java_ltConnect<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    uuid: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, uuid)?;
        Ok(crate::lt_connect(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltConnectAddr<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    ip: JString<'local>,
    port: jint,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, ip)?;
        Ok(crate::lt_connect_addr(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
            port as u16,
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltDisconnect<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    uuid: JString<'local>,
) -> jint {
    env.with_env(|env| -> jni::errors::Result<jint> {
        let c = jstring_to_cstring(env, uuid)?;
        Ok(crate::lt_disconnect(
            c.as_deref().map_or(std::ptr::null(), |c| c.as_ptr()),
        ))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltRespondPair<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    pair_id: jlong,
    accept: jint,
) -> jint {
    env.with_env(|_env| -> jni::errors::Result<jint> {
        Ok(crate::lt_respond_pair(pair_id as u64, accept))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn java_ltRespondTransfer<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    req_id: jlong,
    accept: jint,
) -> jint {
    env.with_env(|_env| -> jni::errors::Result<jint> {
        Ok(crate::lt_respond_transfer(req_id as u64, accept))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// ================= 任务 =================

#[no_mangle]
pub extern "system" fn java_ltSendFiles<'local>(
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
        let code = crate::lt_send_files(
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

passthrough_jlong!(java_ltCancelTask, crate::lt_cancel_task);

#[no_mangle]
pub extern "system" fn java_ltGetTasks<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    match env
        .with_env(|env| -> jni::errors::Result<JString> {
            cstring_to_jstring(env, crate::lt_get_tasks(), true)
        })
        .into_outcome()
    {
        Outcome::Ok(js) => js,
        Outcome::Err(_) | Outcome::Panic(_) => JString::null(),
    }
}

// ================= 内存 =================

#[no_mangle]
pub extern "system" fn java_ltFreeString<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    ptr: jlong,
) {
    let _ = env
        .with_env(|_env| -> jni::errors::Result<()> {
            crate::lt_free_string(ptr as *mut c_char);
            Ok(())
        })
        .into_outcome();
}
