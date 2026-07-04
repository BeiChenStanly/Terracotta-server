use std::io::{BufRead, BufReader, Cursor};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use std::{env, fs};

use crate::room::Room;

// ── Embedded EasyTier archive ────────────────────────────────────────
// Populated at build time by build.rs which downloads the platform-specific
// EasyTier release, recompresses to 7z (LZMA2 + BCJ), and bakes it in
// via include_bytes!().  Identical to Terracotta's embedding strategy.

static EASYTIER_ARCHIVE: (&str, &str, &[u8]) = (
    include_str!(env!("TERRACOTTA_ET_ENTRY_CONF")), // e.g. "easytier-core"
    include_str!(env!("TERRACOTTA_ET_CLI_CONF")),    // e.g. "easytier-cli"
    include_bytes!(env!("TERRACOTTA_ET_ARCHIVE")),   // the .7z blob
);

struct EasytierFactory {
    exe: PathBuf,
    cli: PathBuf,
    dir: PathBuf,
}

static FACTORY: OnceLock<EasytierFactory> = OnceLock::new();

fn get_factory() -> &'static EasytierFactory {
    FACTORY.get().expect("EasyTier 尚未初始化，请先调用 initialize()")
}

fn create_factory() -> EasytierFactory {
    let dir = env::temp_dir()
        .join("terracotta-server")
        .join(format!(
            "{}-{}",
            chrono::Local::now().format("%Y%m%d-%H%M%S"),
            std::process::id()
        ))
        .join("embedded-easytier");

    fs::create_dir_all(&dir).expect("无法创建 EasyTier 临时目录");

    log::info!(
        "释放内嵌 EasyTier ({}) 到 {}",
        env!("TERRACOTTA_ET_VERSION"),
        dir.display()
    );

    sevenz_rust2::decompress(Cursor::new(EASYTIER_ARCHIVE.2.to_vec()), &dir)
        .expect("无法解压内嵌 EasyTier 归档");

    let exe = dir.join(EASYTIER_ARCHIVE.0);
    let cli = dir.join(EASYTIER_ARCHIVE.1);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&exe, &cli] {
            let mut perm = fs::metadata(path).unwrap().permissions();
            perm.set_mode(perm.mode() | 0o755);
            fs::set_permissions(path, perm).unwrap();
        }
    }

    EasytierFactory { exe, cli, dir }
}

/// Force initialization — extracts embedded EasyTier to a temp directory.
/// Call once at startup.
pub fn initialize() {
    FACTORY.get_or_init(create_factory);
}

/// Clean up the extracted EasyTier directory.
pub fn cleanup() {
    if let Some(factory) = FACTORY.get() {
        if let Some(parent) = factory.dir.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}

/// Represents a running EasyTier instance.
pub struct EasyTier {
    process: Arc<Mutex<Child>>,
    rpc_port: u16,
}

/// Information about a peer in the EasyTier network.
#[derive(Debug)]
#[allow(dead_code)]
pub struct EasyTierMember {
    pub hostname: String,
    pub address: Option<Ipv4Addr>,
    pub is_local: bool,
}

/// Build command-line arguments for easytier-core (HOST mode).
pub fn build_arguments(
    room: &Room,
    mc_port: u16,
    scaffolding_port: u16,
    public_servers: &[String],
) -> Vec<String> {
    let mut args: Vec<String> = Vec::with_capacity(32);

    // Network identity
    args.extend_from_slice(&[
        "--network-name".to_string(),
        room.network_name.clone(),
        "--network-secret".to_string(),
        room.network_secret.clone(),
    ]);

    // Public relay servers — includes both Terracotta-specific relays
    // (injected by launchers like PCL2/HMCL into Terracotta clients)
    // and the EasyTier community relays.
    let default_servers = [
        "tcp://public0.terracotta.glavo.site:21010",
        "tcp://public1.terracotta.glavo.site:21010",
        "tcp://public.easytier.top:11010",
        "tcp://public2.easytier.cn:54321",
        "https://etnode.zkitefly.eu.org/node1",
        "https://etnode.zkitefly.eu.org/node2",
    ];
    for server in default_servers {
        args.extend_from_slice(&["-p".to_string(), server.to_string()]);
    }
    for server in public_servers {
        args.extend_from_slice(&["-p".to_string(), server.clone()]);
    }

    // Core arguments matching Terracotta host behaviour
    args.push("--no-tun".to_string());
    args.push("--compression=zstd".to_string());
    args.push("--multi-thread".to_string());
    args.push("--latency-first".to_string());
    args.push("--enable-kcp-proxy".to_string());
    args.extend_from_slice(&["-l".to_string(), "udp://0.0.0.0:0".to_string()]);
    args.extend_from_slice(&["-l".to_string(), "tcp://0.0.0.0:0".to_string()]);
    args.push("--p2p-only".to_string());

    // Host identity
    args.extend_from_slice(&[
        "--hostname".to_string(),
        format!("scaffolding-mc-server-{}", scaffolding_port),
    ]);
    args.extend_from_slice(&["--ipv4".to_string(), "10.144.144.1".to_string()]);

    // Whitelist ports
    args.push(format!("--tcp-whitelist={}", scaffolding_port));
    args.push(format!("--tcp-whitelist={}", mc_port));
    args.push(format!("--udp-whitelist={}", mc_port));

    args
}

/// Request a free port on localhost.
fn request_free_port() -> u16 {
    std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0u16))
        .and_then(|socket| socket.local_addr())
        .map(|addr| addr.port())
        .unwrap_or(35780)
}

/// Start an EasyTier process using the embedded executables.
pub fn start(args: Vec<String>) -> Result<EasyTier, String> {
    let factory = get_factory();
    let rpc_port = request_free_port();

    log::info!("启动 EasyTier: rpc={}", rpc_port);
    log::debug!("EasyTier 参数: {:?}", args);

    let mut process = Command::new(&factory.exe)
        .args(&args)
        .args(["-r", &rpc_port.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动 easytier-core: {}", e))?;

    // Quick health check: wait briefly to see if the process exits immediately
    // (e.g. due to missing DLL dependencies like packet.dll from Npcap).
    if cfg!(windows) {
        thread::sleep(Duration::from_millis(500));
    }
    match process.try_wait() {
        Ok(Some(status)) => {
            let code = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| status.to_string());
            log::error!(
                "EasyTier 在启动后立即退出 (状态: {})。\
                 \n  这可能是因为缺少运行时依赖。\
                 \n  在 Windows 上，EasyTier 需要 Npcap (https://npcap.com) 来提供 packet.dll。\
                 \n  请安装 Npcap 后重试。\
                 \n  如果问题仍然存在，请尝试在命令行中直接运行:\
                 \n    {} --help",
                code,
                factory.exe.display()
            );
            return Err(format!("EasyTier 进程启动后立即退出，状态: {}", code));
        }
        Ok(None) => {
            // Process is still running — good.
        }
        Err(e) => {
            log::warn!("检查 EasyTier 进程状态时出错: {}", e);
        }
    }

    // Forward stdout/stderr to logger (take AFTER the health check to avoid
    // consuming the handles before try_wait).
    if let Some(stdout) = process.stdout.take() {
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                log::debug!("[EasyTier stdout] {}", line);
            }
        });
    }

    if let Some(stderr) = process.stderr.take() {
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                log::debug!("[EasyTier stderr] {}", line);
            }
        });
    }

    Ok(EasyTier {
        process: Arc::new(Mutex::new(process)),
        rpc_port,
    })
}

impl EasyTier {
    /// Check if the EasyTier process is still running.
    pub fn is_alive(&self) -> bool {
        matches!(self.process.lock().unwrap().try_wait(), Ok(None))
    }

    /// Query the list of peers.
    pub fn get_peers(&self) -> Option<Vec<EasyTierMember>> {
        let factory = get_factory();
        let output = Command::new(&factory.cli)
            .args([
                "-p",
                &format!("127.0.0.1:{}", self.rpc_port),
                "-o",
                "json",
                "peer",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .ok()?;

        let text = std::str::from_utf8(&output.stdout).ok()?;
        let object: serde_json::Value = serde_json::from_str(text).ok()?;

        let mut members: Vec<EasyTierMember> = vec![];
        for item in object.as_array()? {
            let hostname = item.as_object()?.get("hostname")?.as_str()?.to_string();
            let address = item
                .as_object()?
                .get("ipv4")?
                .as_str()
                .and_then(|s| Ipv4Addr::from_str(s).ok());
            let is_local = item.as_object()?.get("cost")?.as_str()? == "Local";

            members.push(EasyTierMember {
                hostname,
                address,
                is_local,
            });
        }
        Some(members)
    }

    /// Add a port-forward rule.
    #[allow(dead_code)]
    pub fn add_port_forward(
        &self,
        proto: &str,
        local_addr: &str,
        remote_addr: &str,
    ) -> bool {
        let factory = get_factory();
        for attempt in 0..3 {
            match Command::new(&factory.cli)
                .args([
                    "-p",
                    &format!("127.0.0.1:{}", self.rpc_port),
                    "port-forward",
                    "add",
                    proto,
                    local_addr,
                    remote_addr,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
            {
                Ok(output) if output.status.success() => return true,
                Ok(output) => {
                    log::warn!(
                        "端口转发添加失败 (尝试 {}): {}",
                        attempt + 1,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Err(e) => {
                    log::warn!("无法执行 easytier-cli (尝试 {}): {}", attempt + 1, e);
                }
            }
            thread::sleep(Duration::from_millis(500 + attempt as u64 * 1000));
        }
        false
    }
}

impl Drop for EasyTier {
    fn drop(&mut self) {
        log::info!("停止 EasyTier...");
        let _ = self.process.lock().unwrap().kill();
    }
}
