# uTorrent / BitTorrent Integration Guide

This document explains how to integrate **Torrent Directory Comparison** with uTorrent (or BitTorrent) using the **"Run Program"** feature to automatically clean up empty directories left behind after download completion.

---

## Overview

When a torrent finishes downloading, uTorrent can execute a custom command. This integration uses that hook to:

1. **Parse** the `.torrent` file using `zDirComp.jar`
2. **Compare** the torrent's file list against the download directory
3. **Identify** files that exist on disk but are **not** in the torrent (extra/extraneous files)
4. **Generate** a cleanup batch script (`zDirComp.cmd`) that deletes those extra files
5. **Execute** the cleanup script
6. **Remove** any empty directories left behind after deletion

---

## Setup Location

The tool expects the following file layout:

```
%localappdata%\AutoSync\
├── JRE\
│   ├── bin\
│   │   └── java.exe          ← Portable JRE (Java Runtime)
│   └── zDirComp.jar          ← Compiled Torrent Directory Comparison tool
└── BitTorrent\
    └── *.torrent              ← uTorrent's torrent file storage directory
```

> **Note:** Using a portable JRE under `%localappdata%\AutoSync\JRE\` makes this setup self-contained — no system-wide Java installation required.

### uTorrent Torrent File Storage

For this integration to work, uTorrent must be configured to store `.torrent` files at:

```
%localappdata%\AutoSync\BitTorrent\
```

This is configured in uTorrent under:  
**Preferences → Directories → Store .torrents in:**

---

## Configuration

### Step-by-step

1. Open uTorrent → **Preferences** (Ctrl+P)
2. Navigate to **Advanced → Run Program**
3. In the **"Run this program when a torrent finishes:"** field, paste the command below
4. Click **OK**

### The Command

```bat
cmd /c @echo off && timeout 3 /nobreak && "%localappdata%\AutoSync\JRE\bin\java.exe" -jar "%localappdata%\AutoSync\JRE\zDirComp.jar" "%localappdata%\AutoSync\BitTorrent\%N.torrent" "%D" - > "%D\zDirComp.cmd" && "%D\zDirComp.cmd" ||  for /f "usebackq delims=" %d in (`"dir "%D" /ad/b/s | sort /R"`) do rd "%d"
```

---

## Technical Breakdown

The command is a single-line batch pipeline consisting of **5 stages** chained with `&&` (execute next only if previous succeeds) and `||` (execute on failure / fallback):

### Stage 1 — Silent Mode

```bat
cmd /c @echo off
```

| Part | Purpose |
|---|---|
| `cmd /c` | Launches a new command interpreter to execute the full pipeline |
| `@echo off` | Suppresses echoing of subsequent commands to keep the console clean |

### Stage 2 — Startup Delay

```bat
timeout 3 /nobreak
```

| Part | Purpose |
|---|---|
| `timeout 3` | Waits **3 seconds** before proceeding |
| `/nobreak` | Prevents the user from skipping the wait with a keypress |

**Why the delay?** When uTorrent fires the "torrent finished" event, file handles may still be held by the torrent client. The 3-second delay ensures uTorrent has fully released all file locks before the comparison tool tries to read the directory.

### Stage 3 — Run Directory Comparison

```bat
"%localappdata%\AutoSync\JRE\bin\java.exe" -jar "%localappdata%\AutoSync\JRE\zDirComp.jar" "%localappdata%\AutoSync\BitTorrent\%N.torrent" "%D" - > "%D\zDirComp.cmd"
```

Breaking this down:

| Part | Purpose |
|---|---|
| `java.exe -jar zDirComp.jar` | Runs the Torrent Directory Comparison JAR |
| `%N.torrent` | **uTorrent variable** — expands to the torrent's title + `.torrent` extension |
| `"%D"` | **uTorrent variable** — expands to the download directory path |
| `-` | Flag: output only files that exist **in the directory but not in the torrent** (extra files) |
| `> "%D\zDirComp.cmd"` | **Redirects stdout** to a `.cmd` file inside the download directory |

**What gets generated:** The `-` flag makes `zDirComp.jar` output lines prefixed with `-`, each representing an extra file on disk. The output redirect (`>`) writes these lines into `zDirComp.cmd`.

**Example `zDirComp.cmd` content:**

```
-Thumbs.db
-desktop.ini
-Some Extra File.txt
```

> **Note:** Lines starting with `-` are not valid batch commands, so if there are extra files the `.cmd` file will produce errors (which triggers the `||` fallback for cleanup). If there are _no_ extra files, the file will be empty and execute successfully (as a no-op).

### Stage 4 — Execute Cleanup Script

```bat
"%D\zDirComp.cmd"
```

Executes the generated batch script. Since lines starting with `-` are not recognizable commands, `cmd.exe` will report errors — this is by design. The `&&` chain means this stage's exit code determines the next action:

| Result | Exit Code | Next Action |
|---|---|---|
| No extra files (empty script) | `0` (success) | Pipeline ends — nothing to clean |
| Extra files found (invalid commands) | Non-zero (error) | Falls through to `||` Stage 5 |

### Stage 5 — Remove Empty Directories (Fallback)

```bat
for /f "usebackq delims=" %d in (`"dir "%D" /ad/b/s | sort /R"`) do rd "%d"
```

This runs only when Stage 4 "fails" (i.e., extra files were detected). It cleans up any empty directories:

| Part | Purpose |
|---|---|
| `dir "%D" /ad/b/s` | Lists all subdirectories (`/ad`) in bare format (`/b`), recursively (`/s`) |
| `sort /R` | Sorts in **reverse** order — deepest directories first |
| `for /f ... %d in (...)` | Iterates over each directory path |
| `rd "%d"` | Attempts to remove the directory — `rd` only succeeds on **empty** directories |

**Why reverse sort?** Directories must be deleted deepest-first. If a parent directory is attempted before its children are removed, `rd` will fail because the directory is not empty. Reverse sorting ensures leaf directories are processed first.

**Safe behavior:** `rd` (without `/s`) only removes **empty** directories. Non-empty directories are silently skipped, so no data is ever lost.

---

## Execution Flow Diagram

```
uTorrent fires "torrent finished"
        │
        ▼
   ┌─────────────┐
   │ 3s delay     │ ← Wait for file locks to release
   └──────┬──────┘
          ▼
   ┌──────────────────────────────┐
   │ java -jar zDirComp.jar      │
   │   torrent: %N.torrent       │
   │   directory: %D             │
   │   mode: "-" (extra files)   │
   │   output → zDirComp.cmd     │
   └──────┬───────────────────────┘
          ▼
   ┌─────────────────────────┐
   │ Execute zDirComp.cmd    │
   └──────┬──────────┬───────┘
          │          │
     No extras    Has extras
     (empty file)  (invalid cmds)
          │          │
          ▼          ▼
       ✅ Done   ┌─────────────────────┐
                 │ for each subdir      │
                 │   (deepest first)    │
                 │   rd "%d"            │
                 │   (remove if empty)  │
                 └──────┬──────────────┘
                        ▼
                     ✅ Done
```

---

## uTorrent Variables Reference

These variables are available in the **"Run Program"** fields and are expanded by uTorrent at runtime:

| Variable | Description | Example Value |
|---|---|---|
| `%F` | Name of downloaded file (single-file torrents only) | `movie.mkv` |
| `%D` | Directory where files are saved | `E:\Downloads\My Torrent` |
| `%N` | Title of torrent | `My Torrent` |
| `%P` | Previous state of torrent (numeric) | `6` |
| `%L` | Label assigned to the torrent | `Movies` |
| `%T` | Tracker URL | `http://tracker.example.com` |
| `%M` | Status message string | `Seeding` |
| `%I` | Hex-encoded info-hash | `A1B2C3D4E5...` |
| `%S` | Current state of torrent (numeric) | `11` |
| `%K` | Kind of torrent | `single` or `multi` |

### Torrent State Codes

| Code | State | Description |
|---|---|---|
| 1 | Error | An error occurred |
| 2 | Checked | Hash check complete |
| 3 | Paused | Manually paused |
| 4 | Super seeding | Super-seed mode active |
| 5 | Seeding | Uploading to peers |
| 6 | Downloading | Actively downloading |
| 7 | Super seed [F] | Super-seed, forced |
| 8 | Seeding [F] | Seeding, forced |
| 9 | Downloading [F] | Downloading, forced |
| 10 | Queued seed | Queued for seeding |
| 11 | Finished | Download complete |
| 12 | Queued | Queued for download |
| 13 | Stopped | Manually stopped |
| 17 | Preallocating | Allocating disk space |
| 18 | Downloading Metadata | Fetching torrent metadata (magnet) |
| 19 | Connecting to Peers | Establishing peer connections |
| 20 | Moving | Moving files to new location |
| 21 | Flushing | Flushing disk cache |
| 22 | Need DHT | Waiting for DHT |
| 23 | Finding Peers | Peer discovery in progress |
| 24 | Resolving | DNS resolution |
| 25 | Writing | Writing to disk |

---

## Troubleshooting

| Issue | Cause | Fix |
|---|---|---|
| Command doesn't run | uTorrent setting not saved | Verify **Preferences → Advanced → Run Program** is populated and click **OK** (not just Apply) |
| `.torrent` file not found | Torrent storage directory mismatch | Ensure **Preferences → Directories → Store .torrents in** points to `%localappdata%\AutoSync\BitTorrent\` |
| `java.exe` not found | JRE not at expected path | Verify `%localappdata%\AutoSync\JRE\bin\java.exe` exists |
| Cleanup doesn't run | No extra files exist | This is normal — if all files match, no cleanup is needed |
| Files not deleted | Only empty directories are removed | `rd` without `/s` only removes empty directories — this is a safety feature |