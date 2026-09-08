//! lt-cli：命令行测试端（M1 协议联调 / 双机对传，实施方案第十三节）。
//!
//! 用法：
//! ```text
//! lt-cli serve  [--port 8899] [--name 名称] [--data-dir DIR]   # 监听并接收
//! lt-cli discover [--data-dir DIR]                              # 仅浏览设备
//! lt-cli send [--data-dir DIR] [--port 8902] <uuid|ip[:port]> <文件...>
//! ```
//!
//! 环回冒烟（同机两实例）：
//! ```text
//! lt-cli serve --port 8901 --data-dir target\\data_a
//! lt-cli send  --port 8902 --data-dir target\\data_b 127.0.0.1:8901 some.bin
//! ```

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc;

use lt_task::{App, LtEvent};

struct Args {
    data_dir: Option<PathBuf>,
    port: Option<u16>,
    name: Option<String>,
    rest: Vec<String>,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args {
        data_dir: None,
        port: None,
        name: None,
        rest: Vec::new(),
    };
    let mut i = 0;
    // 跳过子命令
    if matches!(
        argv.first().map(|s| s.as_str()),
        Some("serve" | "discover" | "send")
    ) {
        i = 1;
    }
    while i < argv.len() {
        match argv[i].as_str() {
            "--data-dir" => {
                i += 1;
                if i < argv.len() {
                    args.data_dir = Some(PathBuf::from(&argv[i]));
                }
            }
            "--port" => {
                i += 1;
                if i < argv.len() {
                    args.port = argv[i].parse().ok();
                }
            }
            "--name" => {
                i += 1;
                if i < argv.len() {
                    args.name = Some(argv[i].clone());
                }
            }
            other => args.rest.push(other.to_string()),
        }
        i += 1;
    }
    args
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .try_init();
}

fn make_app(args: &Args) -> anyhow::Result<std::sync::Arc<App>> {
    // --port：在初始化前把端口写入配置（init 读取）
    if let Some(p) = args.port {
        let dir = args
            .data_dir
            .clone()
            .unwrap_or_else(lt_utils::config::default_data_dir);
        std::fs::create_dir_all(&dir).ok();
        let cfg_path = dir.join("config.json");
        let mut v: serde_json::Value = std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        v["listen_port"] = serde_json::json!(p);
        std::fs::write(&cfg_path, v.to_string()).ok();
        return Ok(App::init(Some(dir))?);
    }
    Ok(App::init(args.data_dir.clone())?)
}

fn prompt(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim(), "y" | "Y" | "yes" | "是")
}

fn print_devices(json: &str) {
    let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(json) else {
        return;
    };
    eprintln!("---- 设备列表（{} 台）----", list.len());
    for d in &list {
        eprintln!(
            "  {} | {} | {} | QUIC:{} TCP:{} | via {}",
            d["uuid"].as_str().unwrap_or("?"),
            d["name"].as_str().unwrap_or("?"),
            d["ip"].as_str().unwrap_or("?"),
            d["quic_port"],
            d["tcp_port"],
            d["source"].as_str().unwrap_or("?"),
        );
    }
}

fn run_event_loop(app: &std::sync::Arc<App>, rx: mpsc::Receiver<LtEvent>, mode: Mode) -> i32 {
    while let Ok(ev) = rx.recv() {
        match ev {
            LtEvent::DeviceList { devices_json } => print_devices(&devices_json),
            LtEvent::ConnState {
                uuid,
                name,
                state,
                conn_id: _,
                transport,
                err,
            } => {
                eprintln!("[连接] {name}({uuid}) {state} {transport} err={err:?}");
            }
            LtEvent::PairRequest {
                pair_id,
                uuid,
                name,
                code,
                is_initiator,
            } => {
                if is_initiator {
                    eprintln!("[配对] 正在与 {name}({uuid}) 配对，验证码：【{code}】，等待对方设备确认...");
                } else {
                    eprintln!("[配对] {name}({uuid}) 请求配对，验证码：【{code}】");
                    if mode == Mode::Send {
                        // 发送端自动接受配对（测试便利；正式端由用户确认）
                        eprintln!("  （send 模式自动接受配对）");
                        app.respond_pair(pair_id, true);
                    } else if prompt("  屏幕验证码一致，接受配对？") {
                        app.respond_pair(pair_id, true);
                    } else {
                        app.respond_pair(pair_id, false);
                    }
                }
            }
            LtEvent::TransferRequest {
                req_id,
                uuid,
                name,
                file_count,
                total_size,
            } => {
                eprintln!(
                    "[传输请求] {name}({uuid}) 要发送 {file_count} 个文件，共 {} 字节",
                    total_size
                );
                if prompt("  接受？") {
                    app.respond_transfer(req_id, true);
                } else {
                    app.respond_transfer(req_id, false);
                }
            }
            LtEvent::TaskState {
                task_id,
                incoming,
                state,
            } => {
                eprintln!(
                    "[任务 {task_id} {}] 状态：{state}",
                    if incoming { "接收" } else { "发送" }
                );
                if (state == "done" || state == "error" || state == "cancelled")
                    && mode == Mode::Send
                {
                    return if state == "done" { 0 } else { 1 };
                }
            }
            LtEvent::TaskProgress {
                task_id: _,
                incoming,
                rel_path,
                done,
                total,
                rate_bps,
                eta_secs,
            } => {
                let pct = if total > 0 {
                    done as f64 / total as f64 * 100.0
                } else {
                    0.0
                };
                print!(
                    "\r[{} {}] {:.1}% {}/{} 速率 {}/s 剩余 {}s   ",
                    if incoming { "接收" } else { "发送" },
                    rel_path.as_deref().unwrap_or(""),
                    pct,
                    human(done),
                    human(total),
                    human(rate_bps),
                    eta_secs
                );
                let _ = std::io::stdout().flush();
            }
            LtEvent::TaskSummary {
                task_id,
                incoming,
                ok,
                failed,
                ..
            } => {
                eprintln!();
                eprintln!(
                    "[汇总] 任务 {task_id} {}：成功 {ok}，失败 {failed}",
                    if incoming { "接收" } else { "发送" }
                );
                if mode == Mode::Send {
                    return if failed == 0 { 0 } else { 1 };
                }
            }
            LtEvent::Error {
                task_id,
                code,
                message,
            } => {
                eprintln!("[错误] code={code} task={task_id:?} {message}");
                if mode == Mode::Send && task_id.is_some() {
                    return 1;
                }
            }
        }
        // discover 模式下每收到一次列表即继续等待；serve 模式持续
    }
    0
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Serve,
    Discover,
    Send,
}

fn human(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1}{}", UNITS[i])
}

fn main() -> anyhow::Result<()> {
    init_tracing();
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let args = parse_args();

    match cmd.as_str() {
        "serve" | "discover" | "send" => {}
        _ => {
            eprintln!("用法：lt-cli <serve|discover|send> [选项]");
            eprintln!("  serve    [--port N] [--name X] [--data-dir DIR]");
            eprintln!("  discover [--data-dir DIR]");
            eprintln!("  send     [--data-dir DIR] [--port N] <uuid|ip[:port]> <文件...>");
            std::process::exit(2);
        }
    }

    // 事件队列（回调线程 → 主线程交互）
    let (tx, rx) = mpsc::channel::<LtEvent>();
    let app = make_app(&args)?;
    app.set_event_sink(move |ev| {
        let _ = tx.send(ev);
    });

    if let Some(name) = &args.name {
        app.set_config(&format!(r#"{{"device_name":"{name}"}}"#))?;
    }

    match cmd.as_str() {
        "serve" => {
            eprintln!(
                "已就绪：uuid={} 端口={} 指纹={}",
                app.device_uuid(),
                app.engine_port(),
                app.local_fingerprint()
            );
            eprintln!("等待配对/传输请求…（Ctrl-C 退出）");
            let code = run_event_loop(&app, rx, Mode::Serve);
            app.shutdown();
            std::process::exit(code);
        }
        "discover" => {
            eprintln!("正在浏览局域网设备…（Ctrl-C 退出）");
            let code = run_event_loop(&app, rx, Mode::Discover);
            app.shutdown();
            std::process::exit(code);
        }
        "send" => {
            if args.rest.len() < 2 {
                eprintln!("send 需要目标与至少一个文件路径");
                std::process::exit(2);
            }
            let target = args.rest[0].clone();
            let files: Vec<String> = args.rest[1..].to_vec();

            // 目标：uuid 或 ip[:port]
            let target_uuid = if target.contains(':') || target.parse::<std::net::IpAddr>().is_ok()
            {
                let (ip, port) = match target.split_once(':') {
                    Some((ip, p)) => (ip.to_string(), p.parse().unwrap_or(lt_utils::DEFAULT_PORT)),
                    None => (target.clone(), lt_utils::DEFAULT_PORT),
                };
                app.add_manual_device(&ip, port);
                format!("manual:{ip}")
            } else {
                target.clone()
            };

            eprintln!("发送 {} 个路径 → {target}", files.len());
            match app.send_files(&target_uuid, &files) {
                Ok(task_id) => {
                    eprintln!("任务已创建：task_id={task_id}，等待对方接受…");
                    let code = run_event_loop(&app, rx, Mode::Send);
                    app.shutdown();
                    std::process::exit(code);
                }
                Err(e) => {
                    eprintln!("发送失败：{e}（错误码 {}）", e.code());
                    std::process::exit(1);
                }
            }
        }
        _ => unreachable!(),
    }
}
