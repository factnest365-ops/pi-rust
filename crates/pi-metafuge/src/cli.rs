use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::agent::AgentKind;
use crate::engine::{self, ForgeOptions, RunReport};
use crate::run::RunStatus;

/// One-shot forge: a prompt in, a compiled, tested, director-gated repository out.
#[derive(Debug, Parser)]
#[command(
    name = "metafuge",
    version,
    about = "Forge a repository from one prompt: spec, scaffold, forge, destroy, crucible, director, seal.",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Forge a new repository from a single prompt.
    Forge(ForgeArgs),
    /// Resume an interrupted run from its recorded stage.
    Resume(ResumeArgs),
}

#[derive(Debug, Args)]
pub struct ForgeArgs {
    /// The idea, stated once.
    #[arg(long, value_name = "TEXT")]
    pub prompt: String,
    /// Target directory; must be new or empty.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,
    /// Target stack. This build forges `rust` only.
    #[arg(long, default_value = engine::SUPPORTED_STACK, value_name = "STACK")]
    pub stack: String,
    /// Total build rounds before the run fails. Round 1 is the first attempt.
    #[arg(long, default_value_t = engine::DEFAULT_MAX_ROUNDS, value_name = "N")]
    pub max_rounds: u32,
    /// Which agent writes the code.
    #[arg(long, value_enum, default_value = "template")]
    pub agent: AgentArg,
    /// Command for `--agent subprocess`: one JSON request on stdin, one JSON response on stdout.
    #[arg(long, value_name = "CMD")]
    pub agent_cmd: Option<String>,
    /// Seal an approved build with a local git commit. Never pushes or tags.
    #[arg(long)]
    pub seal: bool,
    /// Emit one JSON object on stdout instead of human progress.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ResumeArgs {
    /// Directory that holds `.metafuge/run.json`.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,
    /// Emit one JSON object on stdout instead of human progress.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AgentArg {
    Template,
    Subprocess,
}

impl From<AgentArg> for AgentKind {
    fn from(value: AgentArg) -> Self {
        match value {
            AgentArg::Template => AgentKind::Template,
            AgentArg::Subprocess => AgentKind::Subprocess,
        }
    }
}

pub async fn run() -> ExitCode {
    let cli = Cli::parse();
    let (json, out) = match &cli.command {
        Command::Forge(args) => (args.json, args.out.clone()),
        Command::Resume(args) => (args.json, args.out.clone()),
    };
    match execute(cli, json).await {
        Ok(report) => {
            print_report(&report, json);
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_failure(&error, json, &out);
            ExitCode::FAILURE
        }
    }
}

async fn execute(cli: Cli, quiet: bool) -> Result<RunReport> {
    match cli.command {
        Command::Forge(args) => {
            let options = ForgeOptions {
                prompt: args.prompt,
                out_dir: args.out,
                stack: args.stack,
                max_rounds: args.max_rounds,
                agent: args.agent.into(),
                agent_cmd: args.agent_cmd,
                seal: args.seal,
            };
            engine::forge(options, quiet).await
        }
        Command::Resume(args) => engine::resume(&args.out, quiet).await,
    }
}

fn print_report(report: &RunReport, json: bool) {
    if json {
        match serde_json::to_string_pretty(report) {
            Ok(body) => println!("{body}"),
            Err(error) => eprintln!("metafuge: could not encode the report: {error}"),
        }
        return;
    }
    let status = match report.status {
        RunStatus::Running => "incomplete",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
    };
    println!("metafuge: {status} in {} round(s)", report.rounds);
    println!("agent: {}", report.agent.note);
    println!(
        "tasks: {}/{} complete",
        report.completed_tasks(),
        report.total_tasks()
    );
    match &report.verdict {
        Some(verdict) => println!(
            "verdict: {} ({})",
            verdict.status.status_line(),
            verdict.judged_by.as_str()
        ),
        None => println!("verdict: none"),
    }
    println!("out: {}", report.out_dir.display());
    println!("telemetry: {}", report.telemetry_dir.display());
    match &report.sealed_commit {
        Some(commit) => println!("seal: {commit} (local only, nothing pushed)"),
        None => println!("seal: not requested"),
    }
}

fn print_failure(error: &anyhow::Error, json: bool, out: &std::path::Path) {
    if json {
        let payload = serde_json::json!({
            "schema": engine::REPORT_SCHEMA,
            "ok": false,
            "status": "failed",
            "error": format!("{error:#}"),
            "out": out,
            "telemetry_dir": out.join(".metafuge").join("telemetry"),
        });
        let body = serde_json::to_string_pretty(&payload)
            .unwrap_or_else(|_| "{\"schema\":1,\"ok\":false}".to_string());
        println!("{body}");
    } else {
        eprintln!("metafuge: {error:#}");
    }
}
