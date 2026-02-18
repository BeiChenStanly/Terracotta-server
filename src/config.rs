use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Configuration for terracotta-server
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Minecraft server port to expose (default: 25565)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Fixed connection code (optional).
    /// If set, the same code is used every startup.
    /// Format: U/XXXX-XXXX-XXXX-XXXX
    pub code: Option<String>,

    /// Additional public relay servers
    #[serde(default)]
    pub public_servers: Vec<String>,
}

fn default_port() -> u16 {
    25565
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("无法读取配置文件 {:?}: {}", path, e))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| format!("配置文件格式错误: {}", e))?;

        if config.port == 0 {
            return Err("端口号不能为 0".to_string());
        }

        Ok(config)
    }

    /// Create a default config and write it to disk
    pub fn create_default(path: &Path) -> Result<(), String> {
        let default_content = r#"# Terracotta Server 配置文件
# Terracotta Server Configuration

# Minecraft 服务端端口 / Minecraft server port
port = 25565

# 固定联机码 (可选) / Fixed connection code (optional)
# 如果设置此项，每次启动都使用相同的联机码。
# If set, the same code is used on every startup.
# Format: U/XXXX-XXXX-XXXX-XXXX
# code = "U/ABCD-1234-EFGH-5678"

# 额外公共中继服务器 / Additional public relay servers
public_servers = []
"#;
        fs::write(path, default_content)
            .map_err(|e| format!("无法创建默认配置文件: {}", e))?;
        Ok(())
    }
}
