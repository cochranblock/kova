// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Kova — augment engine. GUI + serve.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kova", version)]
struct Args {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run native GUI (egui).
    Gui(GuiArgs),
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
    #[command(name = "cluster")]
    Cluster(ClusterArgs),
    /// Rust Binary Factory. Full pipeline: classify → generate → compile → review → fix.
    #[command(name = "factory")]
    Factory(FactoryArgs),
    /// Mixture of Experts. Fan-out to N nodes, compile all, score, pick winner.
    #[command(name = "moe")]
    Moe(MoeArgs),
    /// Academy. MoE-powered autonomous dev agent. Plan → generate → wire → test → fix → commit.
    #[command(name = "academy")]
    Academy(AcademyArgs),
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
        token: kova::c2::Token,
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

#[derive(clap::Args)]
struct ClusterArgs {
    #[command(subcommand)]
    cmd: ClusterCmd,
}

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
    anyhow::bail!("Build with --features gui for GUI mode")
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
    kova::run_test_suite()
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
            kova::c2::run_nodes();
            Ok(())
        }
        C2Cmd::Run {
            token,
            project,
            broadcast,
            release,
            nodes,
            local,
        } => kova::c2::run_command(token, project, broadcast, release, nodes, local),
        C2Cmd::Inspect { json } => {
            let hosts = kova::inspect::run_inspect();
            if json {
                kova::inspect::print_json(&hosts);
            } else {
                kova::inspect::print_table(&hosts);
            }
            Ok(())
        }
        C2Cmd::Recommend => {
            let hosts = kova::inspect::run_inspect();
            kova::inspect::print_table(&hosts);
            kova::inspect::print_recommend(&hosts);
            Ok(())
        }
        C2Cmd::Build {
            broadcast,
            release,
            no_sync,
            local,
            nodes,
            project,
        } => kova::c2::run_build(broadcast, release, no_sync, local, nodes, project),
        C2Cmd::Sync {
            dry_run,
            target,
            local,
            all,
            full,
        } => kova::c2::run_sync(dry_run, &target, local, all, full),
        C2Cmd::SshCa { cmd } => match cmd {
            SshCaCmd::Init => kova::ssh_ca::run_init(),
            SshCaCmd::Sign { node } => kova::ssh_ca::run_sign(&node),
            SshCaCmd::Setup => kova::ssh_ca::run_setup(),
        },
        C2Cmd::Ncmd { cmd, nodes, idle, extra, release, lines, expand, oneline } => {
            let nodes_opt = if idle {
                let all: Vec<String> = kova::c2::default_nodes().into_iter().map(String::from).collect();
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

fn run_cluster(args: ClusterArgs) -> anyhow::Result<()> {
    let cluster = kova::cluster::Cluster::default_hive();
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
        ClusterCmd::Gen { prompt, system, ctx } => {
            let prompt = prompt.join(" ");
            let system = system.unwrap_or_else(|| {
                "You are a Rust systems programming expert. Write clean, idiomatic Rust. No filler.".into()
            });
            println!("[cluster] dispatching code gen...");
            match cluster.dispatch(kova::cluster::TaskKind::CodeGen, &system, &prompt, Some(ctx)) {
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
            match cluster.dispatch(kova::cluster::TaskKind::CodeReview, system, &code, Some(8192)) {
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
            match kova::cluster::quick_fix(
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
            let prompt = "Write a Rust function that computes the nth Fibonacci number iteratively.";
            let system = "You are a Rust expert. Write clean code. No explanation.";

            let handles: Vec<_> = cluster.nodes.iter().map(|node| {
                let url = node.base_url();
                let model = node.model.clone();
                let id = node.id.clone();
                std::thread::spawn(move || {
                    let start = std::time::Instant::now();
                    let result = kova::ollama::generate(&url, &model, system, prompt, Some(4096));
                    let elapsed = start.elapsed();
                    (id, model, result, elapsed)
                })
            }).collect();

            for h in handles {
                if let Ok((id, model, result, elapsed)) = h.join() {
                    match result {
                        Ok(resp) => {
                            let tokens = resp.split_whitespace().count(); // rough estimate
                            let tps = tokens as f64 / elapsed.as_secs_f64();
                            println!("  {} ({}) — {:.1}s, ~{} tokens, ~{:.1} tok/s", id, model, elapsed.as_secs_f64(), tokens, tps);
                        }
                        Err(e) => println!("  {} ({}) — FAILED: {}", id, model, e),
                    }
                }
            }
            Ok(())
        }
        ClusterCmd::Models => {
            for node in &cluster.nodes {
                let url = node.base_url();
                print!("  {} — ", node.id);
                match kova::ollama::list_models(&url) {
                    Ok(models) => {
                        let names: Vec<_> = models.iter().map(|m| {
                            format!("{} ({:.1}GB)", m.name, m.size as f64 / 1_073_741_824.0)
                        }).collect();
                        println!("{}", names.join(", "));
                    }
                    Err(e) => println!("OFFLINE ({})", e),
                }
            }
            Ok(())
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Handle cluster/factory commands synchronously (reqwest::blocking can't run inside tokio)
    match &args.cmd {
        Some(Cmd::Cluster(_)) | Some(Cmd::Factory(_)) | Some(Cmd::Moe(_)) | Some(Cmd::Academy(_)) => {
            return match args.cmd.unwrap() {
                Cmd::Cluster(a) => run_cluster(a),
                Cmd::Factory(a) => {
                    let project = a.project.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    let config = kova::factory::FactoryConfig {
                        max_fix_retries: a.retries,
                        run_clippy: !a.no_clippy,
                        run_tests: !a.no_tests,
                        run_review: !a.no_review,
                        num_ctx: a.ctx,
                        ..Default::default()
                    };
                    kova::factory::run_factory(&a.prompt.join(" "), &project, config);
                    Ok(())
                }
                Cmd::Moe(a) => {
                    let config = kova::moe::MoeConfig {
                        num_experts: a.experts,
                        run_clippy: !a.no_clippy,
                        run_tests: !a.no_tests,
                        run_review: !a.no_review,
                        num_ctx: a.ctx,
                        save_winner: a.save,
                    };
                    kova::moe::run_moe(&a.prompt.join(" "), config);
                    Ok(())
                }
                Cmd::Academy(a) => {
                    let config = kova::academy::AcademyConfig {
                        project_dir: a.project.unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
                        num_experts: a.experts,
                        max_fix_retries: a.retries,
                        num_ctx: a.ctx,
                        auto_commit: !a.no_commit,
                        dry_run: a.dry_run,
                    };
                    kova::academy::run_academy(&a.task.join(" "), config);
                    Ok(())
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
                eprintln!("  orchestration: max_fix_retries={} run_clippy={}",
                    kova::orchestration_max_fix_retries(),
                    kova::orchestration_run_clippy());
                Ok(())
            }
        }
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
            let out = kova::cursor_prompts::load_cursor_prompts(&project);
            if out.is_empty() {
                eprintln!("Prompts disabled (config [cursor] prompts_enabled = false or no rules found)");
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
                args.cmd, args.project, args.features, args.bin,
                args.extra, args.all, args.chain, args.expand,
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
        Some(Cmd::Cluster(_)) | Some(Cmd::Factory(_)) | Some(Cmd::Moe(_)) | Some(Cmd::Academy(_)) => unreachable!("handled before tokio"),
        None => {
            // Default: REPL (like Claude Code). Fallback: GUI.
            #[cfg(feature = "inference")]
            {
                kova::bootstrap()?;
                kova::repl::f137(None)
            }
            #[cfg(all(not(feature = "inference"), feature = "gui"))]
            {
                run_gui(false)
            }
            #[cfg(all(not(feature = "inference"), not(feature = "gui")))]
            {
                eprintln!("Usage: kova <COMMAND>");
                eprintln!("  kova chat  — interactive REPL (requires --features inference)");
                eprintln!("  kova gui   — native GUI (requires --features gui)");
                eprintln!("  kova serve — HTTP API (requires --features serve)");
                Ok(())
            }
        }
    }
}
