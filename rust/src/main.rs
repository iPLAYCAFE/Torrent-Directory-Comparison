//! zDirComp — Torrent Directory Comparison & Cleanup Tool
//!
//! Two modes:
//!   sync   <torrent_file> <directory>  — delete extra files not in torrent
//!   unlock <directory>                 — kill all processes locking files (RmForceShutdown)

mod bencode;
mod logger;
mod safety;
mod sync;
mod unlock;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("zDirComp — Torrent Directory Comparison & Cleanup Tool");
        eprintln!();
        eprintln!("Usage:");
        eprintln!("  zDirComp.exe sync   <torrent_file> <directory>  — delete extra files");
        eprintln!("  zDirComp.exe unlock <directory>                 — kill locking processes");
        process::exit(1);
    }

    let command = args[1].to_lowercase();

    match command.as_str() {
        "sync" => {
            if args.len() < 4 {
                eprintln!("Error: sync requires 2 arguments: <torrent_file> <directory>");
                logger::log("ERROR: sync requires 2 arguments: <torrent_file> <directory>");
                process::exit(1);
            }
            sync::run(&args[2], &args[3]);
        }
        "unlock" => {
            if args.len() < 3 {
                eprintln!("Error: unlock requires 1 argument: <directory>");
                logger::log("ERROR: unlock requires 1 argument: <directory>");
                process::exit(1);
            }
            unlock::run(&args[2]);
        }
        _ => {
            eprintln!("Error: Unknown command '{}'. Use 'sync' or 'unlock'.", command);
            logger::log(&format!("ERROR: Unknown command '{}'. Use 'sync' or 'unlock'.", command));
            process::exit(1);
        }
    }
}
