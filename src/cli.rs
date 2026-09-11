use crate::terminal::{
    ColorMode, InteractionMode, PromptOutcome, TerminalCapabilities, TerminalPrompter,
    TerminalReporter,
};
use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};
use doco::{
    Change, Project, State, init, lifecycle,
    ui::{self, Event, Reporter, Tone},
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about,
    long_about = "File-backed change management. Checks are mechanical, not semantic acceptance.",
    color = clap::ColorChoice::Never
)]
struct Cli {
    /// Explicit project root (defaults to the current directory; no ancestor search)
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    /// Business-output color policy
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,
    /// Allow prompts that supply a missing change ID
    #[arg(long, global = true, conflicts_with = "no_interactive")]
    interactive: bool,
    /// Disable all prompts; destructive commands then require --yes
    #[arg(long, global = true)]
    no_interactive: bool,
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
    /// Create a tracked change skeleton; implementation history belongs in Git/PRs
    New {
        id: String,
        /// Create only proposal.md for a bounded change with no separate design/tasks
        #[arg(long)]
        proposal_only: bool,
    },
    /// List active changes, optionally including historical states
    List {
        /// Include completed changes
        #[arg(long)]
        completed: bool,
        /// Include archived changes
        #[arg(long)]
        archived: bool,
    },
    /// Print current context paths; historical changes require --history
    Context {
        id: Option<String>,
        #[arg(long)]
        history: bool,
    },
    /// Check structure, sections, task IDs, dependencies and template residue
    Check { id: Option<String> },
    /// Move an active work package after mechanical completion checks
    Complete { id: Option<String> },
    /// Delete work/ and move a completed change; use --dry-run for preview only
    Archive {
        id: Option<String>,
        /// Compatibility flag; confirmation is no longer required
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Return a completed work package to active; review/reset tasks yourself
    Reopen { id: Option<String> },
    /// Record cancellation and delete work/; does not roll back code
    Cancel {
        id: Option<String>,
        /// Why this goal is being cancelled
        #[arg(long)]
        reason: String,
        /// How implemented code is retained, reverted, or handed off (or none)
        #[arg(long)]
        disposition: String,
        /// Compatibility flag; confirmation is no longer required
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy)]
enum TargetKind {
    Context { history: bool },
    Check,
    Active,
    Completed,
}

impl TargetKind {
    fn accepts(self, change: &Change) -> bool {
        match self {
            Self::Context { history: true } | Self::Check => true,
            Self::Context { history: false } | Self::Active => change.state == State::Active,
            Self::Completed => change.state == State::Completed,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Context { history: true } | Self::Check => "any lifecycle state",
            Self::Context { history: false } | Self::Active => "active",
            Self::Completed => "completed",
        }
    }
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    if let Err(error) = cli.validate_targets() {
        error.exit();
    }
    let capabilities = TerminalCapabilities::detect();
    let mut reporter = TerminalReporter::stdio(cli.color, &capabilities);
    match execute(cli, &capabilities, &mut reporter) {
        Ok(()) => 0,
        Err(error) if error.downcast_ref::<ui::UserCancelled>().is_some() => {
            let _ = reporter.emit(Event::Line {
                stream: doco::ui::Stream::Stderr,
                tone: Tone::Warning,
                label: "CANCELLED",
                body: "No operation performed.",
            });
            130
        }
        Err(error) => {
            let body = format!("{error:#}");
            let _ = reporter.emit(Event::Line {
                stream: doco::ui::Stream::Stderr,
                tone: Tone::Danger,
                label: "error:",
                body: &body,
            });
            1
        }
    }
}

fn execute(
    cli: Cli,
    capabilities: &TerminalCapabilities,
    mut reporter: &mut TerminalReporter,
) -> Result<()> {
    let interaction = if cli.no_interactive {
        InteractionMode::Never
    } else if cli.interactive {
        InteractionMode::Always
    } else {
        InteractionMode::Auto
    };
    let project = Project::open(&cli.root)?;
    let terminal_prompter = TerminalPrompter::new(cli.color, capabilities);
    let can_prompt = interaction != InteractionMode::Never && capabilities.interactive();

    match cli.command {
        Command::Init {
            mut agent,
            dry_run,
            refresh,
        } => {
            ui::line(
                &mut reporter,
                Tone::Info,
                "Project root:",
                &project.root().display().to_string(),
            )?;
            if agent.is_empty() {
                if !can_prompt {
                    bail!(
                        "non-interactive init requires --agent codex|claude|pi; no files written"
                    );
                }
                agent = match terminal_prompter.select_agents()? {
                    PromptOutcome::Answer(indices) => indices
                        .into_iter()
                        .map(|index| [Agent::Codex, Agent::Claude, Agent::Pi][index])
                        .collect(),
                    PromptOutcome::Cancelled => return ui::cancelled(),
                };
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
            init::run_with_ui(&project, &ids, dry_run, refresh, &mut reporter)
        }
        Command::New { id, proposal_only } => lifecycle::new_change_in_mode_with_ui(
            &project,
            &id,
            if proposal_only {
                doco::PackageMode::ProposalOnly
            } else {
                doco::PackageMode::Full
            },
            &mut reporter,
        ),
        Command::List {
            completed,
            archived,
        } => {
            reporter.emit(Event::Changes {
                changes: &lifecycle::list_changes_filtered(&project, completed, archived)?,
            })?;
            Ok(())
        }
        Command::Context { id, history } => {
            let id = resolve_target(
                &project,
                id,
                TargetKind::Context { history },
                interaction,
                capabilities,
                &terminal_prompter,
            )?;
            lifecycle::context_with_ui(&project, &id, history, &mut reporter)
        }
        Command::Check { id } => {
            let id = resolve_target(
                &project,
                id,
                TargetKind::Check,
                interaction,
                capabilities,
                &terminal_prompter,
            )?;
            lifecycle::check_with_ui(&project, &id, &mut reporter)
        }
        Command::Complete { id } => {
            let id = resolve_target(
                &project,
                id,
                TargetKind::Active,
                interaction,
                capabilities,
                &terminal_prompter,
            )?;
            lifecycle::complete_with_ui(&project, &id, &mut reporter)
        }
        Command::Reopen { id } => {
            let id = resolve_target(
                &project,
                id,
                TargetKind::Completed,
                interaction,
                capabilities,
                &terminal_prompter,
            )?;
            lifecycle::reopen_with_ui(&project, &id, &mut reporter)
        }
        Command::Archive { id, yes, dry_run } => {
            let id = resolve_target(
                &project,
                id,
                TargetKind::Completed,
                interaction,
                capabilities,
                &terminal_prompter,
            )?;
            lifecycle::archive_with_ui(&project, &id, yes, dry_run, None, &mut reporter)
        }
        Command::Cancel {
            id,
            reason,
            disposition,
            yes,
            dry_run,
        } => {
            let id = resolve_target(
                &project,
                id,
                TargetKind::Active,
                interaction,
                capabilities,
                &terminal_prompter,
            )?;
            lifecycle::archive_with_ui(
                &project,
                &id,
                yes,
                dry_run,
                Some((&reason, &disposition)),
                &mut reporter,
            )
        }
    }
}

impl Cli {
    fn validate_targets(&self) -> std::result::Result<(), clap::Error> {
        let missing = match &self.command {
            Command::Check { id }
            | Command::Complete { id }
            | Command::Reopen { id }
            | Command::Archive { id, .. }
            | Command::Cancel { id, .. }
            | Command::Context { id, .. } => id.is_none(),
            Command::Init { .. } | Command::New { .. } | Command::List { .. } => false,
        };
        if missing && !self.interactive {
            return Err(Cli::command().error(
                ErrorKind::MissingRequiredArgument,
                "a change ID is required unless --interactive is supplied",
            ));
        }
        Ok(())
    }
}

fn resolve_target(
    project: &Project,
    supplied: Option<String>,
    kind: TargetKind,
    interaction: InteractionMode,
    capabilities: &TerminalCapabilities,
    prompter: &TerminalPrompter,
) -> Result<String> {
    if let Some(id) = supplied {
        return Ok(id);
    }
    if interaction != InteractionMode::Always {
        bail!("change ID is required unless --interactive is supplied");
    }
    if !capabilities.interactive() {
        bail!("--interactive requires an attached terminal");
    }
    let candidates: Vec<_> = project
        .changes()?
        .into_iter()
        .filter(|change| kind.accepts(change))
        .collect();
    if candidates.is_empty() {
        bail!("no changes match required state: {}", kind.description());
    }
    match prompter.select_change(&candidates)? {
        PromptOutcome::Answer(id) => Ok(id),
        PromptOutcome::Cancelled => ui::cancelled(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_existing_ids_require_interactive_but_new_always_requires_an_id() {
        assert_eq!(
            Cli::try_parse_from(["doco", "--interactive", "new"])
                .err()
                .unwrap()
                .kind(),
            ErrorKind::MissingRequiredArgument
        );
        for args in [
            vec!["doco", "context"],
            vec!["doco", "check"],
            vec!["doco", "complete"],
            vec!["doco", "archive"],
            vec!["doco", "reopen"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!(
                cli.validate_targets().unwrap_err().kind(),
                ErrorKind::MissingRequiredArgument
            );
        }
        assert!(
            Cli::try_parse_from(["doco", "--interactive", "check"])
                .unwrap()
                .validate_targets()
                .is_ok()
        );
        assert!(
            Cli::try_parse_from(["doco", "check", "--interactive"])
                .unwrap()
                .validate_targets()
                .is_ok()
        );
    }

    #[test]
    fn interactive_flags_conflict_and_cancel_fields_stay_required() {
        assert_eq!(
            Cli::try_parse_from(["doco", "--interactive", "--no-interactive", "check"])
                .err()
                .unwrap()
                .kind(),
            ErrorKind::ArgumentConflict
        );
        assert_eq!(
            Cli::try_parse_from(["doco", "--interactive", "cancel"])
                .err()
                .unwrap()
                .kind(),
            ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn color_accepts_global_positions() {
        assert!(Cli::try_parse_from(["doco", "--color", "never", "list"]).is_ok());
        assert!(Cli::try_parse_from(["doco", "list", "--color", "always"]).is_ok());
    }

    #[test]
    fn target_kinds_filter_states_without_changing_order() {
        use std::path::PathBuf;
        let changes: Vec<_> = [State::Active, State::Completed, State::Archived]
            .into_iter()
            .map(|state| Change {
                id: state.to_string(),
                state,
                path: PathBuf::new(),
            })
            .collect();
        let ids = |kind: TargetKind| {
            changes
                .iter()
                .filter(|change| kind.accepts(change))
                .map(|change| change.id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(TargetKind::Active), ["active"]);
        assert_eq!(ids(TargetKind::Completed), ["completed"]);
        assert_eq!(ids(TargetKind::Context { history: false }), ["active"]);
        assert_eq!(
            ids(TargetKind::Context { history: true }),
            ["active", "completed", "archived"]
        );
        assert_eq!(ids(TargetKind::Check), ["active", "completed", "archived"]);
    }

    #[test]
    fn explicit_target_never_requires_a_terminal_or_project_scan() {
        let directory = tempfile::tempdir().unwrap();
        let project = Project::open(directory.path()).unwrap();
        let capabilities = TerminalCapabilities {
            stdin_tty: false,
            stdout_tty: false,
            stderr_tty: false,
            term_dumb: true,
            no_color: true,
            columns: 0,
            rows: 0,
        };
        let prompter = TerminalPrompter::new(ColorMode::Never, &capabilities);
        assert_eq!(
            resolve_target(
                &project,
                Some("explicit".into()),
                TargetKind::Active,
                InteractionMode::Always,
                &capabilities,
                &prompter,
            )
            .unwrap(),
            "explicit"
        );
    }
}
