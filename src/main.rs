// CLI entry point and application orchestration

mod cli;
mod scanner;
mod selector;
mod ffmpeg;
mod processor;
mod progress;
mod error;

use clap::Parser;
use cli::CliArgs;

fn main() {
    let args = CliArgs::parse();
    
    println!("Video Clip Extractor");
    println!("Directory: {}", args.directory.display());
    println!("Strategy: {:?}", args.strategy);
    println!("Resolution: {:?}", args.resolution);
    println!("Include audio: {}", args.include_audio);
}

