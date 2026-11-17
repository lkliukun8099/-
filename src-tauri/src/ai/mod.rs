//! AI 统一接入层 Gateway（架构 §4.2）。
//!
//! 全部云端 AI 调用的唯一出口（隐私审计单点，AC-9）。
//! 职责：路由（≤100ms）→ 熔断检查 → 调用（超时控制）→ 失败沿链降级
//! （≤1s 切换）→ 成本计量（call_log 脱敏落库）。

pub mod adapter;
pub mod circuit_breaker;
pub mod keychain;
pub mod meter;
pub mod providers;
pub mod router;
pub mod stream;

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use futures::StreamExt;

use adapter::{ChatRequest, ChatResponse, ProviderAdapter, StreamChunk, Usage};
use circuit_breaker::CircuitBreaker;
use keychain::Keychain;
use meter::Meter;
use router::Router;

use crate::db::DbPool;
use crate::error::AppError;

/// AI Gateway 主结构。
pub struct Gateway {
    db: DbPool,
    http: reqwest::Client,
    router: Router,
    meter: Meter,
    keychain: Keychain,
    /// 每个 provider.model 一个熔断器
    breakers: DashMap<String, Arc<CircuitBreaker>>,
}

impl Gateway {
    pub fn new(db: DbPool) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("HTTP 客户端构建失败");
        Self {
            router: Router::new(db.clone()),
            meter: Meter::new(db.clone()),
            db,
            http,
            keychain: Keychain::new(),
            breakers: DashMap::new(),
        }
    }

    /// 钥匙串访问（命令层做密钥管理用）。
    pub fn keychain(&self) -> &Keychain {
        &self.keychain
    }

    /// 按 provider 配置构造适配器。
    fn adapter_for(
        &self,
        provider: &crate::db::dao::model::ModelProvider,
        api_key: String,
    ) -> Result<Box<dyn ProviderAdapter>, AppError> {
        match provider.protocol.as_str() {
            "hunyuan_tc3" => Ok(Box::new(providers::hunyuan::HunyuanAdapter::new(
                self.http.clone(),
                provider.name.clone(),
                api_key,
                provider.models.clone(),
            )?)),
            // openai_compat 为基线协议，其余未知协议按兼容处理
            _ => Ok(Box::new(providers::openai_compat::OpenAiCompatAdapter::new(
                self.http.clone(),
                provider.name.clone(),
                provider.base_url.clone(),
                api_key,
                provider.models.clone(),
            ))),
        }
    }

    /// 取某调用目标的熔断器（不存在则创建）。
    fn breaker(&self, full_name: &str) -> Arc<CircuitBreaker> {
        self.breakers
            .entry(full_name.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new()))
            .clone()
    }

    /// 探测外网连通性（在线状态信号之一，§9-8）。
    pub async fn probe_online(&self) -> bool {
        self.http
            .head("https://www.baidu.com")
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// 非流式统一聊天（归因/校验管道用）。
    ///
    /// 沿路由链尝试：熔断 Open 直接跳过；失败记录并降级下一目标。
    pub async fn chat_once(
        &self,
        task_type: &str,
        mut req: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        let targets = self.router.resolve(task_type)?;
        let req_hash = Meter::request_hash(
            &req.messages.iter().map(|m| m.content.as_str()).collect::<String>(),
        );
        let mut last_err: Option<AppError> = None;

        for target in &targets {
            let breaker = self.breaker(&target.full_name);
            if !breaker.allow() {
                self.meter.record(
                    &target.provider.name,
                    &target.model,
                    task_type,
                    0,
                    0,
                    0.0,
                    None,
                    "circuit_open",
                    Some("CIRCUIT_OPEN"),
                    Some(&req_hash),
                );
                continue;
            }

            let api_key = match self.keychain.get_key(&target.provider.key_alias)? {
                Some(k) => k,
                None => {
                    tracing::warn!("provider {} 未配置 API Key，跳过", target.provider.name);
                    self.meter.record(
                        &target.provider.name,
                        &target.model,
                        task_type,
                        0,
                        0,
                        0.0,
                        None,
                        "error",
                        Some("NO_KEY"),
                        Some(&req_hash),
                    );
                    continue;
                }
            };

            let adapter = self.adapter_for(&target.provider, api_key)?;
            req.model = target.model.clone();
            let start = Instant::now();
            let timeout = Duration::from_millis(target.timeout_ms.max(1000) as u64);

            match tokio::time::timeout(timeout, adapter.chat_once(req.clone())).await {
                Ok(Ok(mut resp)) => {
                    breaker.on_success();
                    let latency = start.elapsed().as_millis() as i64;
                    let cost = Meter::cost_cny(
                        adapter.pricing(&target.model),
                        resp.tokens_in,
                        resp.tokens_out,
                    );
                    self.meter.record(
                        &target.provider.name,
                        &target.model,
                        task_type,
                        resp.tokens_in,
                        resp.tokens_out,
                        cost,
                        Some(latency),
                        "ok",
                        None,
                        Some(&req_hash),
                    );
                    resp.served_by = target.full_name.clone();
                    return Ok(resp);
                }
                Ok(Err(e)) => {
                    breaker.on_failure();
                    self.meter.record(
                        &target.provider.name,
                        &target.model,
                        task_type,
                        0,
                        0,
                        0.0,
                        Some(start.elapsed().as_millis() as i64),
                        "error",
                        Some(&e.code.to_string()),
                        Some(&req_hash),
                    );
                    tracing::warn!("{} 调用失败，降级下一目标: {e}", target.full_name);
                    last_err = Some(e);
                }
                Err(_) => {
                    breaker.on_failure();
                    self.meter.record(
                        &target.provider.name,
                        &target.model,
                        task_type,
                        0,
                        0,
                        0.0,
                        Some(target.timeout_ms),
                        "timeout",
                        Some("TIMEOUT"),
                        Some(&req_hash),
                    );
                    tracing::warn!("{} 调用超时（{}ms），降级下一目标", target.full_name, target.timeout_ms);
                    last_err = Some(AppError::ai_call(format!("{} 调用超时", target.full_name)));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AppError::ai_unavailable("全部候选模型不可用（熔断或密钥缺失）")
        }))
    }

    /// 流式统一聊天：沿链建立流后逐块推入 Tauri Channel。
    ///
    /// 降级时后续分块带 `degraded` 标记（原目标名）。
    pub async fn chat_stream(
        &self,
        task_type: &str,
        mut req: ChatRequest,
        channel: tauri::ipc::Channel<StreamChunk>,
    ) -> Result<(), AppError> {
        let targets = self.router.resolve(task_type)?;
        let req_text: String = req.messages.iter().map(|m| m.content.as_str()).collect();
        let req_hash = Meter::request_hash(&req_text);
        let primary_name = targets.first().map(|t| t.full_name.clone());
        let mut last_err: Option<AppError> = None;

        for target in &targets {
            let breaker = self.breaker(&target.full_name);
            if !breaker.allow() {
                self.meter.record(
                    &target.provider.name,
                    &target.model,
                    task_type,
                    0,
                    0,
                    0.0,
                    None,
                    "circuit_open",
                    Some("CIRCUIT_OPEN"),
                    Some(&req_hash),
                );
                continue;
            }

            let api_key = match self.keychain.get_key(&target.provider.key_alias)? {
                Some(k) => k,
                None => {
                    self.meter.record(
                        &target.provider.name,
                        &target.model,
                        task_type,
                        0,
                        0,
                        0.0,
                        None,
                        "error",
                        Some("NO_KEY"),
                        Some(&req_hash),
                    );
                    continue;
                }
            };

            let adapter = self.adapter_for(&target.provider, api_key)?;
            req.model = target.model.clone();
            let start = Instant::now();
            let timeout = Duration::from_millis(target.timeout_ms.max(1000) as u64);

            // 建立流（建连阶段失败可降级）
            let stream_result =
                tokio::time::timeout(timeout, adapter.chat_stream(req.clone())).await;
            let mut chat_stream = match stream_result {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    breaker.on_failure();
                    self.meter.record(
                        &target.provider.name,
                        &target.model,
                        task_type,
                        0,
                        0,
                        0.0,
                        Some(start.elapsed().as_millis() as i64),
                        "error",
                        Some(&e.code.to_string()),
                        Some(&req_hash),
                    );
                    last_err = Some(e);
                    continue;
                }
                Err(_) => {
                    breaker.on_failure();
                    self.meter.record(
                        &target.provider.name,
                        &target.model,
                        task_type,
                        0,
                        0,
                        0.0,
                        Some(target.timeout_ms),
                        "timeout",
                        Some("TIMEOUT"),
                        Some(&req_hash),
                    );
                    last_err = Some(AppError::ai_call(format!("{} 调用超时", target.full_name)));
                    continue;
                }
            };

            // 流式转发（降级标记：非主力目标）
            let degraded = if Some(&target.full_name) != primary_name.as_ref() {
                primary_name.clone()
            } else {
                None
            };
            let mut out_text = String::new();
            let mut final_usage: Option<Usage> = None;
            let mut stream_failed = false;

            while let Some(item) = chat_stream.next().await {
                match item {
                    Ok(mut chunk) => {
                        if chunk.done {
                            final_usage = chunk.usage;
                        } else {
                            out_text.push_str(&chunk.delta);
                        }
                        chunk.degraded = degraded.clone();
                        if channel.send(chunk).is_err() {
                            // 前端已断开（页面关闭），终止推送
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("流式读取中断: {e}");
                        stream_failed = true;
                        break;
                    }
                }
            }

            let latency = start.elapsed().as_millis() as i64;
            if stream_failed {
                breaker.on_failure();
                self.meter.record(
                    &target.provider.name,
                    &target.model,
                    task_type,
                    0,
                    0,
                    0.0,
                    Some(latency),
                    "error",
                    Some("STREAM_BROKEN"),
                    Some(&req_hash),
                );
                let _ = channel.send(StreamChunk::done(None, degraded));
                return Err(AppError::ai_call("流式响应中断"));
            }

            breaker.on_success();
            // usage 缺失时本地估算（AC-7）
            let usage = final_usage.unwrap_or_else(|| Usage {
                tokens_in: adapter.estimate_tokens(&req_text),
                tokens_out: adapter.estimate_tokens(&out_text),
            });
            let cost = Meter::cost_cny(
                adapter.pricing(&target.model),
                usage.tokens_in,
                usage.tokens_out,
            );
            self.meter.record(
                &target.provider.name,
                &target.model,
                task_type,
                usage.tokens_in,
                usage.tokens_out,
                cost,
                Some(latency),
                "ok",
                None,
                Some(&req_hash),
            );
            return Ok(());
        }

        Err(last_err.unwrap_or_else(|| {
            AppError::ai_unavailable("全部候选模型不可用（熔断或密钥缺失）")
        }))
    }

    /// 连通性测试（模型设置页）：一次最小化非流式调用。
    pub async fn test_provider(
        &self,
        provider: &crate::db::dao::model::ModelProvider,
    ) -> Result<String, AppError> {
        let api_key = self
            .keychain
            .get_key(&provider.key_alias)?
            .ok_or_else(|| AppError::keychain("该供应商未配置 API Key"))?;
        let adapter = self.adapter_for(provider, api_key)?;
        let model = provider
            .models
            .first()
            .map(|m| m.id.clone())
            .ok_or_else(|| AppError::invalid_param("供应商未配置任何模型"))?;
        let start = Instant::now();
        let req = ChatRequest {
            model: model.clone(),
            messages: vec![adapter::ChatMessage::user("ping，请回复 pong")],
            temperature: Some(0.0),
            max_tokens: Some(8),
        };
        let resp = match tokio::time::timeout(Duration::from_secs(15), adapter.chat_once(req)).await
        {
            Ok(r) => r?,
            Err(_) => return Err(AppError::ai_call("连通性测试超时（15s）")),
        };
        let latency = start.elapsed().as_millis();
        Ok(format!(
            "连通正常（{latency}ms，模型 {model} 响应 {} 字符）",
            resp.content.chars().count()
        ))
    }
}
