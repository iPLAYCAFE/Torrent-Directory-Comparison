//! Bencode parser — recursive descent from byte slice.
//!
//! Supports all four Bencode types: Integer, ByteString, List, Dictionary.
//! Ported from BencodeSerializer.java.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A Bencode value.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BValue {
    Integer(i64),
    Bytes(Vec<u8>),
    List(Vec<BValue>),
    Dict(BTreeMap<Vec<u8>, BValue>),
}

impl BValue {
    /// Get as dictionary field by key.
    pub fn field(&self, key: &[u8]) -> Option<&BValue> {
        match self {
            BValue::Dict(map) => map.get(key),
            _ => None,
        }
    }

    /// Get as list.
    pub fn as_list(&self) -> Option<&[BValue]> {
        match self {
            BValue::List(v) => Some(v),
            _ => None,
        }
    }

    /// Get as byte string.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            BValue::Bytes(v) => Some(v),
            _ => None,
        }
    }

    /// Get as UTF-8 string (lossy).
    pub fn as_str_lossy(&self) -> Option<String> {
        self.as_bytes()
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }
}

/// Parse error.
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bencode parse error: {}", self.0)
    }
}

/// Parse a Bencode value from a byte slice.
/// Returns the parsed value and the remaining unparsed bytes.
pub fn parse(data: &[u8]) -> Result<(BValue, &[u8]), ParseError> {
    if data.is_empty() {
        return Err(ParseError("Unexpected end of data".to_string()));
    }

    match data[0] {
        // Integer: i<number>e
        b'i' => {
            let rest = &data[1..];
            let end = rest
                .iter()
                .position(|&b| b == b'e')
                .ok_or_else(|| ParseError("Integer: missing 'e'".to_string()))?;
            let num_str = std::str::from_utf8(&rest[..end])
                .map_err(|_| ParseError("Integer: invalid UTF-8".to_string()))?;
            let num = num_str
                .parse::<i64>()
                .map_err(|_| ParseError(format!("Integer: bad format '{}'", num_str)))?;
            Ok((BValue::Integer(num), &rest[end + 1..]))
        }

        // List: l<items>e
        b'l' => {
            let mut rest = &data[1..];
            let mut items = Vec::new();
            while !rest.is_empty() && rest[0] != b'e' {
                let (val, remaining) = parse(rest)?;
                items.push(val);
                rest = remaining;
            }
            if rest.is_empty() {
                return Err(ParseError("List: missing 'e'".to_string()));
            }
            Ok((BValue::List(items), &rest[1..]))
        }

        // Dictionary: d<key><value>...e
        b'd' => {
            let mut rest = &data[1..];
            let mut map = BTreeMap::new();
            while !rest.is_empty() && rest[0] != b'e' {
                let (key, remaining) = parse(rest)?;
                let key_bytes = match key {
                    BValue::Bytes(b) => b,
                    _ => {
                        return Err(ParseError(
                            "Dictionary: key must be byte string".to_string(),
                        ))
                    }
                };
                if remaining.is_empty() {
                    return Err(ParseError("Dictionary: missing value".to_string()));
                }
                let (val, remaining) = parse(remaining)?;
                map.insert(key_bytes, val);
                rest = remaining;
            }
            if rest.is_empty() {
                return Err(ParseError("Dictionary: missing 'e'".to_string()));
            }
            Ok((BValue::Dict(map), &rest[1..]))
        }

        // Byte string: <length>:<data>
        b'0'..=b'9' => {
            let colon = data
                .iter()
                .position(|&b| b == b':')
                .ok_or_else(|| ParseError("String: missing ':'".to_string()))?;
            let len_str = std::str::from_utf8(&data[..colon])
                .map_err(|_| ParseError("String: length not UTF-8".to_string()))?;
            let len = len_str
                .parse::<usize>()
                .map_err(|_| ParseError(format!("String: bad length '{}'", len_str)))?;
            let start = colon + 1;
            if start + len > data.len() {
                return Err(ParseError(format!(
                    "String: expected {} bytes but only {} available",
                    len,
                    data.len() - start
                )));
            }
            let bytes = data[start..start + len].to_vec();
            Ok((BValue::Bytes(bytes), &data[start + len..]))
        }

        other => Err(ParseError(format!(
            "Unexpected byte '{}' (0x{:02x})",
            other as char, other
        ))),
    }
}

/// Extract file paths from a torrent's Bencode data.
///
/// Reads `info.files[].path` for multi-file torrents, or `info.name` for single-file.
/// Returns relative paths using OS path separators.
pub fn torrent_files(data: &[u8]) -> Result<Vec<PathBuf>, ParseError> {
    let (root, _) = parse(data)?;
    let info = root
        .field(b"info")
        .ok_or_else(|| ParseError("Missing 'info' dictionary".to_string()))?;

    // Multi-file torrent: info.files
    if let Some(files) = info.field(b"files") {
        let file_list = files
            .as_list()
            .ok_or_else(|| ParseError("'files' is not a list".to_string()))?;

        let mut paths = Vec::with_capacity(file_list.len());

        for file_entry in file_list {
            let path_list = file_entry
                .field(b"path")
                .and_then(|p| p.as_list())
                .ok_or_else(|| ParseError("File entry missing 'path' list".to_string()))?;

            let mut file_path = PathBuf::new();
            for component in path_list {
                let name = component
                    .as_str_lossy()
                    .ok_or_else(|| ParseError("Path component is not a string".to_string()))?;
                file_path.push(&name);
            }
            paths.push(file_path);
        }

        Ok(paths)
    }
    // Single-file torrent: info.name
    else if let Some(name) = info.field(b"name").and_then(|n| n.as_str_lossy()) {
        Ok(vec![PathBuf::from(name)])
    } else {
        Err(ParseError(
            "No 'files' or 'name' found in torrent info".to_string(),
        ))
    }
}

/// Parse a torrent file from disk and extract file paths.
pub fn parse_torrent_file(path: &Path) -> Result<Vec<PathBuf>, String> {
    let data = std::fs::read(path).map_err(|e| format!("Cannot read torrent file: {}", e))?;
    torrent_files(&data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_integer() {
        let (val, rest) = parse(b"i42e").unwrap();
        assert!(matches!(val, BValue::Integer(42)));
        assert!(rest.is_empty());
    }

    #[test]
    fn test_parse_string() {
        let (val, rest) = parse(b"4:spam").unwrap();
        assert_eq!(val.as_bytes().unwrap(), b"spam");
        assert!(rest.is_empty());
    }

    #[test]
    fn test_parse_list() {
        let (val, rest) = parse(b"l4:spam4:eggse").unwrap();
        let list = val.as_list().unwrap();
        assert_eq!(list.len(), 2);
        assert!(rest.is_empty());
    }

    #[test]
    fn test_parse_dict() {
        let (val, rest) = parse(b"d3:cow3:moo4:spam4:eggse").unwrap();
        assert!(matches!(val, BValue::Dict(_)));
        assert_eq!(val.field(b"cow").unwrap().as_bytes().unwrap(), b"moo");
        assert!(rest.is_empty());
    }

    #[test]
    fn test_multi_file_torrent() {
        let data = b"d4:infod5:filesld6:lengthi100e4:pathl9:file1.txteed6:lengthi200e4:pathl6:SubDir9:file2.txteeeee";
        let files = torrent_files(data).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0], PathBuf::from("file1.txt"));
        assert_eq!(files[1], PathBuf::from("SubDir").join("file2.txt"));
    }

    #[test]
    fn test_single_file_torrent() {
        let data = b"d4:infod4:name9:file1.txtee";
        let files = torrent_files(data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], PathBuf::from("file1.txt"));
    }
}

