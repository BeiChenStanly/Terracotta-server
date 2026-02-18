# Terracotta Server

[English](#english) | [中文](#中文)

---

## English

**Terracotta Server** is a headless (CLI) companion to [Terracotta](https://github.com/burningtnt/Terracotta), designed for **dedicated Minecraft servers**. It exposes a Minecraft server to the Terracotta P2P network, allowing players using Terracotta clients to join via a connection code — no port forwarding required.

### Features

- **Full Terracotta compatibility** — generates the same `U/XXXX-XXXX-XXXX-XXXX` connection codes used by Terracotta clients and launchers (PCL2, HMCL, etc.)
- **Embedded EasyTier** — the P2P networking engine is bundled at build time; no external downloads or configuration needed
- **Fixed connection codes** — optionally set a permanent code so players always use the same address
- **Scaffolding protocol** — implements Terracotta's scaffolding server protocol for player tracking, ping, and profile exchange
- **Pure CLI** — designed for headless servers with no GUI dependencies
- **Cross-platform** — Linux (x86_64, aarch64), Windows (x86_64), macOS (x86_64, aarch64)

### Quick Start

1. Place the `terracotta-server` binary and `config.toml` in the same directory as your Minecraft server (or anywhere you like).

2. Edit `config.toml` if needed:

   ```toml
   # Minecraft server port (default: 25565)
   port = 25565
   
   # Fixed connection code (optional)
   # code = "U/ABCD-1234-EFGH-5678"
   
   # Additional relay servers (optional)
   public_servers = []
   ```

3. Start the server:

   ```bash
   ./terracotta-server
   ```

4. Share the displayed connection code with your players. They enter it in their Terracotta client to join.

### Usage

```
terracotta-server [OPTIONS]

Options:
  -c, --config <PATH>   Path to configuration file [default: config.toml]
      --init             Generate a default configuration file and exit
  -h, --help             Print help
  -V, --version          Print version
```

Generate a default config file:

```bash
./terracotta-server --init
```

Use a custom config path:

```bash
./terracotta-server --config /path/to/my-config.toml
```

### Configuration

| Field            | Type       | Default | Description                                                                         |
| ---------------- | ---------- | ------- | ----------------------------------------------------------------------------------- |
| `port`           | `u16`      | `25565` | The Minecraft server port to expose through the P2P network                         |
| `code`           | `string?`  | *none*  | Fixed connection code (`U/XXXX-XXXX-XXXX-XXXX`). If omitted, a random code is generated on each startup |
| `public_servers` | `string[]` | `[]`    | Additional relay servers to use beyond the built-in defaults                        |

### Building from Source

**Prerequisites:** Rust 1.75+ (stable), internet access (the build script downloads EasyTier binaries).

```bash
cargo build --release
```

The build script (`build.rs`) automatically:
1. Downloads the correct EasyTier release for your target platform
2. Recompresses it with LZMA2 + BCJ filters for smaller binary size
3. Embeds the archive into the binary via `include_bytes!()`

The resulting binary is fully self-contained.

### How It Works

1. **Startup** — reads `config.toml`, extracts embedded EasyTier to a temp directory
2. **Room creation** — generates (or loads from config) a connection code using Terracotta's base-34 encoding algorithm
3. **Scaffolding server** — starts a TCP server implementing Terracotta's scaffolding protocol for player tracking
4. **EasyTier launch** — starts the P2P VPN with the room's network credentials, connecting to public relay servers
5. **Ready** — displays the connection code; players enter it in their Terracotta client to join
6. **Shutdown** — Ctrl+C gracefully stops EasyTier and cleans up temp files

### License

This project is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE), consistent with the upstream [Terracotta](https://github.com/burningtnt/Terracotta) project.

Copyright of [Terracotta](https://github.com/burningtnt/Terracotta) belongs to its original authors.

---

## 中文

**Terracotta Server** 是 [Terracotta](https://github.com/burningtnt/Terracotta) 的服务端伴侣工具（纯命令行），专为**独立 Minecraft 服务器**设计。它将 Minecraft 服务器暴露到 Terracotta P2P 网络中，让使用 Terracotta 客户端的玩家通过联机码加入——无需端口转发。

### 特性

- **完全兼容 Terracotta** — 生成与 Terracotta 客户端和启动器（PCL2、HMCL 等）相同的 `U/XXXX-XXXX-XXXX-XXXX` 联机码
- **内嵌 EasyTier** — P2P 网络引擎在编译时打包，无需额外下载或配置
- **固定联机码** — 可选设置固定码，让玩家始终使用同一个地址连接
- **Scaffolding 协议** — 实现 Terracotta 的 Scaffolding 服务器协议，支持玩家追踪、Ping 和身份交换
- **纯 CLI** — 为无 GUI 的服务器环境设计
- **跨平台** — Linux (x86_64, aarch64)、Windows (x86_64)、macOS (x86_64, aarch64)

### 快速开始

1. 将 `terracotta-server` 可执行文件和 `config.toml` 放到 Minecraft 服务器同目录下（或任意位置）。

2. 根据需要编辑 `config.toml`：

   ```toml
   # Minecraft 服务端端口（默认：25565）
   port = 25565
   
   # 固定联机码（可选）
   # code = "U/ABCD-1234-EFGH-5678"
   
   # 额外中继服务器（可选）
   public_servers = []
   ```

3. 启动服务器：

   ```bash
   ./terracotta-server
   ```

4. 将显示的联机码分享给玩家，他们在 Terracotta 客户端中输入即可加入。

### 使用方法

```
terracotta-server [选项]

选项:
  -c, --config <路径>   配置文件路径 [默认: config.toml]
      --init            生成默认配置文件并退出
  -h, --help            显示帮助
  -V, --version         显示版本
```

生成默认配置文件：

```bash
./terracotta-server --init
```

使用自定义配置路径：

```bash
./terracotta-server --config /path/to/my-config.toml
```

### 配置说明

| 字段             | 类型       | 默认值  | 说明                                                                 |
| ---------------- | ---------- | ------- | ------------------------------------------------------------------- |
| `port`           | `u16`      | `25565` | 要通过 P2P 网络暴露的 Minecraft 服务端端口                           |
| `code`           | `string?`  | *无*    | 固定联机码（`U/XXXX-XXXX-XXXX-XXXX`）。未设置时每次启动随机生成      |
| `public_servers` | `string[]` | `[]`    | 除内置默认服务器外的额外中继服务器                                    |

### 从源码构建

**前置条件：** Rust 1.75+（stable）、联网环境（构建脚本会下载 EasyTier 二进制）。

```bash
cargo build --release
```

构建脚本（`build.rs`）会自动：
1. 下载适合目标平台的 EasyTier 发行版
2. 使用 LZMA2 + BCJ 滤波器重新压缩以减小体积
3. 通过 `include_bytes!()` 将压缩包嵌入可执行文件

生成的二进制文件完全自包含，无外部依赖。

### 工作原理

1. **启动** — 读取 `config.toml`，将内嵌的 EasyTier 释放到临时目录
2. **房间创建** — 使用 Terracotta 的 base-34 编码算法生成（或从配置加载）联机码
3. **Scaffolding 服务器** — 启动 TCP 服务器，实现 Terracotta 的 Scaffolding 协议用于玩家追踪
4. **EasyTier 启动** — 使用房间的网络凭证启动 P2P VPN，连接到公共中继服务器
5. **就绪** — 显示联机码；玩家在 Terracotta 客户端中输入即可加入
6. **关闭** — Ctrl+C 优雅停止 EasyTier 并清理临时文件

### 许可证

本项目使用 [GNU Affero General Public License v3.0 or later](LICENSE) 许可，与上游 [Terracotta](https://github.com/burningtnt/Terracotta) 项目保持一致。

[Terracotta](https://github.com/burningtnt/Terracotta) 的版权归其原作者所有。

---

## License

AGPL-3.0-or-later
