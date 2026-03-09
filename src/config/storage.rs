use crate::config::AppConfig;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// 配置文件存储管理
pub struct ConfigStorage;

impl ConfigStorage {
    /// 获取配置目录路径
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Unable to determine config directory")?
            .join("tinyimg");
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create config dir: {}", dir.display()))?;
        }
        Ok(dir)
    }

    /// 获取配置文件路径
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// 加载配置（不存在则返回默认值）
    pub fn load() -> Result<AppConfig> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(AppConfig::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let config: AppConfig =
            toml::from_str(&content).context("Failed to parse config.toml")?;
        Ok(config)
    }

    /// 保存配置
    pub fn save(config: &AppConfig) -> Result<()> {
        let path = Self::config_path()?;
        let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config: {}", path.display()))?;
        Ok(())
    }
}
