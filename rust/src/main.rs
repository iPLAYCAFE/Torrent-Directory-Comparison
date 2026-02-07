//! zDirComp — Torrent Directory Comparison & Cleanup Tool
//!
//! Two modes:
//!   sync   <torrent_file> <directory>  — delete extra files not in torrent
//!   unlock <directory>                 — kill processes locking files (except uTorrent/BitTorrent)

#![windows_subsystem = "windows"]

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
        logger::log("ERROR: No command specified. Usage: zDirComp.exe sync|unlock ...");
        process::exit(1);
    }

    let command = args[1].to_lowercase();

    match command.as_str() {
        "sync" => {
            if args.len() < 4 {
                logger::log("ERROR: sync requires 2 arguments: <torrent_file> <directory>");
                process::exit(1);
            }
            sync::run(&args[2], &args[3]);
        }
        "unlock" => {
            if args.len() < 3 {
                logger::log("ERROR: unlock requires 1 argument: <directory>");
                process::exit(1);
            }
            unlock::run(&args[2]);
        }
        _ => {
            logger::log(&format!("ERROR: Unknown command '{}'. Use 'sync' or 'unlock'.", command));
            process::exit(1);
        }
    }
}
