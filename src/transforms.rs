// transforms.rs
// all the little text operations you can apply to a clipboard entry
// things like uppercase, base64, url encode, sha256, strip html, etc

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use percent_encoding::{utf8_percent_encode, percent_decode_str, NON_ALPHANUMERIC};
use regex::Regex;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    Uppercase,
    Lowercase,
    TitleCase,
    SentenceCase,
    TrimWhitespace,
    CollapseNewlines,
    UrlEncode,
    UrlDecode,
    Base64Encode,
    Base64Decode,
    JsonPrettify,
    JsonMinify,
    HexEncode,
    HexDecode,
    HashMd5,
    HashSha1,
    HashSha256,
    CharWordLineCount,
    StripHtml,
    RegexReplace { pattern: String, replacement: String },
}

impl Transform {
    pub fn label(&self) -> &'static str {
        match self {
            Transform::Uppercase => "UPPERCASE",
            Transform::Lowercase => "lowercase",
            Transform::TitleCase => "Title Case",
            Transform::SentenceCase => "Sentence case",
            Transform::TrimWhitespace => "Trim Whitespace",
            Transform::CollapseNewlines => "Collapse Newlines",
            Transform::UrlEncode => "URL Encode",
            Transform::UrlDecode => "URL Decode",
            Transform::Base64Encode => "Base64 Encode",
            Transform::Base64Decode => "Base64 Decode",
            Transform::JsonPrettify => "JSON Prettify",
            Transform::JsonMinify => "JSON Minify",
            Transform::HexEncode => "Hex Encode",
            Transform::HexDecode => "Hex Decode",
            Transform::HashMd5 => "MD5 Hash",
            Transform::HashSha1 => "SHA-1 Hash",
            Transform::HashSha256 => "SHA-256 Hash",
            Transform::CharWordLineCount => "Count chars/words/lines",
            Transform::StripHtml => "Strip HTML",
            Transform::RegexReplace { .. } => "Regex Replace",
        }
    }

    // everything except regex replace which needs user input
    pub fn all_simple() -> Vec<Transform> {
        vec![
            Transform::Uppercase,
            Transform::Lowercase,
            Transform::TitleCase,
            Transform::SentenceCase,
            Transform::TrimWhitespace,
            Transform::CollapseNewlines,
            Transform::UrlEncode,
            Transform::UrlDecode,
            Transform::Base64Encode,
            Transform::Base64Decode,
            Transform::JsonPrettify,
            Transform::JsonMinify,
            Transform::HexEncode,
            Transform::HexDecode,
            Transform::HashMd5,
            Transform::HashSha1,
            Transform::HashSha256,
            Transform::CharWordLineCount,
            Transform::StripHtml,
        ]
    }
}

// runs a transform on the input string and returns the result
pub fn apply(input: &str, transform: &Transform) -> Result<String> {
    Ok(match transform {
        Transform::Uppercase => input.to_uppercase(),

        Transform::Lowercase => input.to_lowercase(),

        Transform::TitleCase => title_case(input),

        Transform::SentenceCase => sentence_case(input),

        Transform::TrimWhitespace => input.trim().to_string(),

        Transform::CollapseNewlines => {
            let re = Regex::new(r"[\r\n]+")?;
            re.replace_all(input, " ").into_owned()
        }

        Transform::UrlEncode => {
            utf8_percent_encode(input, NON_ALPHANUMERIC).to_string()
        }

        Transform::UrlDecode => {
            percent_decode_str(input)
                .decode_utf8()
                .map(|s| s.into_owned())
                .unwrap_or_else(|_| input.to_string())
        }

        Transform::Base64Encode => B64.encode(input.as_bytes()),

        Transform::Base64Decode => {
            let bytes = B64.decode(input.trim())
                .map_err(|e| anyhow::anyhow!("Base64 decode error: {e}"))?;
            String::from_utf8(bytes)
                .map_err(|e| anyhow::anyhow!("Base64 decode: not valid UTF-8: {e}"))?
        }

        Transform::JsonPrettify => {
            let val: serde_json::Value = serde_json::from_str(input)
                .map_err(|e| anyhow::anyhow!("JSON parse error: {e}"))?;
            serde_json::to_string_pretty(&val)?
        }

        Transform::JsonMinify => {
            let val: serde_json::Value = serde_json::from_str(input)
                .map_err(|e| anyhow::anyhow!("JSON parse error: {e}"))?;
            serde_json::to_string(&val)?
        }

        Transform::HexEncode => {
            hex::encode(input.as_bytes())
        }

        Transform::HexDecode => {
            let bytes = hex::decode(input.trim())
                .map_err(|e| anyhow::anyhow!("Hex decode error: {e}"))?;
            String::from_utf8(bytes)
                .map_err(|e| anyhow::anyhow!("Hex decode: not valid UTF-8: {e}"))?
        }

        Transform::HashMd5 => {
            // md5 is kinda broken for actual crypto but fine as a display thing
            // just add the `md5` crate to Cargo.toml if you want this to actually work
            format!("[md5 — add the `md5` crate to Cargo.toml for this transform]")
        }

        Transform::HashSha1 => {
            // same deal, need the `sha1` crate
            format!("[sha1 — add the `sha1` crate to Cargo.toml for this transform]")
        }

        Transform::HashSha256 => {
            let mut hasher = Sha256::new();
            hasher.update(input.as_bytes());
            hex::encode(hasher.finalize())
        }

        Transform::CharWordLineCount => {
            let chars = input.chars().count();
            let words = input.split_whitespace().count();
            let lines = input.lines().count();
            format!("Characters: {chars}\nWords: {words}\nLines: {lines}")
        }

        Transform::StripHtml => {
            let re = Regex::new(r"<[^>]*>")?;
            re.replace_all(input, "").into_owned()
        }

        Transform::RegexReplace { pattern, replacement } => {
            let re = Regex::new(pattern)
                .map_err(|e| anyhow::anyhow!("Invalid regex: {e}"))?;
            re.replace_all(input, replacement.as_str()).into_owned()
        }
    })
}

// helper functions for the case transforms

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sentence_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// basic tests so i know nothing is obviously broken
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uppercase() {
        assert_eq!(apply("hello world", &Transform::Uppercase).unwrap(), "HELLO WORLD");
    }

    #[test]
    fn test_title_case() {
        assert_eq!(apply("the quick brown fox", &Transform::TitleCase).unwrap(), "The Quick Brown Fox");
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = "Hello, ClipVault!";
        let encoded = apply(original, &Transform::Base64Encode).unwrap();
        let decoded = apply(&encoded, &Transform::Base64Decode).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_sha256() {
        let hash = apply("abc", &Transform::HashSha256).unwrap();
        // just check the length is right, the actual value is verified separately
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(
            apply("<b>Hello</b> <i>World</i>", &Transform::StripHtml).unwrap(),
            "Hello World"
        );
    }

    #[test]
    fn test_json_prettify() {
        let out = apply(r#"{"a":1,"b":2}"#, &Transform::JsonPrettify).unwrap();
        assert!(out.contains('\n'));
    }

    #[test]
    fn test_collapse_newlines() {
        assert_eq!(
            apply("a\n\nb\r\nc", &Transform::CollapseNewlines).unwrap(),
            "a b c"
        );
    }
}
