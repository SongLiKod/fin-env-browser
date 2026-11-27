use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};

use futures_util::{stream, StreamExt};
use tauri::Emitter;

use crate::{
    audit_log::{actions, Audit},
    error::{AppError, Result},
    models::{
        new_id, now_iso, Proxy, ProxyBatchCheckStart, ProxyBatchProgress, ProxyCheckResult,
        ProxyJobView,
    },
    proxy_check,
    state::SharedState,
};

pub struct ProxyJob {
    pub progress: Mutex<ProxyBatchProgress>,
    pub cancel: AtomicBool,
    pub results: Mutex<Vec<ProxyCheckResult>>,
    pub created_at: Instant,
}

pub struct ProxyJobManager {
    jobs: RwLock<HashMap<String, Arc<ProxyJob>>>,
    refreshing: Mutex<HashSet<String>>,
}

impl Default for ProxyJobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyJobManager {
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
            refreshing: Mutex::new(HashSet::new()),
        }
    }

    pub fn start_check(
        &self,
        state: SharedState,
        proxies: Vec<Proxy>,
        concurrency: u32,
        actor: String,
    ) -> Result<ProxyBatchCheckStart> {
        self.cleanup();
        let running = self
            .jobs
            .read()
            .map_err(|_| AppError::internal("代理任务锁已中毒"))?
            .values()
            .filter(|job| {
                job.progress
                    .lock()
                    .map(|progress| progress.state == "running")
                    .unwrap_or(false)
            })
            .count();
        if running >= 4 {
            return Err(AppError::conflict(
                "同时最多运行 4 个代理检测任务，请稍后再试",
            ));
        }
        let total = proxies.len() as u32;
        if total == 0 {
            return Err(AppError::bad_request("没有可检测的代理"));
        }
        let job_id = new_id();
        let started_at = now_iso();
        let job = Arc::new(ProxyJob {
            progress: Mutex::new(ProxyBatchProgress {
                job_id: job_id.clone(),
                kind: "check".into(),
                state: "running".into(),
                total,
                completed: 0,
                ok: 0,
                failed: 0,
                started_at,
                finished_at: None,
                message: format!("正在检测 0/{total}"),
                recent: Vec::new(),
            }),
            cancel: AtomicBool::new(false),
            results: Mutex::new(Vec::new()),
            created_at: Instant::now(),
        });
        self.jobs
            .write()
            .map_err(|_| AppError::internal("代理任务锁已中毒"))?
            .insert(job_id.clone(), job.clone());
        let concurrency = concurrency.clamp(1, 32) as usize;
        tauri::async_runtime::spawn(async move {
            run_check_job(state, job, proxies, concurrency, actor).await;
        });
        Ok(ProxyBatchCheckStart { job_id, total })
    }

    pub fn get(&self, job_id: &str) -> Result<ProxyJobView> {
        self.cleanup();
        let job = self
            .jobs
            .read()
            .map_err(|_| AppError::internal("代理任务锁已中毒"))?
            .get(job_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("检测任务不存在或已过期"))?;
        let progress = job
            .progress
            .lock()
            .map_err(|_| AppError::internal("代理任务锁已中毒"))?
            .clone();
        let results = job
            .results
            .lock()
            .map_err(|_| AppError::internal("代理任务锁已中毒"))?
            .clone();
        Ok(ProxyJobView { progress, results })
    }

    pub fn cancel(&self, job_id: &str) -> Result<()> {
        let job = self
            .jobs
            .read()
            .map_err(|_| AppError::internal("代理任务锁已中毒"))?
            .get(job_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("检测任务不存在或已过期"))?;
        job.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn begin_refresh(&self, subscription_id: &str) -> Result<()> {
        let mut refreshing = self
            .refreshing
            .lock()
            .map_err(|_| AppError::internal("订阅刷新锁已中毒"))?;
        if !refreshing.insert(subscription_id.to_string()) {
            return Err(AppError::conflict("该订阅正在刷新，请稍后再试"));
        }
        Ok(())
    }

    pub fn end_refresh(&self, subscription_id: &str) {
        if let Ok(mut refreshing) = self.refreshing.lock() {
            refreshing.remove(subscription_id);
        }
    }

    fn cleanup(&self) {
        if let Ok(mut jobs) = self.jobs.write() {
            jobs.retain(|_, job| {
                job.created_at.elapsed() < Duration::from_secs(1800)
                    || job
                        .progress
                        .lock()
                        .map(|progress| progress.state == "running")
                        .unwrap_or(false)
            });
        }
    }
}

async fn run_check_job(
    state: SharedState,
    job: Arc<ProxyJob>,
    proxies: Vec<Proxy>,
    concurrency: usize,
    actor: String,
) {
    let total = proxies.len() as u32;
    let mut pending = Vec::new();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut cancelled = false;

    let mut stream = stream::iter(proxies.into_iter().map(|proxy| {
        let state = state.clone();
        async move {
            let password = if proxy.password_enc.is_empty() {
                None
            } else {
                state.crypto.decrypt(&proxy.password_enc).ok()
            };
            proxy_check::check_proxy_core(&proxy, password.as_deref()).await
        }
    }))
    .buffer_unordered(concurrency);

    while let Some(result) = stream.next().await {
        {
            if let Ok(mut progress) = job.progress.lock() {
                progress.completed += 1;
                if result.ok {
                    progress.ok += 1;
                } else {
                    progress.failed += 1;
                }
                progress.message = format!("正在检测 {}/{}", progress.completed, total);
                progress.recent.push(result.clone());
                if progress.recent.len() > 50 {
                    let overflow = progress.recent.len() - 50;
                    progress.recent.drain(0..overflow);
                }
            }
            if let Ok(mut results) = job.results.lock() {
                results.push(result.clone());
            }
        }
        pending.push(result);
        if pending.len() >= 25 {
            if let Err(error) = state.db.save_proxy_checks(&pending) {
                tracing::warn!(error = %error.message(), "failed to persist proxy batch results");
            }
            pending.clear();
        }
        if last_emit.elapsed() >= Duration::from_millis(200) {
            emit_progress(&state, &job);
            last_emit = Instant::now();
        }
        if job.cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
    }

    if !pending.is_empty() {
        if let Err(error) = state.db.save_proxy_checks(&pending) {
            tracing::warn!(error = %error.message(), "failed to persist proxy batch results");
        }
    }

    let final_state = if cancelled { "cancelled" } else { "done" };
    let (ok, failed, completed) = {
        if let Ok(mut progress) = job.progress.lock() {
            progress.state = final_state.into();
            progress.finished_at = Some(now_iso());
            progress.message = if cancelled {
                format!("已取消，完成 {}/{}", progress.completed, total)
            } else {
                format!("检测完成：成功 {}，失败 {}", progress.ok, progress.failed)
            };
            (progress.ok, progress.failed, progress.completed)
        } else {
            (0, 0, 0)
        }
    };
    emit_progress(&state, &job);
    let _ = Audit::global(
        &state.db,
        &actor,
        actions::PROXY_BATCH_CHECK,
        format!(
            "批量检测代理：完成 {completed}/{total}，成功 {ok}，失败 {failed}，取消 {cancelled}"
        ),
    );
}

fn emit_progress(state: &SharedState, job: &ProxyJob) {
    let Some(app) = state.app_handle() else {
        return;
    };
    if let Ok(progress) = job.progress.lock() {
        let _ = app.emit("proxy://batch", progress.clone());
    }
}
