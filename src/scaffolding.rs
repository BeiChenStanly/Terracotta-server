use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;

use socket2::{Domain, SockAddr, Socket, Type};

/// Profile kinds matching Terracotta's protocol.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ProfileKind {
    HOST,
    #[allow(dead_code)]
    LOCAL,
    GUEST,
}

/// Player profile information.
#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub machine_id: String,
    pub name: String,
    pub vendor: String,
    pub kind: ProfileKind,
}

/// Shared state accessible by the scaffolding server.
pub struct ScaffoldingState {
    pub mc_port: u16,
    #[allow(dead_code)]
    pub machine_id: String,
    pub profiles: Vec<(SystemTime, Profile)>,
}

/// A scaffolding server for Terracotta client compatibility.
pub struct ScaffoldingServer {
    pub port: u16,
    pub state: Arc<Mutex<ScaffoldingState>>,
}

impl ScaffoldingServer {
    /// Start the scaffolding server on a random available port.
    pub fn start(mc_port: u16, machine_id: String) -> io::Result<ScaffoldingServer> {
        let state = Arc::new(Mutex::new(ScaffoldingState {
            mc_port,
            machine_id: machine_id.clone(),
            profiles: vec![(
                SystemTime::now(),
                Profile {
                    machine_id: machine_id.clone(),
                    name: "Terracotta Server".to_string(),
                    vendor: format!(
                        "terracotta-server {}, EasyTier {}",
                        env!("CARGO_PKG_VERSION"),
                        env!("TERRACOTTA_ET_VERSION")
                    ),
                    kind: ProfileKind::HOST,
                },
            )],
        }));

        // Try port 13448 first, then 0 (random)
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
        let bind_result = socket.bind(&SockAddr::from(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            13448,
        )));

        let socket = if bind_result.is_err() {
            let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
            socket.bind(&SockAddr::from(SocketAddrV4::new(
                Ipv4Addr::UNSPECIFIED,
                0,
            )))?;
            socket
        } else {
            socket
        };

        let timeout = Duration::from_secs(64);
        socket.set_read_timeout(Some(timeout))?;
        socket.set_write_timeout(Some(timeout))?;
        socket.listen(128)?;

        let port = socket.local_addr()?.as_socket().unwrap().port();
        let state_clone = state.clone();

        // Start profile cleanup thread
        let state_cleanup = state.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));
            let mut state = state_cleanup.lock().unwrap();
            let now = SystemTime::now();
            // Remove stale guest profiles (timeout after 15 seconds)
            state.profiles.retain(|(time, profile)| {
                if profile.kind == ProfileKind::HOST {
                    return true;
                }
                now.duration_since(*time)
                    .map(|d| d < Duration::from_secs(15))
                    .unwrap_or(true)
            });
        });

        // Accept connections
        thread::spawn(move || {
            let listener: TcpListener = socket.into();
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        let state = state_clone.clone();
                        thread::spawn(move || {
                            loop {
                                if let Err(e) = handle_connection(&mut stream, &state) {
                                    log::debug!("[ScaffoldingServer] 连接关闭: {:?}", e);
                                    return;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        log::debug!("[ScaffoldingServer] 接受连接失败: {:?}", e);
                    }
                }
            }
        });

        log::info!("Scaffolding 服务器已启动, 端口: {}", port);

        Ok(ScaffoldingServer { port, state })
    }
}

/// Handle a single request on a scaffolding connection.
fn handle_connection(
    stream: &mut TcpStream,
    state: &Arc<Mutex<ScaffoldingState>>,
) -> io::Result<()> {
    // Read kind length (1 byte)
    let mut kind_size_buf = [0u8; 1];
    stream.read_exact(&mut kind_size_buf)?;
    let kind_size = kind_size_buf[0] as usize;

    // Read kind string
    let mut kind_buf = vec![0u8; kind_size];
    stream.read_exact(&mut kind_buf)?;
    let kind_str =
        String::from_utf8(kind_buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let kinds: Vec<&str> = kind_str.splitn(3, ':').collect();
    if kinds.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid request kind format",
        ));
    }

    // Read body length (4 bytes big-endian)
    let mut body_size_buf = [0u8; 4];
    stream.read_exact(&mut body_size_buf)?;
    let body_size = u32::from_be_bytes(body_size_buf) as usize;

    // Read body
    let mut body = vec![0u8; body_size];
    stream.read_exact(&mut body)?;

    // Process the request
    let (status, response_body) = process_request(kinds[0], kinds[1], &body, state);

    // Build response: [status (1 byte)] [body_length (4 bytes)] [body]
    let mut response = Vec::with_capacity(5 + response_body.len());
    response.push(status);
    response.extend_from_slice(&(response_body.len() as u32).to_be_bytes());
    response.extend_from_slice(&response_body);

    stream.write_all(&response)?;
    stream.flush()?;

    Ok(())
}

/// Process a scaffolding protocol request. Returns (status, response_body).
fn process_request(
    namespace: &str,
    path: &str,
    body: &[u8],
    state: &Arc<Mutex<ScaffoldingState>>,
) -> (u8, Vec<u8>) {
    match (namespace, path) {
        // Ping: echo back the request body
        ("c", "ping") => (0, body.to_vec()),

        // Protocols: list supported protocols
        ("c", "protocols") => {
            let protocols = [
                "c:ping",
                "c:protocols",
                "c:server_port",
                "c:player_ping",
                "c:player_profiles_list",
            ];
            let data = protocols.join("\0");
            (0, data.into_bytes())
        }

        // Server port: return the configured MC port
        ("c", "server_port") => {
            let state = state.lock().unwrap();
            (0, state.mc_port.to_be_bytes().to_vec())
        }

        // Player ping: register/update a player profile
        ("c", "player_ping") => {
            let result: Result<(), String> = (|| {
                let value: Value = serde_json::from_slice(body)
                    .map_err(|e| format!("Invalid JSON: {}", e))?;

                let name = value
                    .as_object()
                    .and_then(|o| o.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'name'")?;
                let machine_id = value
                    .as_object()
                    .and_then(|o| o.get("machine_id"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'machine_id'")?;
                let vendor = value
                    .as_object()
                    .and_then(|o| o.get("vendor"))
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'vendor'")?;

                let mut state = state.lock().unwrap();

                match state
                    .profiles
                    .iter()
                    .position(|(_, p)| p.machine_id == machine_id)
                {
                    Some(i) if i >= 1 => {
                        state.profiles[i].0 = SystemTime::now();
                        if state.profiles[i].1.name != name {
                            state.profiles[i].1.name = name.to_string();
                        }
                    }
                    Some(_) => {
                        // index 0 is the host, cannot modify
                    }
                    None => {
                        state.profiles.push((
                            SystemTime::now(),
                            Profile {
                                machine_id: machine_id.to_string(),
                                name: name.to_string(),
                                vendor: vendor.to_string(),
                                kind: ProfileKind::GUEST,
                            },
                        ));
                        log::info!("玩家加入: {} ({})", name, machine_id);
                    }
                }

                Ok(())
            })();

            match result {
                Ok(()) => (0, vec![]),
                Err(e) => (255, e.into_bytes()),
            }
        }

        // Player profiles list: return all connected player profiles
        ("c", "player_profiles_list") => {
            let state = state.lock().unwrap();
            let profiles: Vec<&Profile> = state.profiles.iter().map(|(_, p)| p).collect();
            match serde_json::to_vec(&profiles) {
                Ok(data) => (0, data),
                Err(e) => (255, format!("Serialization error: {}", e).into_bytes()),
            }
        }

        // Unknown protocol
        _ => (
            255,
            "Requested protocol hasn't been implemented."
                .as_bytes()
                .to_vec(),
        ),
    }
}
