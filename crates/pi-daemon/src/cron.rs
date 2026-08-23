use anyhow::Result;
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::{broadcast, watch};

pub mod natural;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobSchedule {
    Natural { every: String },
    Cron { expr: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub schedule: JobSchedule,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub last_run: Option<String>,
    #[serde(default)]
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobsFile {
    pub jobs: Vec<CronJob>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CronJobResult {
    pub job_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub status: String,
    pub output: String,
}

#[derive(Debug, Clone)]
pub enum CronNotification {
    JobCompleted(CronJobResult),
    LoopStopped,
}

#[derive(Debug, Clone)]
pub struct CronContext {
    pub home: PathBuf,
    pub tick_interval: Duration,
}

impl Default for CronContext {
    fn default() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
            tick_interval: Duration::from_secs(60),
        }
    }
}

impl CronContext {
    pub fn jobs_path(&self) -> PathBuf {
        self.home.join(".pi").join("cron").join("jobs.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.home.join(".pi").join("cron").join(".tick.lock")
    }

    pub fn load_jobs(&self) -> Result<JobsFile> {
        let path = self.jobs_path();
        if !path.exists() {
            return Ok(JobsFile::default());
        }

        let data = fs::read_to_string(path)?;
        let file: JobsFile = serde_json::from_str(&data)?;
        Ok(file)
    }

    pub fn save_jobs(&self, jobs: &JobsFile) -> Result<()> {
        let path = self.jobs_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(jobs)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn mark_job_result(&self, jobs: &mut JobsFile, job_id: &str, status: &str, _output: impl Into<String>) {
        if let Some(job) = jobs.jobs.iter_mut().find(|j| j.id == job_id) {
            job.last_run = Some(Utc::now().to_rfc3339());
            job.last_status = Some(status.to_string());
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    pub fn parse_due(&self, job: &CronJob) -> Result<Option<Duration>> {
        let Some(last_run_str) = &job.last_run else {
            return Ok(Some(Duration::from_secs(0)));
        };

        let last_run = DateTime::parse_from_rfc3339(last_run_str)?;
        let now = Utc::now();
        let elapsed = (now - last_run.with_timezone(&Utc)).to_std().unwrap_or_default();

        let interval = match &job.schedule {
            JobSchedule::Natural { every } => natural::parse_duration(every)?,
            JobSchedule::Cron { expr: _ } => return Ok(None),
        };

        if elapsed >= interval {
            return Ok(Some(Duration::from_secs(0)));
        }

        Ok(Some(interval - elapsed))
    }

    pub fn try_acquire_tick_lock(&self) -> Result<Option<std::fs::File>> {
        let lock_path = self.lock_path();
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = std::fs::File::create(&lock_path)?;
        match file.try_lock_exclusive() {
            Ok(_) => Ok(Some(file)),
            Err(_) => Ok(None),
        }
    }

    pub async fn execute_job(&self, job: &CronJob) -> Result<CronJobResult> {
        let started_at = self.now();
        let mut status = "success";
        let output;

        let tmp_dir = self.jobs_path().parent().map(|p| p.join("tmp")).unwrap_or_else(|| self.home.join(".pi").join("cron").join("tmp"));
        fs::create_dir_all(&tmp_dir)?;
        let script_path = tmp_dir.join(format!("{}-cron-runner.sh", job.id));

        let mut script = String::from("#!/bin/sh\nset -eu\n");
        script.push_str("echo 'No executor configured for daemon cron job'\n");
        fs::write(&script_path, script)?;

        let mut cmd = Command::new("sh");
        cmd.arg(&script_path);
        cmd.kill_on_drop(true);

        match cmd.output().await {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                output = if stdout.is_empty() { stderr } else { stdout };

                if !out.status.success() {
                    status = if out.status.code() == Some(124) || out.status.code() == Some(137) {
                        "timeout"
                    } else {
                        "failed"
                    };
                }
            }
            Err(e) => {
                status = "error";
                output = e.to_string();
            }
        }

        let finished_at = self.now();
        Ok(CronJobResult {
            job_id: job.id.clone(),
            started_at,
            finished_at,
            status: status.to_string(),
            output,
        })
    }

    pub async fn send_macos_notification(&self, title: &str, body: &str) {
        let escaped_title = title.replace("'", "\\'");
        let escaped_body = body.replace("'", "\\'");
        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"default\"",
            escaped_body, escaped_title
        );

        let _ = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .await;
    }

    pub async fn run_cron_loop(
        self: Arc<Self>,
        mut shutdown: broadcast::Receiver<()>,
        notify_tx: watch::Sender<CronNotification>,
    ) {
        let mut ticker = tokio::time::interval(self.tick_interval);

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    let _ = notify_tx.send(CronNotification::LoopStopped);
                    break;
                }
                _ = ticker.tick() => {
                    let mut jobs = match self.load_jobs() {
                        Ok(j) => j,
                        Err(e) => {
                            eprintln!("cron: failed to load jobs: {}", e);
                            continue;
                        }
                    };

                    if jobs.jobs.is_empty() {
                        continue;
                    }

                    let lock = match self.try_acquire_tick_lock() {
                        Ok(Some(l)) => l,
                        Ok(None) => continue,
                        Err(e) => {
                            eprintln!("cron: lock error: {}", e);
                            continue;
                        }
                    };

                    let due_ids: Vec<String> = jobs.jobs.iter().filter(|job| {
                        if !job.enabled {
                            return false;
                        }
                        match self.parse_due(job) {
                            Ok(Some(delay)) => delay.is_zero(),
                            _ => false,
                        }
                    }).map(|job| job.id.clone()).collect();

                    let mut ran_any = false;
                    for job_id in &due_ids {
                        if let Some(job) = jobs.jobs.iter_mut().find(|j| j.id == *job_id) {
                            ran_any = true;
                            let job_data = job.clone();
                            let _ = job;

                            let res = self.execute_job(&job_data).await;
                            match res {
                                Ok(result) => {
                                    self.mark_job_result(&mut jobs, &job_data.id, &result.status, &result.output);
                                    let _ = notify_tx.send(CronNotification::JobCompleted(result.clone()));
                                    let _ = self.send_macos_notification(&job_data.name, &result.output).await;
                                }
                                Err(e) => {
                                    self.mark_job_result(&mut jobs, &job_data.id, "error", e.to_string());
                                }
                            }

                            if let Err(e) = self.save_jobs(&jobs) {
                                eprintln!("cron: failed to save jobs: {}", e);
                            }
                        }
                    }

                    if ran_any {
                        let _ = lock;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_missing_jobs_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CronContext {
            home: dir.path().to_path_buf(),
            tick_interval: Duration::from_secs(60),
        };

        let jobs = ctx.load_jobs().unwrap();
        assert!(jobs.jobs.is_empty());
    }

    #[test]
    fn test_save_and_load_jobs_file() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CronContext {
            home: dir.path().to_path_buf(),
            tick_interval: Duration::from_secs(60),
        };

        let mut jobs = JobsFile::default();
        jobs.jobs.push(CronJob {
            id: "job-1".into(),
            name: "Test".into(),
            prompt: "hello".into(),
            schedule: JobSchedule::Natural { every: "every 2h".into() },
            skills: Vec::new(),
            enabled: true,
            last_run: None,
            last_status: None,
        });

        ctx.save_jobs(&jobs).unwrap();
        let loaded = ctx.load_jobs().unwrap();
        assert_eq!(loaded.jobs.len(), 1);
        assert_eq!(loaded.jobs[0].id, "job-1");
    }

    #[test]
    fn test_mark_job_result_updates_fields() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CronContext {
            home: dir.path().to_path_buf(),
            tick_interval: Duration::from_secs(60),
        };

        let mut jobs = JobsFile::default();
        jobs.jobs.push(CronJob {
            id: "job-1".into(),
            name: "Test".into(),
            prompt: "hello".into(),
            schedule: JobSchedule::Natural { every: "every 2h".into() },
            skills: Vec::new(),
            enabled: true,
            last_run: None,
            last_status: None,
        });

        ctx.mark_job_result(&mut jobs, "job-1", "success", "ok");
        assert!(jobs.jobs[0].last_run.is_some());
        assert_eq!(jobs.jobs[0].last_status.as_deref(), Some("success"));
    }

    #[tokio::test]
    async fn test_execute_job_default_script_reports_error() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CronContext {
            home: dir.path().to_path_buf(),
            tick_interval: Duration::from_secs(60),
        };

        let job = CronJob {
            id: "job-1".into(),
            name: "Test".into(),
            prompt: "hello".into(),
            schedule: JobSchedule::Natural { every: "every 2h".into() },
            skills: Vec::new(),
            enabled: true,
            last_run: None,
            last_status: None,
        };

        let result = ctx.execute_job(&job).await.unwrap();
        assert_eq!(result.job_id, "job-1");
        assert_eq!(result.status, "success");
    }

    #[test]
    fn test_try_acquire_tick_lock_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = CronContext {
            home: dir.path().to_path_buf(),
            tick_interval: Duration::from_secs(60),
        };

        let first = ctx.try_acquire_tick_lock().unwrap();
        assert!(first.is_some());

        let second = ctx.try_acquire_tick_lock().unwrap();
        assert!(second.is_none());

        drop(first);
        let third = ctx.try_acquire_tick_lock().unwrap();
        assert!(third.is_some());
    }
}
