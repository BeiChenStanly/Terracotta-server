mod config;
mod easytier;
mod room;
mod scaffolding;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use clap::Parser;

/// Terracotta Server - Expose Minecraft servers via EasyTier P2P networking.
///
/// A headless (CLI) companion to Terracotta, designed for dedicated Minecraft
/// servers. Reads a TOML config file, starts an EasyTier VPN, and outputs a
/// connection code that Terracotta clients can use to join.
#[derive(Parser, Debug)]
#[command(name = "terracotta-server", version, about)]
struct Args {
    /// Path to the configuration file.
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Generate a default configuration file and exit.
    #[arg(long)]
    init: bool,
}

fn main() {
    // Initialise logger (respects RUST_LOG env var, defaults to info)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let args = Args::parse();

    // --init: write a default config and exit
    if args.init {
        if args.config.exists() {
            log::error!("配置文件已存在: {:?}。如需重新生成请先删除。", args.config);
            std::process::exit(1);
        }
        match config::Config::create_default(&args.config) {
            Ok(()) => {
                log::info!("已生成默认配置文件: {:?}", args.config);
                std::process::exit(0);
            }
            Err(e) => {
                log::error!("{}", e);
                std::process::exit(1);
            }
        }
    }

    // Load configuration
    let cfg = match config::Config::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            log::error!("{}", e);
            log::info!("提示: 运行 `terracotta-server --init` 生成默认配置文件。");
            std::process::exit(1);
        }
    };

    log::info!(
        "Terracotta Server v{}, EasyTier: {}",
        env!("CARGO_PKG_VERSION"),
        env!("TERRACOTTA_ET_VERSION")
    );
    log::info!("Minecraft 服务端端口: {}", cfg.port);

    // ── Extract embedded EasyTier ────────────────────────────────────
    easytier::initialize();

    // ── Determine room (connection code) ──────────────────────────────
    let room = if let Some(ref code) = cfg.code {
        match room::parse_code(code) {
            Some(r) => {
                log::info!("使用固定联机码: {}", r.code);
                r
            }
            None => {
                log::error!("配置文件中的联机码无效: {}", code);
                std::process::exit(1);
            }
        }
    } else {
        let r = room::create_room();
        log::info!("已生成随机联机码（每次启动都会变化）");
        r
    };

    // ── Generate machine ID ──────────────────────────────────────────
    let machine_id = {
        use rand::Rng;
        let bytes: [u8; 16] = rand::thread_rng().r#gen();
        hex::encode(bytes)
    };

    // ── Start scaffolding server ─────────────────────────────────────
    let scaffolding = match scaffolding::ScaffoldingServer::start(cfg.port, machine_id) {
        Ok(s) => s,
        Err(e) => {
            log::error!("无法启动 Scaffolding 服务器: {}", e);
            std::process::exit(1);
        }
    };
    let scaffolding_port = scaffolding.port;
    log::info!("Scaffolding 协议端口: {}", scaffolding_port);

    // ── Build & launch EasyTier ──────────────────────────────────────
    let et_args = easytier::build_arguments(&room, cfg.port, scaffolding_port, &cfg.public_servers);
    let et = match easytier::start(et_args) {
        Ok(et) => et,
        Err(e) => {
            log::error!("{}", e);
            std::process::exit(1);
        }
    };

    // ── Print connection banner ──────────────────────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║            Terracotta Server 已启动              ║");
    println!("║                                                  ║");
    println!("║  联机码 / Connection Code:                       ║");
    println!("║                                                  ║");
    println!("║    {}                     ║", room.code);
    println!("║                                                  ║");
    println!("║  将此联机码分享给好友，                          ║");
    println!("║  他们可以在 Terracotta 客户端中输入此码加入。    ║");
    println!("║                                                  ║");
    println!("║  Share this code with your friends.              ║");
    println!("║  They can enter it in Terracotta to join.        ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    if cfg.code.is_some() {
        log::info!("提示: 联机码已固定。每次启动使用相同的码。");
    } else {
        log::info!(
            "提示: 如需固定联机码，在 config.toml 中设置 code = \"{}\"",
            room.code
        );
    }

    // ── Ctrl-C handler ───────────────────────────────────────────────
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        log::info!("收到终止信号，正在停止...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("无法设置 Ctrl-C 处理器");

    // ── Main loop: monitor EasyTier health ───────────────────────────
    while running.load(Ordering::SeqCst) {
        if !et.is_alive() {
            log::error!("EasyTier 进程已意外退出！");
            break;
        }

        // Periodically log peer count
        if let Some(peers) = et.get_peers() {
            let remote_count = peers.iter().filter(|p| !p.is_local).count();
            if remote_count > 0 {
                log::info!(
                    "当前网络节点数: {} (远程: {})",
                    peers.len(),
                    remote_count
                );
            }
        }

        // Log connected player profiles
        {
            let state = scaffolding.state.lock().unwrap();
            let guest_count = state
                .profiles
                .iter()
                .filter(|(_, p)| p.kind == scaffolding::ProfileKind::GUEST)
                .count();
            if guest_count > 0 {
                log::info!("当前在线玩家数: {}", guest_count);
            }
        }

        thread::sleep(Duration::from_secs(10));
    }

    log::info!("Terracotta Server 正在停止...");
    drop(et); // kill EasyTier process
    easytier::cleanup(); // remove temp files
    log::info!("Terracotta Server 已停止。");
}
