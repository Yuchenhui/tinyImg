pub mod preset;
pub mod storage;

use crate::config::preset::CompressionPreset;
use serde::{Deserialize, Serialize};

/// 全局应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 界面语言
    pub language: String,
    /// 主题
    pub theme: Theme,
    /// 输出目录策略
    pub output_dir: OutputDir,
    /// 是否覆盖原文件
    pub overwrite: bool,
    /// 输出文件后缀
    pub suffix: String,
    /// 默认预设名称
    pub default_preset: String,
    /// 用户自定义预设列表
    pub presets: Vec<CompressionPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputDir {
    /// 输出到原文件同目录
    SameAsInput,
    /// 输出到自定义目录
    Custom(String),
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            theme: Theme::System,
            output_dir: OutputDir::SameAsInput,
            overwrite: false,
            suffix: "_compressed".into(),
            default_preset: "web_optimized".into(),
            presets: Vec::new(),
        }
    }
}
