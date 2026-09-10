use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use doco::{Project, init, lifecycle};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about,
    long_about = "File-backed change management. Checks are mechanical, not semantic acceptance."
)]
struct Cli {
    /// Explicit project root (defaults to the current directory; no ancestor search)
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Agent {
    Codex,
    Claude,
    Pi,
}

#[derive(Subcommand)]
enum Command {
    /// Install project-local agent workflows without overwriting project facts
    Init {
        #[arg(long, value_enum)]
        agent: Vec<Agent>,
        #[arg(long)]
        dry_run: bool,
        /// Replace only recognized doco-managed integration content
        #[arg(long)]
        refresh: bool,
    },
    /// Create skeletons; an agent must supply the actual design
    New { id: String },
    /// List changes, deriving state from directories
    List,
    /// Print current context paths; historical changes require --history
    Context {
        id: String,
        #[arg(long)]
        history: bool,
    },
    /// Check structure, sections, task IDs, dependencies and template residue
    Check { id: String },
    /// Move an active work package after mechanical completion checks
    Complete { id: String },
    /// Delete work/ and move a completed change; previews before confirmation
    Archive {
        id: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Return a completed work package to active; review/reset tasks yourself
    Reopen { id: String },
    /// Record cancellation and delete work/; does not roll back code
    Cancel {
        id: String,
        /// Why this goal is being cancelled
        #[arg(long)]
        reason: String,
        /// How implemented code is retained, reverted, or handed off (or none)
        #[arg(long)]
        disposition: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let project = Project::open(&cli.root)?;
    match cli.command {
        Command::Init {
            mut agent,
            dry_run,
            refresh,
        } => {
            println!("Project root: {}", project.root().display());
            if agent.is_empty() {
                if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                    bail!(
                        "non-interactive init requires --agent codex|claude|pi; no files written"
                    );
                }
                print!("Select one or more agents (codex claude pi), separated by spaces: ");
                io::stdout().flush()?;
                let mut line = String::new();
                io::stdin().read_line(&mut line)?;
                for id in line.split_whitespace() {
                    agent.push(Agent::from_str(id, false).map_err(anyhow::Error::msg)?);
                }
                if agent.is_empty() {
                    bail!("select at least one agent");
                }
            }
            agent.sort();
            agent.dedup();
            let ids: Vec<&str> = agent
                .iter()
                .map(|a| match a {
                    Agent::Codex => "codex",
                    Agent::Claude => "claude",
                    Agent::Pi => "pi",
                })
                .collect();
            init::run(&project, &ids, dry_run, refresh)
        }
        Command::New { id } => lifecycle::new_change(&project, &id),
        Command::List => {
            for change in project.changes()? {
                println!("{}\t{}", change.state, change.id);
            }
            Ok(())
        }
        Command::Context { id, history } => lifecycle::context(&project, &id, history),
        Command::Check { id } => lifecycle::check(&project, &id),
        Command::Complete { id } => lifecycle::complete(&project, &id),
        Command::Reopen { id } => lifecycle::reopen(&project, &id),
        Command::Archive { id, yes, dry_run } => {
            lifecycle::archive(&project, &id, yes, dry_run, None)
        }
        Command::Cancel {
            id,
            reason,
            disposition,
            yes,
            dry_run,
        } => lifecycle::archive(&project, &id, yes, dry_run, Some((&reason, &disposition))),
    }
}
