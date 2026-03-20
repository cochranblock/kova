//! Kova — augment engine. GUI + serve.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kova", version)]
struct Args {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run native GUI (egui). Legacy — use TUI instead.
    Gui(GuiArgs),
    /// Terminal UI. Chat + Visual QC. Like Claude Code but local.
    Tui(TuiArgs),
    /// Run HTTP API server. Web client at /.
    Serve(ServeArgs),
    /// Worker daemon for swarm. Phase 1: schema stub.
    Node,
    /// Tokenized orchestration. f18–f23 local or broadcast.
    C2(C2Args),
    /// Create ~/.kova, prompts, config. Run on first use.
    Bootstrap,
    /// Print loaded Cursor prompts (baked + external). For testing and debugging.
    Prompts,
    /// Model management (install Qwen2.5-Coder).
    Model(ModelArgs),
    /// Recent changes (f86). Tokenized for LLM context. Stay on task.
    Recent(RecentArgs),
    /// Autopilot: type prompt into Cursor composer. No API costs. Requires Cursor focused.
    Autopilot(AutopilotArgs),
    /// Deploy quality gate: clippy, TRIPLE SIMS, release build, smoke, baked demo. Requires --features tests.
    Test,
    /// Tokenized cargo commands. §13 compressed output. x0=build x1=check x2=test x3=clippy x4=run x5=build-rel x6=clean x7=doc x8=fmt-chk x9=bench.
    X(XArgs),
    /// Interactive REPL. Agentic tool loop with local LLM. Like Claude Code but local.
    Chat(ChatArgs),
    /// Tokenized git commands. §13 compressed output. g0=status g1=diff g2=log g3=push g4=pull g5=commit g6=branch g7=stash g8=add g9=staged.
    #[command(name = "git")]
    Git(GitArgs),
    /// Short serve alias. `kova s` = `kova serve --open`. `kova s -d` = demo mode.
    S(SShortArgs),
    /// IRONHIVE cluster inference. Distributed AI across worker nodes.
    #[cfg(feature = "inference")]
    #[command(name = "cluster")]
    T193(ClusterArgs),
    /// Rust Binary T181. Full pipeline: classify → generate → compile → review → fix.
    #[cfg(feature = "inference")]
    #[command(name = "factory")]
    T181(FactoryArgs),
    /// Mixture of Experts. Fan-out to N nodes, compile all, score, pick winner.
    #[cfg(feature = "inference")]
    #[command(name = "moe")]
    Moe(MoeArgs),
    /// Academy. MoE-powered autonomous dev agent. Plan → generate → wire → test → fix → commit.
    #[cfg(feature = "inference")]
    #[command(name = "academy")]
    Academy(AcademyArgs),
    /// Gauntlet. Hell Week stress test for the AI pipeline. 5 phases, no mercy.
    #[cfg(feature = "inference")]
    #[command(name = "gauntlet")]
    Gauntlet(GauntletArgs),
    /// Micro-model registry. List, run, and validate tiny purpose-built AI units.
    #[cfg(feature = "inference")]
    #[command(name = "micro")]
    Micro(MicroArgs),
    /// RAG: index code, search semantically, retrieve context for LLM.
    #[command(name = "rag")]
    Rag(RagArgs),
    /// LLM call traces. Observability for every inference call.
    #[command(name = "traces")]
    Traces(TracesArgs),
    /// MCP server (Model Context Protocol). Stdio transport for AI tool interop.
    #[command(name = "mcp")]
    Mcp(McpArgs),
    /// CI mode. Headless quality gate: run check/clippy/test, watch for changes.
    #[command(name = "ci")]
    Ci(CiArgs),
    /// Export training data from LLM traces. DPO/SFT fine-tuning.
    #[command(name = "export")]
    Export(ExportArgs),
    /// Code review. Review staged changes or branch diff via LLM.
    #[cfg(feature = "inference")]
    #[command(name = "review")]
    Review(ReviewArgs),
    /// Feedback loop. View/export tournament failure data and generated challenges.
    #[cfg(feature = "inference")]
    #[command(name = "feedback")]
    Feedback(FeedbackArgs),
    /// Tokenization validator. Check compression protocol coverage.
    #[command(name = "tokens")]
    Tokens,
}

#[derive(clap::Args)]
struct McpArgs {
    /// Project directory (default: cwd).
    #[arg(short, long)]
    project: Option<std::path::PathBuf>,
}

#[derive(clap::Args)]
struct CiArgs {
    #[command(subcommand)]
    cmd: CiCmd,
}

#[derive(clap::Subcommand)]
enum CiCmd {
    /// Single CI check on current or specified project.
    Run {
        /// Project directory (default: cwd).
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
        /// Skip clippy.
        #[arg(long)]
        no_clippy: bool,
        /// Skip tests.
        #[arg(long)]
        no_tests: bool,
    },
    /// Continuous watch mode. Re-runs CI on file changes.
    Watch {
        /// Project directory (default: cwd).
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
        /// Poll interval in seconds (default: 5).
        #[arg(short, long, default_value = "5")]
        interval: u64,
        /// Skip clippy.
        #[arg(long)]
        no_clippy: bool,
        /// Skip tests.
        #[arg(long)]
        no_tests: bool,
    },
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct ReviewArgs {
    #[command(subcommand)]
    cmd: ReviewCmd,
}

#[cfg(feature = "inference")]
#[derive(clap::Subcommand)]
enum ReviewCmd {
    /// Review staged changes.
    Staged {
        /// Project directory (default: cwd).
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
    },
    /// Review diff between current branch and base.
    Branch {
        /// Base branch to diff against (default: main).
        #[arg(default_value = "main")]
        base: String,
        /// Project directory (default: cwd).
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
    },
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct FeedbackArgs {
    #[command(subcommand)]
    cmd: FeedbackCmd,
}

#[cfg(feature = "inference")]
#[derive(clap::Subcommand)]
enum FeedbackCmd {
    /// Show failure statistics.
    Stats,
    /// List recent failures.
    Recent {
        /// Number of failures to show.
        #[arg(short = 'n', default_value = "10")]
        limit: usize,
    },
    /// Export generated challenges as Rust tce() calls.
    Export,
}

#[derive(clap::Args)]
struct ExportArgs {
    #[command(subcommand)]
    cmd: ExportCmd,
}

#[derive(clap::Subcommand)]
enum ExportCmd {
    /// Export LLM traces as training data (JSONL, CSV, or DPO pairs).
    Training {
        /// Format: jsonl, csv, or dpo.
        #[arg(long, default_value = "jsonl")]
        format: String,
        /// Output file path. Default: ~/.kova/training_data/<format-based>.
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },
}

#[derive(clap::Args)]
struct RagArgs {
    #[command(subcommand)]
    cmd: RagCmd,
}

#[derive(clap::Subcommand)]
enum RagCmd {
    /// Index a directory (default: current dir). Embeds all .rs files.
    Index {
        /// Directory to index.
        #[arg(default_value = ".")]
        dir: String,
    },
    /// Search the index with a natural language query.
    Search {
        /// Query string.
        query: String,
        /// Number of results.
        #[arg(short = 'k', default_value = "5")]
        k: usize,
    },
    /// Show index stats.
    Stats,
    /// Clear the entire index.
    Clear,
    /// Index all discovered projects (from config).
    IndexAll,
    /// Auto-reindex: only re-index projects with modified .rs files since last index.
    Auto,
}

#[derive(clap::Args)]
struct TracesArgs {
    #[command(subcommand)]
    cmd: TracesCmd,
}

#[derive(clap::Subcommand)]
enum TracesCmd {
    /// Show recent LLM call traces.
    Recent {
        /// Number of traces to show.
        #[arg(short = 'n', default_value = "20")]
        limit: usize,
    },
    /// Show aggregate LLM stats.
    Stats,
}

#[derive(clap::Args)]
struct C2Args {
    #[command(subcommand)]
    cmd: C2Cmd,
}

#[derive(Subcommand)]
enum C2Cmd {
    /// Run tokenized command (f18–f23).
    Run {
        #[arg(value_enum)]
        token: kova::c2::T212,
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
        #[arg(short, long)]
        broadcast: bool,
        #[arg(long)]
        release: bool,
        /// Restrict broadcast to specific nodes (e.g. lf,bt). Default: all reachable.
        #[arg(long)]
        nodes: Option<String>,
        /// Build on local path (/tmp/hive-build) instead of NFS. Faster; run sync --local first.
        #[arg(long)]
        local: bool,
    },
    /// List worker nodes (lf gd bt st).
    Nodes,
    /// Inspect resources (CPU, RAM, disk, GPU) on c2-core + all workers.
    Inspect {
        /// Output JSON for scripting.
        #[arg(long)]
        json: bool,
    },
    /// Print placement recommendations based on inspect data.
    Recommend,
    /// One-command sync + build. Parallel. Prefer over sync + run.
    Build {
        #[arg(long)]
        broadcast: bool,
        #[arg(long)]
        release: bool,
        /// Skip sync; assume workers already have fresh content.
        #[arg(long)]
        no_sync: bool,
        /// Build on /tmp/hive-build (local) instead of NFS. Faster.
        #[arg(long)]
        local: bool,
        /// Restrict to nodes (e.g. lf,gd). Default: all reachable.
        #[arg(long)]
        nodes: Option<String>,
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
    },
    /// Sync workspace to workers. Prefer `kova c2 build --broadcast` for one-command sync+build.
    Sync {
        #[arg(long)]
        dry_run: bool,
        /// Target host (default: st). Ignored when --all.
        #[arg(long, default_value = "st")]
        target: String,
        /// Sync to /tmp/hive-build on workers instead of /mnt/hive. Use with run --local.
        #[arg(long)]
        local: bool,
        /// Sync to all workers (lf gd bt st).
        #[arg(long)]
        all: bool,
        /// Full sync (tar-stream). Use when workers have no content. Default: incremental rsync.
        #[arg(long)]
        full: bool,
    },
    /// SSH host CA: init, sign, setup. No host key churn when IPs change.
    SshCa {
        #[command(subcommand)]
        cmd: SshCaCmd,
    },
    /// Offload build artifacts to worker node, free local disk.
    Offload {
        /// Show what would be moved without doing it.
        #[arg(long)]
        dry_run: bool,
        /// Disk usage threshold to trigger offload (default: from config or 90%).
        #[arg(long)]
        threshold: Option<u8>,
        /// Target worker node for archive (default: bt).
        #[arg(long)]
        target: Option<String>,
    },
    /// Wake-on-LAN: power on a worker node (lf, gd, bt). st has no WoL.
    Wake {
        /// Node to wake: lf, gd, or bt.
        node: String,
    },
    /// Tokenized node commands (c1-c9, ci, ct). §13 compressed output.
    Ncmd {
        /// Command token: c1(nstat) c2(nspec) c3(nsvc) c4(nrust) c5(nsync) c6(nbuild) c7(nlog) c8(nkill) c9(ndeploy) ci(inspect) ct(ntest).
        #[arg(value_enum)]
        cmd: kova::node_cmd::t96,
        /// Restrict to nodes (e.g. n0,n2 or lf,bt). Default: all.
        #[arg(long)]
        nodes: Option<String>,
        /// Use idlest node (from nci). Overrides --nodes.
        #[arg(long)]
        idle: bool,
        /// Extra arg: process name (c8), unit (c7), project path (c5/c6/c9), "install" (c4), project (ct).
        #[arg(long)]
        extra: Option<String>,
        /// Release mode (c6/c9).
        #[arg(long)]
        release: bool,
        /// Lines to tail (c7, default 50).
        #[arg(long, default_value = "50")]
        lines: u32,
        /// Expand oN tokens to human-readable names.
        #[arg(long)]
        expand: bool,
        /// Ultra-compact single line (ci only): n0:4c/2G/0.5 n1:8c/3G/0.2
        #[arg(long)]
        oneline: bool,
    },
}

#[derive(clap::Subcommand)]
enum SshCaCmd {
    /// Create CA key, add @cert-authority to known_hosts.
    Init,
    /// Sign host cert for one node, deploy, print sshd instructions.
    Sign {
        #[arg(value_name = "NODE")]
        node: String,
    },
    /// Init + sign all workers (lf gd bt st).
    Setup,
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct FactoryArgs {
    /// What to build.
    prompt: Vec<String>,
    /// Project directory (default: cwd).
    #[arg(short, long)]
    project: Option<std::path::PathBuf>,
    /// Max fix retries (default: 2).
    #[arg(long, default_value = "2")]
    retries: u32,
    /// Skip code review stage.
    #[arg(long)]
    no_review: bool,
    /// Skip clippy.
    #[arg(long)]
    no_clippy: bool,
    /// Skip tests.
    #[arg(long)]
    no_tests: bool,
    /// Context window size.
    #[arg(long, default_value = "8192")]
    ctx: u32,
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct MoeArgs {
    /// What to build.
    prompt: Vec<String>,
    /// Number of expert variants to generate (default: 3).
    #[arg(short, long, default_value = "3")]
    experts: usize,
    /// Skip code review stage.
    #[arg(long)]
    no_review: bool,
    /// Skip clippy.
    #[arg(long)]
    no_clippy: bool,
    /// Skip tests.
    #[arg(long)]
    no_tests: bool,
    /// Context window size.
    #[arg(long, default_value = "8192")]
    ctx: u32,
    /// Save winning expert to ~/.kova/experts/.
    #[arg(long)]
    save: bool,
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct AcademyArgs {
    /// High-level task description.
    task: Vec<String>,
    /// Project directory (default: cwd).
    #[arg(short, long)]
    project: Option<std::path::PathBuf>,
    /// Number of MoE experts per generation step (default: 2).
    #[arg(short, long, default_value = "2")]
    experts: usize,
    /// Max fix retries per step (default: 3).
    #[arg(long, default_value = "3")]
    retries: u32,
    /// Context window size.
    #[arg(long, default_value = "8192")]
    ctx: u32,
    /// Skip auto-commit.
    #[arg(long)]
    no_commit: bool,
    /// Dry run — plan only, don't execute.
    #[arg(long)]
    dry_run: bool,
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct GauntletArgs {
    /// Run only specific phases (e.g. 1 2 3). Default: all.
    phases: Vec<u8>,
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct MicroArgs {
    #[command(subcommand)]
    cmd: MicroCmd,
}

#[cfg(feature = "inference")]
#[derive(Subcommand)]
enum MicroCmd {
    /// List all registered micro-model templates.
    List,
    /// Run a specific micro-model template against input.
    Run {
        /// Template ID (e.g. f79, f80, f81) or name (e.g. classify_intent).
        template: String,
        /// Input text. If not provided, reads from stdin.
        input: Vec<String>,
        /// Target node URL (bypass cluster routing).
        #[arg(long)]
        node: Option<String>,
        /// Model override.
        #[arg(long)]
        model: Option<String>,
    },
    /// Validate a micro-model response.
    Validate {
        /// Template ID.
        template: String,
        /// Response text to validate.
        response: String,
        /// Original input (for coherence check).
        #[arg(long)]
        input: Option<String>,
    },
    /// Route an input to the best micro-model (shows what would be selected).
    Route {
        /// Input text to classify.
        input: Vec<String>,
    },
    /// Full pipeline: classify → route → run → validate. One command.
    Pipe {
        /// Input text.
        input: Vec<String>,
    },
    /// Benchmark all templates against held-out challenges.
    Bench,
    /// Show historical per-template run statistics.
    Stats,
    /// Tournament: pit every model on every node against each other. Auto-resumes from checkpoint.
    Tournament,
    /// Clear a stale tournament checkpoint (start fresh next run).
    TournamentClear,
    /// Export training data from tournament results.
    Export {
        /// Format: dpo, sft, or all.
        #[arg(long, default_value = "all")]
        format: String,
    },
    /// Show training data stats from last tournament.
    TrainStats,
    /// Academy: analyze tournament results, detect gaps, recommend curriculum changes.
    Academy,
    /// Mine conversation logs for training data.
    Mine,
    /// Mine and export conversation logs as training JSONL.
    MineExport,
    /// Run LoRA fine-tuning via mlx_lm (Apple Silicon).
    Train {
        /// Format: sft or dpo (default: sft).
        #[arg(long, default_value = "sft")]
        format: String,
        /// Training iterations.
        #[arg(long)]
        iters: Option<u32>,
        /// Print command, don't run.
        #[arg(long)]
        dry_run: bool,
    },
}

#[cfg(feature = "inference")]
#[derive(clap::Args)]
struct ClusterArgs {
    #[command(subcommand)]
    cmd: ClusterCmd,
}

#[cfg(feature = "inference")]
#[derive(Subcommand)]
enum ClusterCmd {
    /// Show cluster status: nodes, models, health.
    Status,
    /// Ping all ollama endpoints.
    Health,
    /// Generate code via cluster (routes to best node).
    Gen {
        /// Prompt for code generation.
        prompt: Vec<String>,
        /// System prompt override.
        #[arg(long)]
        system: Option<String>,
        /// Context window size.
        #[arg(long, default_value = "8192")]
        ctx: u32,
    },
    /// Review code via cluster.
    Review {
        /// File to review.
        file: std::path::PathBuf,
    },
    /// Fix compile error via cluster.
    Fix {
        /// File with broken code.
        file: std::path::PathBuf,
        /// Compiler error message.
        #[arg(long)]
        error: String,
    },
    /// Benchmark tok/s on each node.
    Bench,
    /// List models on all nodes.
    Models,
}

#[derive(clap::Args)]
struct ChatArgs {
    /// Project directory (default: cwd).
    #[arg(short, long)]
    project: Option<std::path::PathBuf>,
}

#[derive(clap::Args)]
struct XArgs {
    /// Command token: x0(build) x1(check) x2(test) x3(clippy) x4(run) x5(build-rel) x6(clean) x7(doc) x8(fmt-chk) x9(bench).
    #[arg(value_enum)]
    cmd: kova::cargo_cmd::t99,
    /// Project token: p0(kova) p1(approuter) p2(cochranblock) p3(oakilydokily) p4(rogue-repo) p5(ronin-sites). Default: p0.
    #[arg(short, long)]
    project: Option<String>,
    /// Features (e.g. "gui,serve,inference"). Overrides preset.
    #[arg(short, long)]
    features: Option<String>,
    /// Binary name (for x4/run).
    #[arg(long)]
    bin: Option<String>,
    /// Extra cargo args.
    #[arg(last = true)]
    extra: Vec<String>,
    /// Run on all workspace crates in parallel.
    #[arg(long)]
    all: bool,
    /// Chain commands sequentially: --chain x1,x2,x3 (stop on first error).
    #[arg(long)]
    chain: Option<String>,
    /// Expand rN tokens to human-readable names.
    #[arg(long)]
    expand: bool,
}

#[derive(clap::Args)]
struct GuiArgs {
    /// Enable demo mode: record actions to ~/.kova/demos/
    #[arg(long)]
    demo: bool,
}

#[derive(clap::Args)]
struct TuiArgs {
    /// Project directory (default: cwd).
    #[arg(short, long)]
    project: Option<std::path::PathBuf>,
}

#[derive(clap::Args)]
struct ServeArgs {
    /// Open browser to web client after starting
    #[arg(long)]
    open: bool,
    /// Enable demo mode: open with ?demo=1 for action recording
    #[arg(long)]
    demo: bool,
}

#[derive(clap::Args)]
struct GitArgs {
    /// Command token: g0(status) g1(diff) g2(log) g3(push) g4(pull) g5(commit) g6(branch) g7(stash) g8(add) g9(staged).
    #[arg(value_enum)]
    cmd: kova::git_cmd::t107,
    /// Log count for g2 (default 10).
    #[arg(short = 'n', long, default_value = "10")]
    count: u32,
    /// Commit message for g5.
    #[arg(short, long)]
    message: Option<String>,
    /// Files for g8 (add). Empty = add all.
    #[arg(last = true)]
    files: Vec<String>,
}

#[derive(clap::Args)]
struct SShortArgs {
    /// Demo mode.
    #[arg(short, long)]
    demo: bool,
}

#[derive(clap::Args)]
struct AutopilotArgs {
    /// Prompt to type into Cursor agent composer
    #[arg(required = true)]
    prompt: Vec<String>,
}

#[derive(clap::Args)]
struct RecentArgs {
    /// Project directory (default: cwd)
    #[arg(short, long)]
    project: Option<std::path::PathBuf>,
    /// Files modified in last N minutes (default: 30)
    #[arg(short, long, default_value = "30")]
    minutes: u64,
}

#[derive(clap::Args)]
struct ModelArgs {
    #[command(subcommand)]
    cmd: ModelCmd,
}

#[derive(Subcommand)]
enum ModelCmd {
    /// Download Qwen2.5-Coder-0.5B-Instruct GGUF to ~/.kova/models/
    Install,
    /// List configured models and paths (router, coder, fix).
    List,
}

#[cfg(feature = "gui")]
fn run_gui(demo: bool) -> anyhow::Result<()> {
    kova::bootstrap()?;
    kova::gui::run(demo)
}

#[cfg(not(feature = "gui"))]
fn run_gui(_demo: bool) -> anyhow::Result<()> {
    anyhow::bail!("GUI feature not enabled. Use `kova tui` or `kova serve --open`")
}

#[cfg(feature = "tui")]
fn run_tui(project: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    kova::bootstrap()?;
    kova::tui::run(project)
}

#[cfg(not(feature = "tui"))]
fn run_tui(_project: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    anyhow::bail!("Build with --features tui for terminal UI")
}

#[cfg(feature = "tests")]
fn run_test() -> anyhow::Result<()> {
    // Delegate to kova-test binary to avoid tokio runtime nesting (baked demo uses block_on).
    let exe = std::env::current_exe()?;
    let test_bin = exe
        .parent()
        .unwrap()
        .join("kova-test")
        .with_extension(std::env::consts::EXE_EXTENSION);
    if test_bin.exists() {
        let status = std::process::Command::new(&test_bin).status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
    kova::f315()
}

#[cfg(feature = "serve")]
async fn run_serve(open: bool, demo: bool) -> anyhow::Result<()> {
    let addr = kova::bind_addr();
    if open {
        let url = if demo {
            format!("http://{}?demo=1", addr)
        } else {
            format!("http://{}", addr)
        };
        kova::serve::run_with_open(addr, &url).await
    } else {
        kova::serve::run(addr).await
    }
}

#[cfg(not(feature = "serve"))]
async fn run_serve(_open: bool, _demo: bool) -> anyhow::Result<()> {
    anyhow::bail!("Build with --features serve for serve mode")
}

#[cfg(feature = "daemon")]
fn run_node() -> anyhow::Result<()> {
    kova::daemon::run();
    Ok(())
}

#[cfg(not(feature = "daemon"))]
fn run_node() -> anyhow::Result<()> {
    anyhow::bail!("Build with --features daemon for node mode")
}

async fn run_c2(args: C2Args) -> anyhow::Result<()> {
    kova::bootstrap()?;
    match args.cmd {
        C2Cmd::Nodes => {
            kova::c2::f355();
            Ok(())
        }
        C2Cmd::Run {
            token,
            project,
            broadcast,
            release,
            nodes,
            local,
        } => kova::c2::f354(token, project, broadcast, release, nodes, local),
        C2Cmd::Inspect { json } => {
            let hosts = kova::inspect::f359();
            if json {
                kova::inspect::f362(&hosts);
            } else {
                kova::inspect::f360(&hosts);
            }
            Ok(())
        }
        C2Cmd::Recommend => {
            let hosts = kova::inspect::f359();
            kova::inspect::f360(&hosts);
            kova::inspect::f361(&hosts);
            Ok(())
        }
        C2Cmd::Build {
            broadcast,
            release,
            no_sync,
            local,
            nodes,
            project,
        } => kova::c2::f356(broadcast, release, no_sync, local, nodes, project),
        C2Cmd::Sync {
            dry_run,
            target,
            local,
            all,
            full,
        } => kova::c2::f358(dry_run, &target, local, all, full),
        C2Cmd::Offload { dry_run, threshold, target } => {
            let thresh = threshold.unwrap_or_else(|| kova::config::offload_threshold());
            kova::c2::f360(dry_run, thresh, target)
        }
        C2Cmd::Wake { node } => {
            match kova::c2::f352(&node) {
                Ok(()) => {
                    println!("WoL sent to {}", node);
                    Ok(())
                }
                Err(e) => anyhow::bail!("{}", e),
            }
        }
        C2Cmd::SshCa { cmd } => match cmd {
            SshCaCmd::Init => kova::ssh_ca::f298(),
            SshCaCmd::Sign { node } => kova::ssh_ca::f299(&node),
            SshCaCmd::Setup => kova::ssh_ca::f300(),
        },
        C2Cmd::Ncmd {
            cmd,
            nodes,
            idle,
            extra,
            release,
            lines,
            expand,
            oneline,
        } => {
            let nodes_opt = if idle {
                let all: Vec<String> = kova::c2::f350()
                    .into_iter()
                    .map(String::from)
                    .collect();
                kova::node_cmd::pick_idlest(&all)
            } else {
                nodes
            };
            if idle && nodes_opt.is_none() {
                anyhow::bail!("No reachable nodes. Run: kova c2 ncmd ci");
            }
            kova::node_cmd::f132(cmd, nodes_opt, extra, release, lines, expand, oneline)
        }
    }
}

#[cfg(feature = "inference")]
fn run_micro(args: MicroArgs) -> anyhow::Result<()> {
    use kova::micro::{
        bench, pipe, registry::T149, router::T153, runner, stats, validate,
    };

    let mut registry = T149::new();

    // Load user templates from ~/.kova/micro/
    let micro_dir = kova::kova_dir().join("micro");
    if micro_dir.is_dir()
        && let Ok(n) = registry.load_dir(&micro_dir)
        && n > 0
    {
        eprintln!("[micro] loaded {} user templates from {:?}", n, micro_dir);
    }

    match args.cmd {
        MicroCmd::List => {
            print!("{}", registry.status());
            Ok(())
        }
        MicroCmd::Run {
            template,
            input,
            node,
            model,
        } => {
            // Find template by ID or name
            let tmpl = registry
                .get(&template)
                .or_else(|| registry.get_by_name(&template))
                .ok_or_else(|| anyhow::anyhow!("unknown template: {}", template))?;

            let input_text = if input.is_empty() {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            } else {
                input.join(" ")
            };

            let result = if let Some(url) = node {
                runner::f245(tmpl, &input_text, &url, model.as_deref())
                    .map_err(|e| anyhow::anyhow!("{}", e))?
            } else {
                let cluster = kova::cluster::T193::default_hive();
                let breaker = runner::T155::new(3);
                let budget = runner::T156::new(100_000);
                runner::f244(tmpl, &input_text, &cluster, &breaker, &budget)
                    .map_err(|e| anyhow::anyhow!("{}", e))?
            };

            println!("{}", result.response);
            eprintln!(
                "[micro] {} on {} ({:?})",
                result.template_id, result.node_id, result.duration
            );

            // Record stats
            let sp = stats::f246();
            let mut st = stats::T158::load(&sp);
            let dur_ms = result.duration.as_millis() as u64;
            let tokens = result.tokens.unwrap_or(0);
            if validate::f264(&result.response) {
                st.record_pass(&result.template_id, dur_ms, tokens);
            } else {
                st.record_fail(&result.template_id, dur_ms, tokens);
            }
            let _ = st.save(&sp);

            Ok(())
        }
        MicroCmd::Validate {
            template,
            response,
            input,
        } => {
            let tmpl = registry
                .get(&template)
                .or_else(|| registry.get_by_name(&template))
                .ok_or_else(|| anyhow::anyhow!("unknown template: {}", template))?;

            let mock_result = runner::T154 {
                template_id: tmpl.id.clone(),
                node_id: "validate".into(),
                model: tmpl.model.clone(),
                response: response.clone(),
                duration: std::time::Duration::ZERO,
                tokens: None,
            };

            let input_text = input.as_deref().unwrap_or("");
            let result = validate::f263(&mock_result, input_text, &tmpl.output_schema);

            println!("{}", result.summary);
            for check in &result.checks {
                println!(
                    "  {} {} — {}",
                    if check.passed { "✓" } else { "✗" },
                    check.name,
                    check.detail
                );
            }
            println!("confidence: {:.0}%", result.confidence * 100.0);
            Ok(())
        }
        MicroCmd::Route { input } => {
            let input_text = input.join(" ");
            let router = T153::new();
            let decision = router.route(&input_text, &registry, None);
            let tmpl = registry.get(&decision.template_id);
            println!(
                "route: {} (confidence: {:.0}%, method: {:?})",
                decision.template_id,
                decision.confidence * 100.0,
                decision.method
            );
            if let Some(t) = tmpl {
                println!("  name: {}", t.name);
                println!("  tier: {} ({})", t.tier, t.model);
                println!("  purpose: {}", t.purpose);
            }
            Ok(())
        }
        MicroCmd::Pipe { input } => {
            let input_text = input.join(" ");
            if input_text.is_empty() {
                anyhow::bail!("pipe requires input text");
            }
            let cluster = kova::cluster::T193::default_hive();
            match pipe::f240(&input_text, &registry, &cluster) {
                Ok(result) => {
                    pipe::f241(&result);

                    // Record stats
                    let sp = stats::f246();
                    let mut st = stats::T158::load(&sp);
                    let dur_ms = result.total_duration.as_millis() as u64;
                    if result.validation.passed {
                        st.record_pass(&result.template_id, dur_ms, 0);
                    } else {
                        st.record_fail(&result.template_id, dur_ms, 0);
                    }
                    let _ = st.save(&sp);
                    Ok(())
                }
                Err(e) => anyhow::bail!("pipe failed: {}", e),
            }
        }
        MicroCmd::Bench => {
            let cluster = kova::cluster::T193::default_hive();
            let results = bench::f233(&registry, &cluster);
            bench::f236(&results);

            // Record bench results into stats
            let sp = stats::f246();
            let mut st = stats::T158::load(&sp);
            for r in &results {
                let tokens = (r.response.len() / 4) as u64;
                if r.error.is_some() {
                    st.record_error(&r.template_id, r.duration_ms);
                } else if r.passed {
                    st.record_pass(&r.template_id, r.duration_ms, tokens);
                } else {
                    st.record_fail(&r.template_id, r.duration_ms, tokens);
                }
            }
            let _ = st.save(&sp);
            Ok(())
        }
        MicroCmd::Stats => {
            let sp = stats::f246();
            let st = stats::T158::load(&sp);
            if st.templates.is_empty() {
                println!("No stats yet. Run `kova micro run` or `kova micro bench` first.");
            } else {
                st.print();
            }
            Ok(())
        }
        MicroCmd::Tournament => {
            use kova::micro::tournament;
            let cluster = kova::cluster::T193::default_hive();
            let result = tournament::f250(&registry, &cluster);
            tournament::f251(&result);

            // Save results + feed stats
            let _ = tournament::f252(&result);
            let sp = stats::f246();
            let mut st = stats::T158::load(&sp);
            for m in &result.matches {
                let key = format!("{}:{}", m.competitor.model, m.category);
                if m.passed {
                    st.record_pass(&key, m.duration_ms, m.tokens);
                } else if m.tokens == 0 && m.response_len == 0 {
                    st.record_error(&key, m.duration_ms);
                } else {
                    st.record_fail(&key, m.duration_ms, m.tokens);
                }
            }
            let _ = st.save(&sp);
            Ok(())
        }
        MicroCmd::TournamentClear => {
            use kova::micro::tournament;
            let cp = tournament::f254();
            if cp.exists() {
                std::fs::remove_file(&cp)?;
                println!("Checkpoint cleared. Next tournament starts fresh.");
            } else {
                println!("No checkpoint found.");
            }
            Ok(())
        }
        MicroCmd::Export { format } => {
            use kova::micro::{tournament, train};
            let tp = tournament::f253();
            if !tp.exists() {
                anyhow::bail!("no tournament results found — run `kova micro tournament` first");
            }
            let json = std::fs::read_to_string(&tp)?;
            let result: tournament::T165 = serde_json::from_str(&json)?;
            train::f259(&result, &registry, &format)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(())
        }
        MicroCmd::TrainStats => {
            use kova::micro::{tournament, train};
            let tp = tournament::f253();
            if !tp.exists() {
                anyhow::bail!("no tournament results found — run `kova micro tournament` first");
            }
            let json = std::fs::read_to_string(&tp)?;
            let result: tournament::T165 = serde_json::from_str(&json)?;
            train::f260(&result, &registry);
            Ok(())
        }
        MicroCmd::Academy => {
            use kova::micro::{tournament, academy};
            let tp = tournament::f253();
            if !tp.exists() {
                anyhow::bail!("no tournament results found — run `kova micro tournament` first");
            }
            let json = std::fs::read_to_string(&tp)?;
            let result: tournament::T165 = serde_json::from_str(&json)?;
            let report = academy::f230(&result);
            academy::f231(&report);
            let path = academy::f232(&report).map_err(|e| anyhow::anyhow!("{}", e))?;
            eprintln!("Report saved: {}", path.display());
            Ok(())
        }
        MicroCmd::Mine => {
            use kova::micro::logmine;
            let (examples, stats) = logmine::f237().map_err(|e| anyhow::anyhow!("{}", e))?;
            logmine::f239(&stats, &examples);
            Ok(())
        }
        MicroCmd::MineExport => {
            use kova::micro::logmine;
            let (examples, stats) = logmine::f237().map_err(|e| anyhow::anyhow!("{}", e))?;
            logmine::f239(&stats, &examples);
            logmine::f238(&examples).map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(())
        }
        MicroCmd::Train { format, iters, dry_run } => {
            use kova::micro::train_harness::{f262, T172};
            let fmt = match format.as_str() {
                "sft" => T172::Sft,
                "dpo" => T172::Dpo,
                _ => anyhow::bail!("format must be sft or dpo"),
            };
            f262(fmt, iters, dry_run).map_err(anyhow::Error::msg)
        }
    }
}

#[cfg(feature = "inference")]
fn run_cluster(args: ClusterArgs) -> anyhow::Result<()> {
    let cluster = kova::cluster::T193::default_hive();
    match args.cmd {
        ClusterCmd::Status => {
            print!("{}", cluster.status());
            Ok(())
        }
        ClusterCmd::Health => {
            let results = cluster.health_check();
            for (id, online, ver) in &results {
                let status = if *online {
                    format!("online ({})", ver.as_deref().unwrap_or("?"))
                } else {
                    "OFFLINE".into()
                };
                println!("  {} — {}", id, status);
            }
            let online = results.iter().filter(|(_, o, _)| *o).count();
            println!("\n{}/{} nodes online", online, results.len());
            Ok(())
        }
        ClusterCmd::Gen {
            prompt,
            system,
            ctx,
        } => {
            let prompt = prompt.join(" ");
            let system = system.unwrap_or_else(|| {
                "You are a Rust systems programming expert. Write clean, idiomatic Rust. No filler.".into()
            });
            println!("[cluster] dispatching code gen...");
            match cluster.dispatch(
                kova::cluster::T191::CodeGen,
                &system,
                &prompt,
                Some(ctx),
            ) {
                Ok((node, response)) => {
                    println!("[cluster] {} responded:\n", node);
                    println!("{}", response);
                    Ok(())
                }
                Err(e) => anyhow::bail!("cluster gen failed: {}", e),
            }
        }
        ClusterCmd::Review { file } => {
            let code = std::fs::read_to_string(&file)?;
            let system = "Review this Rust code. Flag: correctness issues, anti-patterns, P12 slop words (utilize/leverage/optimize/comprehensive/robust/seamlessly), unnecessary abstractions. Be direct.";
            println!("[cluster] dispatching review of {}...", file.display());
            match cluster.dispatch(
                kova::cluster::T191::CodeReview,
                system,
                &code,
                Some(8192),
            ) {
                Ok((node, response)) => {
                    println!("[cluster] {} review:\n", node);
                    println!("{}", response);
                    Ok(())
                }
                Err(e) => anyhow::bail!("cluster review failed: {}", e),
            }
        }
        ClusterCmd::Fix { file, error } => {
            let code = std::fs::read_to_string(&file)?;
            match kova::cluster::f340(
                "Fix this Rust code. Return only the corrected code block.",
                &code,
                &error,
            ) {
                Ok(response) => {
                    println!("{}", response);
                    Ok(())
                }
                Err(e) => anyhow::bail!("cluster fix failed: {}", e),
            }
        }
        ClusterCmd::Bench => {
            println!("[cluster] benchmarking all nodes...\n");
            let prompt =
                "Write a Rust function that computes the nth Fibonacci number iteratively.";
            let system = "You are a Rust expert. Write clean code. No explanation.";

            let handles: Vec<_> = cluster
                .nodes
                .iter()
                .map(|node| {
                    let provider = node.provider();
                    let model = node.model.clone();
                    let id = node.id.clone();
                    std::thread::spawn(move || {
                        let start = std::time::Instant::now();
                        let result =
                            kova::providers::f199(&provider, &model, system, prompt)
                                .map(|r| r.text);
                        let elapsed = start.elapsed();
                        (id, model, result, elapsed)
                    })
                })
                .collect();

            for h in handles {
                if let Ok((id, model, result, elapsed)) = h.join() {
                    match result {
                        Ok(resp) => {
                            let tokens = resp.split_whitespace().count(); // rough estimate
                            let tps = tokens as f64 / elapsed.as_secs_f64();
                            println!(
                                "  {} ({}) — {:.1}s, ~{} tokens, ~{:.1} tok/s",
                                id,
                                model,
                                elapsed.as_secs_f64(),
                                tokens,
                                tps
                            );
                        }
                        Err(e) => println!("  {} ({}) — FAILED: {}", id, model, e),
                    }
                }
            }
            Ok(())
        }
        ClusterCmd::Models => {
            for node in &cluster.nodes {
                print!("  {} — ", node.id);
                match kova::providers::f336(&node.provider()) {
                    Ok(models) => {
                        let names: Vec<_> = models
                            .iter()
                            .map(|m| {
                                format!("{} ({:.1}GB)", m.name, m.size as f64 / 1_073_741_824.0)
                            })
                            .collect();
                        println!("{}", names.join(", "));
                    }
                    Err(e) => println!("OFFLINE ({})", e),
                }
            }
            Ok(())
        }
    }
}

#[cfg(feature = "inference")]
fn run_review(args: ReviewArgs) -> anyhow::Result<()> {
    let provider = kova::providers::f333();

    match args.cmd {
        ReviewCmd::Staged { project } => {
            let project = project.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let result = kova::review::f186(&project, &provider)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("{}", kova::review::f188(&result));
            Ok(())
        }
        ReviewCmd::Branch { base, project } => {
            let project = project.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let result = kova::review::f187(&project, &base, &provider)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("{}", kova::review::f188(&result));
            Ok(())
        }
    }
}

#[cfg(feature = "inference")]
fn run_feedback(args: FeedbackArgs) -> anyhow::Result<()> {
    match args.cmd {
        FeedbackCmd::Stats => {
            let stats = kova::feedback::f198();
            println!("Failures: {}", stats.total_failures);
            println!("Generated challenges: {}", stats.generated_challenges);
            if !stats.by_model.is_empty() {
                println!("\nBy model:");
                for (model, count) in &stats.by_model {
                    println!("  {}: {}", model, count);
                }
            }
            if !stats.by_event.is_empty() {
                println!("\nBy event:");
                for (event, count) in &stats.by_event {
                    println!("  {}: {}", event, count);
                }
            }
            Ok(())
        }
        FeedbackCmd::Recent { limit } => {
            let failures = kova::feedback::f195(limit);
            if failures.is_empty() {
                println!("No failures recorded.");
                return Ok(());
            }
            for (i, f) in failures.iter().enumerate() {
                println!(
                    "#{} [{}] {} — model={}, event={}",
                    i + 1,
                    f.ts,
                    f.challenge_desc,
                    f.model,
                    f.event_type
                );
            }
            Ok(())
        }
        FeedbackCmd::Export => {
            let failures = kova::feedback::f195(100);
            if failures.is_empty() {
                println!("No failures to generate challenges from.");
                return Ok(());
            }
            let provider = kova::providers::f333();
            let mut challenges = Vec::new();
            for f in &failures {
                match kova::feedback::f196(f, &provider) {
                    Ok(ch) => challenges.push(ch),
                    Err(e) => eprintln!("skip: {}", e),
                }
            }
            if challenges.is_empty() {
                println!("No challenges generated.");
            } else {
                println!("{}", kova::feedback::f197(&challenges));
            }
            Ok(())
        }
    }
}

fn run_export(args: ExportArgs) -> anyhow::Result<()> {
    match args.cmd {
        ExportCmd::Training { format, output } => {
            let fmt = kova::training_data::T117::f316(&format)
                .ok_or_else(|| anyhow::anyhow!("unknown format: {} (expected jsonl, csv, or dpo)", format))?;
            kova::training_data::f181(fmt, output)?;
            Ok(())
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Handle cluster/factory commands synchronously (reqwest::blocking can't run inside tokio)
    match &args.cmd {
        #[cfg(feature = "inference")]
        Some(Cmd::T193(_))
        | Some(Cmd::T181(_))
        | Some(Cmd::Moe(_))
        | Some(Cmd::Academy(_))
        | Some(Cmd::Gauntlet(_))
        | Some(Cmd::Micro(_)) => {
            return match args.cmd.unwrap() {
                Cmd::T193(a) => run_cluster(a),
                Cmd::T181(a) => {
                    let project = a
                        .project
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    let config = kova::factory::T180 {
                        max_fix_retries: a.retries,
                        run_clippy: !a.no_clippy,
                        run_tests: !a.no_tests,
                        run_review: !a.no_review,
                        num_ctx: a.ctx,
                        ..Default::default()
                    };
                    kova::factory::f297(&a.prompt.join(" "), &project, config);
                    Ok(())
                }
                Cmd::Moe(a) => {
                    let config = kova::moe::T196 {
                        num_experts: a.experts,
                        run_clippy: !a.no_clippy,
                        run_tests: !a.no_tests,
                        run_review: !a.no_review,
                        num_ctx: a.ctx,
                        save_winner: a.save,
                    };
                    kova::moe::f341(&a.prompt.join(" "), config);
                    Ok(())
                }
                Cmd::Academy(a) => {
                    let config = kova::academy::T185 {
                        project_dir: a
                            .project
                            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
                        num_experts: a.experts,
                        max_fix_retries: a.retries,
                        num_ctx: a.ctx,
                        auto_commit: !a.no_commit,
                        dry_run: a.dry_run,
                    };
                    kova::academy::f301(&a.task.join(" "), config);
                    Ok(())
                }
                Cmd::Gauntlet(a) => {
                    let phases = if a.phases.is_empty() {
                        None
                    } else {
                        Some(a.phases)
                    };
                    kova::gauntlet::f305(phases);
                    Ok(())
                }
                Cmd::Micro(a) => run_micro(a),
                _ => unreachable!(),
            };
        }
        #[cfg(feature = "inference")]
        Some(Cmd::Review(_))
        | Some(Cmd::Feedback(_)) => {
            return match args.cmd.unwrap() {
                Cmd::Review(a) => run_review(a),
                Cmd::Feedback(a) => run_feedback(a),
                _ => unreachable!(),
            };
        }
        Some(Cmd::Rag(_))
        | Some(Cmd::Traces(_))
        | Some(Cmd::Mcp(_))
        | Some(Cmd::Ci(_))
        | Some(Cmd::Export(_))
        | Some(Cmd::Tokens) => {
            return match args.cmd.unwrap() {
                #[cfg(feature = "rag")]
                Cmd::Rag(a) => run_rag(a),
                Cmd::Traces(a) => run_traces(a),
                Cmd::Mcp(a) => {
                    let project = a
                        .project
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    kova::mcp::f176(&project);
                    Ok(())
                }
                Cmd::Ci(a) => run_ci(a),
                Cmd::Export(a) => run_export(a),
                Cmd::Tokens => {
                    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
                    let report = kova::tokenization::f294(&src);
                    print!("{}", report);
                    if report.ok() { Ok(()) } else {
                        anyhow::bail!("{} untokenized items", report.untokenized.len())
                    }
                }
                _ => unreachable!(),
            };
        }
        _ => {}
    }

    // Everything else runs inside tokio
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(args.cmd))
}

async fn async_main(cmd: Option<Cmd>) -> anyhow::Result<()> {
    #[cfg(feature = "serve")]
    {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

    match cmd {
        Some(Cmd::Gui(args)) => run_gui(args.demo),
        Some(Cmd::Tui(args)) => run_tui(args.project),
        Some(Cmd::Serve(args)) => run_serve(args.open, args.demo).await,
        Some(Cmd::S(args)) => run_serve(true, args.demo).await,
        Some(Cmd::Node) => run_node(),
        Some(Cmd::C2(args)) => run_c2(args).await,
        Some(Cmd::Model(args)) => match args.cmd {
            ModelCmd::Install => {
                #[cfg(feature = "inference")]
                {
                    kova::bootstrap()?;
                    kova::model::f77().await
                }
                #[cfg(not(feature = "inference"))]
                {
                    anyhow::bail!("Build with --features inference for model install")
                }
            }
            ModelCmd::List => {
                kova::bootstrap()?;
                for (role, path) in [
                    ("router", kova::f78(kova::ModelRole::Router)),
                    ("coder", kova::f78(kova::ModelRole::Coder)),
                    ("fix", kova::f78(kova::ModelRole::Fix)),
                ] {
                    match path {
                        Some(p) => eprintln!("  {}: {}", role, p.display()),
                        None => eprintln!("  {}: (not found)", role),
                    }
                }
                eprintln!(
                    "  orchestration: max_fix_retries={} run_clippy={}",
                    kova::orchestration_max_fix_retries(),
                    kova::orchestration_run_clippy()
                );
                Ok(())
            }
        },
        Some(Cmd::Bootstrap) => {
            kova::bootstrap()?;
            eprintln!("Bootstrap complete. ~/.kova/ ready.");
            eprintln!("  prompts: ~/.kova/prompts/");
            eprintln!("  config:  ~/.kova/config.toml");
            Ok(())
        }
        Some(Cmd::Prompts) => {
            kova::bootstrap()?;
            let project = kova::default_project();
            let out = kova::cursor_prompts::f111(&project);
            if out.is_empty() {
                eprintln!(
                    "Prompts disabled (config [cursor] prompts_enabled = false or no rules found)"
                );
            } else {
                print!("{}", out);
            }
            Ok(())
        }
        Some(Cmd::Autopilot(autopilot_args)) => {
            #[cfg(feature = "autopilot")]
            {
                kova::autopilot::run(autopilot_args.prompt.join(" "))
            }
            #[cfg(not(feature = "autopilot"))]
            {
                let _ = autopilot_args;
                anyhow::bail!("Build with --features autopilot for autopilot mode")
            }
        }
        Some(Cmd::Git(args)) => {
            kova::git_cmd::f160(args.cmd, args.count, args.message, args.files, false)
        }
        Some(Cmd::X(args)) => {
            kova::bootstrap()?;
            kova::cargo_cmd::f136(
                args.cmd,
                args.project,
                args.features,
                args.bin,
                args.extra,
                args.all,
                args.chain,
                args.expand,
            )
        }
        #[cfg(feature = "inference")]
        Some(Cmd::Chat(args)) => {
            kova::bootstrap()?;
            kova::repl::f137(args.project)
        }
        #[cfg(not(feature = "inference"))]
        Some(Cmd::Chat(_)) => {
            anyhow::bail!("Build with --features inference for chat mode")
        }
        #[cfg(feature = "tests")]
        Some(Cmd::Test) => run_test(),
        #[cfg(not(feature = "tests"))]
        Some(Cmd::Test) => {
            anyhow::bail!("Build with --features tests for kova test")
        }
        Some(Cmd::Recent(args)) => {
            let project = args
                .project
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let changes = kova::recent_changes::f86(
                &project,
                std::time::Duration::from_secs(args.minutes * 60),
            );
            let out = kova::recent_changes::f87(&changes);
            if out.is_empty() {
                eprintln!("No files modified in last {} minutes.", args.minutes);
            } else {
                print!("{}", out);
            }
            Ok(())
        }
        #[cfg(feature = "inference")]
        Some(Cmd::T193(_))
        | Some(Cmd::T181(_))
        | Some(Cmd::Moe(_))
        | Some(Cmd::Academy(_))
        | Some(Cmd::Gauntlet(_))
        | Some(Cmd::Micro(_))
        | Some(Cmd::Review(_))
        | Some(Cmd::Feedback(_)) => unreachable!("handled before tokio"),
        Some(Cmd::Rag(_))
        | Some(Cmd::Traces(_))
        | Some(Cmd::Mcp(_))
        | Some(Cmd::Ci(_))
        | Some(Cmd::Export(_))
        | Some(Cmd::Tokens) => unreachable!("handled before tokio"),
        None => {
            // Default: TUI (like Claude Code). Fallback: REPL, then GUI.
            #[cfg(feature = "tui")]
            {
                run_tui(None)
            }
            #[cfg(all(not(feature = "tui"), feature = "inference"))]
            {
                kova::bootstrap()?;
                kova::repl::f137(None)
            }
            #[cfg(all(not(feature = "tui"), not(feature = "inference"), feature = "gui"))]
            {
                run_gui(false)
            }
            #[cfg(all(not(feature = "tui"), not(feature = "inference"), not(feature = "gui")))]
            {
                eprintln!("Usage: kova <COMMAND>");
                eprintln!("  kova tui   — terminal UI (requires --features tui)");
                eprintln!("  kova serve — HTTP API (requires --features serve)");
                Ok(())
            }
        }
    }
}

#[cfg(feature = "rag")]
fn run_rag(args: RagArgs) -> anyhow::Result<()> {
    use kova::rag;

    match args.cmd {
        RagCmd::Index { dir } => {
            let dir = std::path::Path::new(&dir).canonicalize()?;
            let store = rag::T200::open(&rag::T200::default_path())?;
            let count = rag::f345(&store, &dir)?;
            println!("{} chunks indexed from {}", count, dir.display());
        }
        RagCmd::Search { query, k } => {
            let store = rag::T200::open(&rag::T200::default_path())?;
            let results = rag::search(&store, &query, k)?;
            if results.is_empty() {
                println!("No results. Run `kova rag index` first.");
            } else {
                for (i, r) in results.iter().enumerate() {
                    println!(
                        "{}. [score: {:.3}] {}:{}-{}",
                        i + 1,
                        r.score,
                        r.chunk.file,
                        r.chunk.lines.0,
                        r.chunk.lines.1
                    );
                    // Show first 3 lines of content
                    let preview: String = r.chunk.text.lines().take(3).collect::<Vec<_>>().join("\n");
                    println!("   {}", preview.replace('\n', "\n   "));
                    println!();
                }
            }
        }
        RagCmd::Stats => {
            let store = rag::T200::open(&rag::T200::default_path())?;
            let stats = store.stats()?;
            println!("RAG Index Stats:");
            println!("  Chunks: {}", stats.total_chunks);
            println!("  Files:  {}", stats.total_files);
            println!("  Dim:    {}", stats.embedding_dim);
        }
        RagCmd::Clear => {
            let store = rag::T200::open(&rag::T200::default_path())?;
            store.clear()?;
            println!("Index cleared.");
        }
        RagCmd::IndexAll => {
            let store = rag::T200::open(&rag::T200::default_path())?;
            let projects = kova::discover_projects();
            if projects.is_empty() {
                println!("No projects found. Run `kova bootstrap` first.");
            } else {
                let mut total = 0;
                for p in &projects {
                    if p.exists() {
                        match rag::f345(&store, p) {
                            Ok(n) => {
                                println!("{}: {} chunks", p.display(), n);
                                total += n;
                            }
                            Err(e) => eprintln!("{}: error: {}", p.display(), e),
                        }
                    } else {
                        eprintln!("{}: not found, skipping", p.display());
                    }
                }
                println!("Total: {} chunks across {} projects", total, projects.len());
            }
        }
        RagCmd::Auto => {
            let store = rag::T200::open(&rag::T200::default_path())?;
            let projects = kova::discover_projects();
            if projects.is_empty() {
                println!("No projects found. Run `kova bootstrap` first.");
            } else {
                let mut total = 0;
                let mut reindexed = 0;
                for p in &projects {
                    if !p.exists() {
                        eprintln!("{}: not found, skipping", p.display());
                        continue;
                    }
                    match rag::f169(&store, p) {
                        Ok(0) => {
                            println!("{}: fresh", p.display());
                        }
                        Ok(n) => {
                            println!("{}: reindexed {} chunks", p.display(), n);
                            total += n;
                            reindexed += 1;
                        }
                        Err(e) => eprintln!("{}: error: {}", p.display(), e),
                    }
                }
                println!(
                    "Done: {} projects reindexed, {} chunks total ({} already fresh)",
                    reindexed,
                    total,
                    projects.len() - reindexed
                );
            }
        }
    }

    Ok(())
}

fn run_traces(args: TracesArgs) -> anyhow::Result<()> {
    match args.cmd {
        TracesCmd::Recent { limit } => kova::trace::f165(limit),
        TracesCmd::Stats => kova::trace::f164(),
    }
    Ok(())
}

fn run_ci(args: CiArgs) -> anyhow::Result<()> {
    match args.cmd {
        CiCmd::Run {
            project,
            no_clippy,
            no_tests,
        } => {
            let dir =
                project.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let config = kova::ci::t114 {
                project_dir: dir.clone(),
                run_clippy: !no_clippy,
                run_tests: !no_tests,
                ..Default::default()
            };
            let result = kova::ci::f177(&dir, &config);
            kova::ci::f180(&result);
            if !result.passed {
                std::process::exit(1);
            }
            Ok(())
        }
        CiCmd::Watch {
            project,
            interval,
            no_clippy,
            no_tests,
        } => {
            let dir =
                project.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let config = kova::ci::t114 {
                project_dir: dir,
                watch_interval_secs: interval,
                run_clippy: !no_clippy,
                run_tests: !no_tests,
            };
            kova::ci::f178(&config)
        }
    }
}