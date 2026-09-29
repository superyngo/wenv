//! Text encoding and line-ending detection so saving never rewrites bytes the user did not edit

use anyhow::{anyhow, Result};

/// Byte-order variants of UTF-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Utf16Order {
    Little,
    Big,
}

/// How a file was encoded on disk; replayed by [`TextFormat::encode`] on save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextFormat {
    /// Original file started with a UTF-8 BOM.
    pub bom: bool,
    /// Original file was UTF-16 (always with BOM).
    pub utf16: Option<Utf16Order>,
    /// Majority of line endings were CRLF.
    pub crlf: bool,
    /// Original file ended with a newline (or was empty).
    pub final_newline: bool,
}

impl Default for TextFormat {
    fn default() -> Self {
        Self {
            bom: false,
            utf16: None,
            crlf: false,
            final_newline: true,
        }
    }
}

impl TextFormat {
    /// Decode raw file bytes into LF-normalized text (no BOM) plus the detected format.
    pub fn decode(bytes: &[u8]) -> Result<(String, TextFormat)> {
        let mut fmt = TextFormat::default();
        let text = if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
            fmt.utf16 = Some(Utf16Order::Little);
            decode_utf16(rest, Utf16Order::Little)?
        } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
            fmt.utf16 = Some(Utf16Order::Big);
            decode_utf16(rest, Utf16Order::Big)?
        } else if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
            fmt.bom = true;
            String::from_utf8(rest.to_vec()).map_err(|_| anyhow!("file is not valid UTF-8"))?
        } else {
            String::from_utf8(bytes.to_vec()).map_err(|_| anyhow!("file is not valid UTF-8"))?
        };

        let crlf_count = text.matches("\r\n").count();
        let lf_only = text.matches('\n').count() - crlf_count;
        fmt.crlf = crlf_count > 0 && crlf_count >= lf_only;
        let text = if crlf_count > 0 {
            text.replace("\r\n", "\n")
        } else {
            text
        };
        fmt.final_newline = text.is_empty() || text.ends_with('\n');
        Ok((text, fmt))
    }

    /// Encode LF text (each line terminated by `\n`) back to the original on-disk format.
    pub fn encode(&self, text: &str) -> Vec<u8> {
        let mut body = text.to_string();
        if !self.final_newline && body.ends_with('\n') {
            body.pop();
        }
        if self.crlf {
            body = body.replace('\n', "\r\n");
        }
        match self.utf16 {
            Some(order) => {
                let mut out = match order {
                    Utf16Order::Little => vec![0xFF, 0xFE],
                    Utf16Order::Big => vec![0xFE, 0xFF],
                };
                for unit in body.encode_utf16() {
                    out.extend_from_slice(&match order {
                        Utf16Order::Little => unit.to_le_bytes(),
                        Utf16Order::Big => unit.to_be_bytes(),
                    });
                }
                out
            }
            None => {
                let mut out = if self.bom {
                    vec![0xEF, 0xBB, 0xBF]
                } else {
                    Vec::new()
                };
                out.extend_from_slice(body.as_bytes());
                out
            }
        }
    }
}

fn decode_utf16(bytes: &[u8], order: Utf16Order) -> Result<String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(anyhow!("file is not valid UTF-16"));
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| match order {
            Utf16Order::Little => u16::from_le_bytes([c[0], c[1]]),
            Utf16Order::Big => u16::from_be_bytes([c[0], c[1]]),
        })
        .collect();
    String::from_utf16(&units).map_err(|_| anyhow!("file is not valid UTF-16"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(bytes: &[u8]) {
        let (text, fmt) = TextFormat::decode(bytes).unwrap();
        assert!(!text.contains('\r'));
        // Parser output re-joined with terminators equals the LF text (plus a final \n)
        let mut lf = text.clone();
        if !lf.is_empty() && !lf.ends_with('\n') {
            lf.push('\n');
        }
        assert_eq!(fmt.encode(&lf), bytes, "input {:?}", bytes);
    }

    #[test]
    fn test_text_format_roundtrip_variants() {
        roundtrip(b"");
        roundtrip(b"a\nb\n");
        roundtrip(b"a\nb");
        roundtrip(b"a\r\nb\r\n");
        roundtrip(b"a\r\nb");
        roundtrip(b"\xEF\xBB\xBFa\n");
        roundtrip(b"\xFF\xFEa\x00\n\x00");
        roundtrip(b"\xFE\xFF\x00a\x00\n");
    }

    #[test]
    fn test_text_format_rejects_invalid_utf8() {
        assert!(TextFormat::decode(b"a\xFFb").is_err());
        assert!(TextFormat::decode(b"\xFF\xFEa").is_err());
    }
}
