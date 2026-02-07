# Torrent Directory Comparison

A command-line Java tool that parses `.torrent` files using a custom **Bencode** deserializer and compares the file listing inside the torrent against an actual directory on disk. It highlights which files are **missing**, **extra**, or **matching** — making it easy to verify download completeness or detect extraneous files.

---

## Table of Contents

- [Features](#features)
- [Requirements](#requirements)
- [Building](#building)
- [Usage](#usage)
- [Architecture](#architecture)
  - [Project Structure](#project-structure)
  - [Torrent.java — Main Application](#torrentjava--main-application)
  - [BencodeSerializer.java — Bencode Codec](#bencodeserializerjava--bencode-codec)
- [Design Patterns & Techniques](#design-patterns--techniques)
- [Bencode Format Reference](#bencode-format-reference)
- [License](#license)

---

## Features

| Feature | Description |
|---|---|
| **List torrent contents** | Prints every file path contained in a `.torrent` file |
| **Directory comparison** | Compares torrent file list against a real directory tree |
| **Diff-style output** | Uses `+` / `-` / `=` prefixes to show added, deleted, and matching files |
| **Selective output** | Filter output to show only `+`, `-`, or `=` results (or any combination) |
| **Full Bencode codec** | Custom serializer/deserializer supporting all four Bencode types |
| **Null-safe navigation** | Absent Node pattern prevents `NullPointerException` chains when traversing torrent metadata |
| **Zero dependencies** | Pure Java — no external libraries required |

---

## Requirements

- **Java 11** or later (source and target level are set to Java 11)
- **Apache Ant** (for building via the included `build.xml`)
- Alternatively, any IDE that supports NetBeans project format (e.g. Apache NetBeans)

---

## Building

### With Apache Ant

```bash
# Clean and build
ant clean jar

# The output JAR will be located at:
#   dist/Torrent.jar
```

### With NetBeans IDE

1. Open the project folder in Apache NetBeans.
2. Right-click the project → **Build** (or press `F11`).
3. The compiled JAR is placed in `dist/Torrent.jar`.

### Manual Compilation

```bash
# Compile
javac -d build/classes src/torrent/*.java

# Run directly
java -cp build/classes torrent.Torrent <file.torrent> [directory] [+ - =]
```

---

## Usage

```
java -jar dist/Torrent.jar <file.torrent> [directory [+] [-] [=]]
```

### Parameters

| Parameter | Required | Description |
|---|---|---|
| `file.torrent` | ✅ | Path to the `.torrent` file (must end with `.torrent`) |
| `directory` | ❌ | Path to the directory to compare against the torrent's file list |
| `+` | ❌ | Show files that exist **in the torrent** but **not in the directory** |
| `-` | ❌ | Show files that exist **in the directory** but **not in the torrent** |
| `=` | ❌ | Show files that exist **in both** the directory and the torrent |

> **Note:** If no `+`, `-`, or `=` flags are specified, all three are shown by default.
> The flags must appear in the order `+` `-` `=` if used.

### Examples

```bash
# List all files inside a torrent
java -jar dist/Torrent.jar myfiles.torrent

# Compare torrent against a directory (shows +, -, and = by default)
java -jar dist/Torrent.jar myfiles.torrent /path/to/downloaded/folder

# Show only files missing from disk (need to download)
java -jar dist/Torrent.jar myfiles.torrent /path/to/folder +

# Show only extra files on disk (not in torrent)
java -jar dist/Torrent.jar myfiles.torrent /path/to/folder -

# Show missing AND extra files (skip matches)
java -jar dist/Torrent.jar myfiles.torrent /path/to/folder + -

# Show all three categories
java -jar dist/Torrent.jar myfiles.torrent /path/to/folder + - =
```

### Output Format

Each output line is prefixed with a symbol:

| Prefix | Meaning |
|---|---|
| `+` | File exists in the torrent but **not** in the directory (needs to be downloaded) |
| `-` | File exists in the directory but **not** in the torrent (extra / extraneous file) |
| `=` | File exists in **both** the torrent and the directory (verified match) |

When running without a directory argument, file paths are printed without any prefix.

---

## Architecture

### Project Structure

```
Torrent-Directory-Comparison/
├── build.xml                          # Ant build script
├── nbproject/                         # NetBeans project configuration
│   ├── build-impl.xml                 # Generated Ant build implementation
│   ├── genfiles.properties            # Generated files checksums
│   ├── project.properties             # Project settings (Java 11, main class, etc.)
│   └── project.xml                    # NetBeans project descriptor
├── src/
│   └── torrent/
│       ├── BencodeSerializer.java     # Full Bencode encoder/decoder (337 lines)
│       └── Torrent.java               # Main application & CLI logic (161 lines)
└── README.md
```

**Total source code:** ~498 lines across 2 Java files.

---

### `Torrent.java` — Main Application

**Package:** `torrent`  
**Main class:** `torrent.Torrent`  
**Lines:** 161

This is the entry point and application logic. It contains:

#### Inner Class: `Params`
A data holder for parsed command-line arguments:
- `dir` — directory path to compare against (or `null` for list-only mode)
- `plus` / `minus` / `equal` — boolean flags controlling which output categories to display

#### Key Methods

| Method | Visibility | Description |
|---|---|---|
| `main(String[])` | `public static` | Entry point — parses args, loads torrent, lists or compares |
| `checkParams(String[])` | `private static` | Validates and parses CLI arguments into a `Params` object; returns `null` on invalid input |
| `loadTorrent(String)` | `private static` | Reads a `.torrent` file and deserializes it into a `Node` tree via `BencodeSerializer.unserialize()` |
| `getTorrentFiles(Node)` | `private static` | Navigates the torrent's `info → files` structure, extracts the `path` list from each file entry, and joins path components using the OS file separator |
| `getDirFiles(String)` | `private static` | Recursively scans a directory and returns all file paths relative to the given root |
| `addDirFiles(File, ArrayList)` | `private static` | Recursive helper: traverses subdirectories depth-first, collecting file paths |
| `printComparison(Params, ArrayList)` | `private static` | Core comparison logic — merges two sorted lists and categorizes files into `equal`, `add` (`+`), and `del` (`-`) |
| `help()` | `private static` | Prints usage instructions and exits |
| `error(String)` | `private static` | Prints an error message and exits with code 1 |

#### Comparison Algorithm

The comparison uses a **sorted merge** (similar to the merge step of merge sort):

1. Both the torrent file list and directory file list are sorted lexicographically.
2. Two pointers (`dirIdx`, `torIdx`) advance through the lists simultaneously.
3. At each step, the current elements are compared:
   - **Equal** → file exists in both → added to `equalfiles`
   - **Torrent < Directory** → file is in torrent but missing from disk → added to `addfiles`
   - **Directory < Torrent** → file is on disk but not in torrent → added to `delfiles`

**Time complexity:** O(n log n) for sorting + O(n) for the merge pass, where n is the total number of files.

---

### `BencodeSerializer.java` — Bencode Codec

**Package:** `torrent`  
**Lines:** 337

A complete, self-contained Bencode serializer and deserializer. The class handles all four Bencode data types:

| Bencode Type | Java Representation | Encoding |
|---|---|---|
| Integer | `long` | `i<number>e` (e.g., `i42e`) |
| Byte String | `byte[]` | `<length>:<data>` (e.g., `4:spam`) |
| List | `ArrayList<Node>` | `l<items>e` |
| Dictionary | `TreeMap<String, Node>` | `d<key><value>...e` (keys sorted by raw bytes) |

#### Serialization (Java → Bencode)

The serializer uses a **fluent API** pattern with chained `write()` methods.

**Key components:**

- **`BencodeSerializer`** — main class, wraps an `OutputStream` and provides overloaded `write()` methods for each type
- **`BencodeList`** — functional interface for objects that can serialize themselves as Bencode lists
- **`BencodeDictionary`** — functional interface for objects that can serialize themselves as Bencode dictionaries
- **`Fields`** — accumulates dictionary key-value pairs in a `TreeMap<byte[], Runnable>` to ensure keys are emitted in sorted byte order per the Bencode spec

**Dictionary key sorting:** The `Fields` class uses a `TreeMap<byte[], Runnable>` to buffer all field entries. On `export()`, fields are written in sorted key order. Values are stored as `Runnable` lambdas to defer serialization until sort order is determined.

#### Deserialization (Bencode → Java)

The `unserialize(InputStream)` method is a **recursive descent parser** that reads one byte to determine the type:

| First Byte | Type | Parsing Strategy |
|---|---|---|
| `'i'` | Integer | Read digits until `'e'`, parse as `Long` |
| `'l'` | List | Recursively unserialize items until `'e'` |
| `'d'` | Dictionary | Recursively unserialize key-value pairs until `'e'` |
| `'0'`–`'9'` | Byte String | Parse length, read `:`, then read exactly N bytes |
| `'e'` | Terminator | Returns `null` to signal end of list/dict |

#### Node Interface (Null Object Pattern)

The `Node` interface provides safe, chainable access to deserialized data with no risk of `NullPointerException`:

```java
// Safe navigation — no null checks needed:
Node files = torrent.getField("info").getField("files");
if (files.isList()) { ... }
```

**Node implementations:**

| Class | Type | Key Behavior |
|---|---|---|
| `AbsentNode` | Missing data | `isExist()` returns `false`; all getters return safe defaults |
| `IntegerNode` | `long` | `isInteger()` → `true`, `getInteger()` → the value |
| `ByteArrayNode` | `byte[]` | `isByteArray()` → `true`, `getString()` → UTF-8 decoded, `getByteArray()` → raw bytes |
| `ListNode` | `ArrayList<Node>` | `isList()` → `true`, `getList()` → the list |
| `DictionaryNode` | `TreeMap<String, Node>` | `isDictionary()` → `true`, `getField(name)` → value or `ABSENT_NODE` |

**Singleton pattern:** A single `ABSENT_NODE` instance is shared across all missing-field lookups.

**Safe Array:** The inner `Array` class extends `ArrayList<Node>` and overrides `get(int)` to return `ABSENT_NODE` instead of throwing `IndexOutOfBoundsException`.

#### Custom Exception

`FormatException` — thrown when the Bencode data is malformed (bad integer format, missing dictionary value, unexpected EOF, etc.).

---

## Design Patterns & Techniques

| Pattern | Where Used | Purpose |
|---|---|---|
| **Null Object** | `AbsentNode` / `ABSENT_NODE` | Eliminates null checks when navigating nested torrent structures |
| **Recursive Descent Parser** | `unserialize()` | Parses Bencode format by dispatching on the first byte |
| **Fluent API** | `BencodeSerializer.write()` | Enables chained serialization calls (`s.write("a").write(42)`) |
| **Deferred Execution** | `Fields.Runnable` lambdas | Dictionary values are serialized only after all keys are collected and sorted |
| **Sorted Merge** | `printComparison()` | O(n) comparison of two sorted file lists |
| **Functional Interfaces** | `BencodeList`, `BencodeDictionary` | Allow any object to define its own Bencode serialization |
| **Type-safe Node hierarchy** | `Node` interface + implementations | Each Bencode type has a dedicated class with typed accessors |

---

## Bencode Format Reference

[Bencode](https://wiki.theory.org/BitTorrentSpecification#Bencoding) is the encoding format used by BitTorrent for `.torrent` files and tracker communication.

```
Integers:     i<integer>e             → i42e
Byte Strings: <length>:<contents>     → 4:spam
Lists:        l<items>e               → l4:spam4:eggse
Dictionaries: d<key><value>...e       → d3:cow3:moo4:spam4:eggse
```

**Dictionary keys** must be byte strings and must appear in **sorted order** (by raw byte value).

### Torrent File Structure (relevant fields)

```
{
  "info": {
    "files": [
      {
        "length": <file size>,
        "path": ["dir1", "dir2", "filename.ext"]
      },
      ...
    ]
  }
}
```

This tool reads `info → files → path` to reconstruct the full path of each file within the torrent.

---

## License

This project does not currently include a license file. Please contact the repository owner for licensing information.
