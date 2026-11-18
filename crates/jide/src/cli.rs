use clap::Parser;

/// Command-line arguments for Just IDE (`jide`).
#[derive(Debug, Clone, Parser)]
#[command(
    name = "jide",
    version,
    about = "Just IDE — lightweight, native, AI-free code editor"
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Files or folders to open (`file:LINE[:COL]` supported).
    #[arg(value_name = "PATHS")]
    pub paths: Vec<String>,

    /// Open a new window instead of reusing an existing instance.
    #[arg(long)]
    pub new_window: bool,

    /// Block until opened files are closed.
    #[arg(long)]
    pub wait: bool,

    /// Run headless smoke checks, print JSON report, and exit.
    #[arg(long)]
    pub self_check: bool,

    /// Measure cold startup to first frame in milliseconds and exit.
    #[arg(long)]
    pub bench_startup: bool,

    /// Clear saved session and backup state.
    #[arg(long)]
    pub reset_state: bool,

    /// Internal verification flag: open the window for N seconds without interaction and print repaint count.
    #[arg(long, hide = true, value_name = "SECONDS")]
    pub smoke_idle: Option<u64>,
}
