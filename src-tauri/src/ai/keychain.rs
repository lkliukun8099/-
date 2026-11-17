//! API Key 钥匙串管理与版本化轮转（隐私要求 7：密钥不出 Rust 进程、永不回传）。
//!
//! 存储结构（keyring 条目名）：
//! - `maiduo/{alias}`      → 指针，值为当前版本号（如 "v2"）
//! - `maiduo/{alias}/v{n}` → 第 n 版密钥本体
//!
//! 轮转语义（架构 §4.2）：写入新版本并更新指针；旧版本在下次成功调用后删除
//! （本实现简化为轮转成功后即尝试删除旧版本，失败仅记日志不影响流程）。

use crate::error::AppError;

const SERVICE: &str = "maiduo-cuotiben";

/// 钥匙串访问封装。
pub struct Keychain {
    service: &'static str,
}

impl Default for Keychain {
    fn default() -> Self {
        Self::new()
    }
}

impl Keychain {
    pub fn new() -> Self {
        Self { service: SERVICE }
    }

    fn entry(&self, name: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(self.service, name)
            .map_err(|e| AppError::keychain(format!("钥匙串条目创建失败: {e}")))
    }

    fn pointer_name(alias: &str) -> String {
        format!("maiduo/{alias}")
    }

    fn version_name(alias: &str, version: u32) -> String {
        format!("maiduo/{alias}/v{version}")
    }

    /// 读取当前密钥版本号（无则 0）。
    fn current_version(&self, alias: &str) -> u32 {
        self.entry(&Self::pointer_name(alias))
            .and_then(|e| e.get_password().map_err(AppError::from_keyring))
            .ok()
            .and_then(|v| v.trim_start_matches('v').parse::<u32>().ok())
            .unwrap_or(0)
    }

    /// 读取当前生效密钥（不存在返回 None）。
    pub fn get_key(&self, alias: &str) -> Result<Option<String>, AppError> {
        let version = self.current_version(alias);
        if version == 0 {
            return Ok(None);
        }
        match self
            .entry(&Self::version_name(alias, version))?
            .get_password()
        {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::keychain(format!("密钥读取失败: {e}"))),
        }
    }

    /// 是否已配置密钥（供前端展示，不返回密钥本体）。
    pub fn has_key(&self, alias: &str) -> bool {
        self.get_key(alias).map(|k| k.is_some()).unwrap_or(false)
    }

    /// 写入/轮转密钥：版本号 +1，旧版本尝试删除。
    /// 返回新版本号。
    pub fn store_key(&self, alias: &str, key: &str) -> Result<u32, AppError> {
        if key.trim().is_empty() {
            return Err(AppError::invalid_param("API Key 不能为空"));
        }
        let old_version = self.current_version(alias);
        let new_version = old_version + 1;

        // 1. 写新版本
        self.entry(&Self::version_name(alias, new_version))?
            .set_password(key)
            .map_err(|e| AppError::keychain(format!("密钥写入失败: {e}")))?;
        // 2. 更新指针
        self.entry(&Self::pointer_name(alias))?
            .set_password(&format!("v{new_version}"))
            .map_err(|e| AppError::keychain(format!("密钥指针更新失败: {e}")))?;
        // 3. 尝试删除旧版本（失败仅告警）
        if old_version > 0 {
            if let Ok(old) = self.entry(&Self::version_name(alias, old_version)) {
                if let Err(e) = old.delete_credential() {
                    tracing::warn!("旧版本密钥删除失败（alias={alias}）: {e}");
                }
            }
        }
        tracing::info!("密钥已轮转: alias={alias} v{old_version} -> v{new_version}");
        Ok(new_version)
    }

    /// 删除某别名的全部密钥版本。
    pub fn delete_key(&self, alias: &str) -> Result<(), AppError> {
        let version = self.current_version(alias);
        for v in 1..=version {
            if let Ok(entry) = self.entry(&Self::version_name(alias, v)) {
                let _ = entry.delete_credential();
            }
        }
        if let Ok(pointer) = self.entry(&Self::pointer_name(alias)) {
            let _ = pointer.delete_credential();
        }
        Ok(())
    }
}

impl AppError {
    /// keyring 错误转换辅助（内部使用）。
    fn from_keyring(e: keyring::Error) -> Self {
        AppError::keychain(e.to_string())
    }
}
