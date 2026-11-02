//! Display-only styling for Zalo rich-text (`action: "rtf"`) messages.
//!
//! The message text stays exactly as sent; styles are side metadata with UTF-16 offsets
//! (`params.styles[].start/len/st`), matching official Zalo clients.

use serde_json::Value;

/// Upper bound on spans kept per message; remote payloads are untrusted.
const MAX_STYLES: usize = 256;

/// One styled UTF-16 range of a message text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TextStyle {
    /// Start offset in UTF-16 code units.
    pub start: u32,
    /// Length in UTF-16 code units; always non-zero for stored styles.
    pub len: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
}

/// Visual attributes of one byte range produced by [`segments`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SegmentStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

impl SegmentStyle {
    /// `true` when no attribute is set.
    pub fn is_plain(self) -> bool {
        self == Self::default()
    }
}

fn number(v: &Value) -> Option<u32> {
    v.as_u64()
        .or_else(|| v.as_str()?.trim().parse().ok())
        .and_then(|n| u32::try_from(n).ok())
}

/// Parse styles from an incoming message `content` object.
///
/// Returns an empty list for anything that is not a well-formed `rtf` payload. Ranges outside
/// `text`, zero-length ranges and unknown tags are dropped instead of guessed.
pub fn parse_rtf_styles(content: &Value, text: &str) -> Vec<TextStyle> {
    let content = match content {
        Value::String(raw) if raw.trim_start().starts_with('{') => {
            match serde_json::from_str::<Value>(raw) {
                Ok(value) => return parse_rtf_styles(&value, text),
                Err(_) => return Vec::new(),
            }
        }
        value => value,
    };
    if content.get("action").and_then(Value::as_str) != Some("rtf") {
        return Vec::new();
    }
    let params = match content.get("params") {
        Some(Value::String(raw)) => serde_json::from_str::<Value>(raw).unwrap_or(Value::Null),
        Some(value) => value.clone(),
        None => Value::Null,
    };
    let Some(items) = params.get("styles").and_then(Value::as_array) else {
        return Vec::new();
    };
    let units = text.encode_utf16().count() as u64;
    items
        .iter()
        .filter_map(|item| {
            let start = item.get("start").and_then(number)?;
            let len = item.get("len").and_then(number)?;
            if len == 0 || u64::from(start) + u64::from(len) > units {
                return None;
            }
            let mut style = TextStyle {
                start,
                len,
                ..TextStyle::default()
            };
            for tag in item.get("st").and_then(Value::as_str)?.split(',') {
                match tag.trim() {
                    "b" => style.bold = true,
                    "i" => style.italic = true,
                    "u" => style.underline = true,
                    "s" => style.strike = true,
                    // Larger font sizes (`f_18`, ...) are rendered as emphasis, not resized.
                    tag if tag.starts_with("f_") => style.bold = true,
                    _ => {}
                }
            }
            (style.bold || style.italic || style.underline || style.strike).then_some(style)
        })
        .take(MAX_STYLES)
        .collect()
}

/// Split `text` into consecutive byte ranges with combined styles, covering the whole text.
///
/// Overlapping styles combine; offsets that fall inside a surrogate pair are rounded to the
/// next character boundary, so the result always slices `text` safely.
pub fn segments(text: &str, styles: &[TextStyle]) -> Vec<(std::ops::Range<usize>, SegmentStyle)> {
    if text.is_empty() {
        return Vec::new();
    }
    if styles.is_empty() {
        return vec![(0..text.len(), SegmentStyle::default())];
    }
    // Map every UTF-16 boundary to a byte offset (one entry per char plus the end).
    let mut boundaries: Vec<(u32, usize)> = Vec::with_capacity(text.len() + 1);
    let mut unit = 0u32;
    for (byte, ch) in text.char_indices() {
        boundaries.push((unit, byte));
        unit = unit.saturating_add(ch.len_utf16() as u32);
    }
    boundaries.push((unit, text.len()));
    let to_byte = |offset: u32| -> usize {
        match boundaries.binary_search_by_key(&offset, |(u, _)| *u) {
            Ok(i) => boundaries[i].1,
            Err(i) => boundaries.get(i).map_or(text.len(), |(_, b)| *b),
        }
    };
    let mut cuts: Vec<usize> = vec![0, text.len()];
    for style in styles {
        cuts.push(to_byte(style.start));
        cuts.push(to_byte(style.start.saturating_add(style.len)));
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter(|w| w[0] < w[1])
        .map(|w| {
            let mut combined = SegmentStyle::default();
            for style in styles {
                let (from, to) = (
                    to_byte(style.start),
                    to_byte(style.start.saturating_add(style.len)),
                );
                if from <= w[0] && w[1] <= to {
                    combined.bold |= style.bold;
                    combined.italic |= style.italic;
                    combined.underline |= style.underline;
                    combined.strike |= style.strike;
                }
            }
            (w[0]..w[1], combined)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rtf(title: &str, styles: Value) -> Value {
        json!({"action":"rtf","title":title,"params":json!({"styles":styles,"ver":0}).to_string()})
    }

    #[test]
    fn styles_parse_tags_and_keep_text_unchanged_when_payload_is_rtf() {
        let content = rtf(
            "Xin chào bạn",
            json!([{"start":0,"len":3,"st":"b"},{"start":4,"len":4,"st":"i,u"},{"start":9,"len":3,"st":"f_18"}]),
        );
        let styles = parse_rtf_styles(&content, "Xin chào bạn");
        assert_eq!(styles.len(), 3);
        assert!(styles[0].bold && !styles[0].italic);
        assert!(styles[1].italic && styles[1].underline);
        assert!(styles[2].bold);
    }

    #[test]
    fn styles_drop_out_of_range_zero_length_and_unknown_tags_when_untrusted() {
        let content = rtf(
            "abc",
            json!([{"start":2,"len":5,"st":"b"},{"start":0,"len":0,"st":"b"},{"start":0,"len":1,"st":"c_ff0000"},{"start":"1","len":"1","st":"s"}]),
        );
        assert_eq!(
            parse_rtf_styles(&content, "abc"),
            vec![TextStyle {
                start: 1,
                len: 1,
                strike: true,
                ..Default::default()
            }]
        );
    }

    #[test]
    fn styles_are_empty_when_content_is_not_rtf() {
        assert!(parse_rtf_styles(&json!({"title":"x","params":"{}"}), "x").is_empty());
        assert!(parse_rtf_styles(&json!("plain text"), "plain text").is_empty());
        assert!(parse_rtf_styles(&json!({"action":"rtf","params":"not json"}), "x").is_empty());
    }

    #[test]
    fn styles_parse_string_encoded_content_when_nested() {
        let content = rtf("hi", json!([{"start":0,"len":2,"st":"b"}])).to_string();
        assert_eq!(parse_rtf_styles(&json!(content), "hi").len(), 1);
    }

    #[test]
    fn segments_use_utf16_offsets_when_text_has_emoji_and_vietnamese() {
        // "😀" is two UTF-16 units and four bytes; "đậm" starts at unit 3.
        let text = "😀 đậm x";
        let styles = [TextStyle {
            start: 3,
            len: 3,
            bold: true,
            ..Default::default()
        }];
        let parts = segments(text, &styles);
        let rendered: Vec<(&str, bool)> = parts
            .iter()
            .map(|(r, s)| (&text[r.clone()], s.bold))
            .collect();
        assert_eq!(rendered, vec![("😀 ", false), ("đậm", true), (" x", false)]);
    }

    #[test]
    fn segments_combine_overlaps_and_round_inside_surrogate_pairs() {
        let text = "a😀b";
        let styles = [
            TextStyle {
                start: 0,
                len: 2,
                bold: true,
                ..Default::default()
            },
            TextStyle {
                start: 1,
                len: 3,
                italic: true,
                ..Default::default()
            },
        ];
        let parts = segments(text, &styles);
        let joined: String = parts.iter().map(|(r, _)| &text[r.clone()]).collect();
        assert_eq!(joined, text);
        assert!(parts
            .iter()
            .any(|(r, s)| &text[r.clone()] == "a" && s.bold && !s.italic));
        assert!(parts
            .iter()
            .any(|(r, s)| &text[r.clone()] == "b" && s.italic && !s.bold));
    }

    #[test]
    fn segments_cover_text_once_when_unstyled() {
        assert_eq!(segments("abc", &[]), vec![(0..3, SegmentStyle::default())]);
        assert!(segments("", &[]).is_empty());
    }
}
