// CLI argument parsing using clap derive macros

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "video-clip-extractor")]
#[command(about = "Extract thematic clips from video files", long_about = None)]
pub struct CliArgs {
    /// Directory path to scan for videos
    #[arg(value_name = "PATH")]
    pub directory: PathBuf,

    /// Clip selection strategy
    #[arg(short = 's', long = "strategy", value_enum, default_value = "random")]
    pub strategy: SelectionStrategy,

    /// Output resolution
    #[arg(short = 'r', long = "resolution", value_enum, default_value = "1080p")]
    pub resolution: Resolution,

    /// Include audio in output clip
    #[arg(short = 'a', long = "audio", default_value = "true")]
    pub include_audio: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SelectionStrategy {
    Random,
    IntenseAudio,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum Resolution {
    #[value(name = "720p")]
    Hd720,
    #[value(name = "1080p")]
    Hd1080,
}
