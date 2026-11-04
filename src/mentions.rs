//! Explicit composer identities and UTF-16 mention ranges for the group API.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mention {
    pub uid: String,
    pub pos: usize,
    pub len: usize,
    #[serde(rename = "type")]
    pub kind: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub text: String,
    pub mentions: Vec<Mention>,
}

impl From<String> for Draft {
    fn from(text: String) -> Self {
        Self {
            text,
            mentions: Vec::new(),
        }
    }
}
impl From<&str> for Draft {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}
impl std::ops::Deref for Draft {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}
impl PartialEq<&str> for Draft {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}
impl Draft {
    pub fn clear(&mut self) {
        self.text.clear();
        self.mentions.clear();
    }
}

pub(crate) fn byte_at(text: &str, units: usize) -> Option<usize> {
    let mut offset = 0;
    for (byte, ch) in text.char_indices() {
        if offset == units {
            return Some(byte);
        }
        offset += ch.len_utf16();
    }
    (offset == units).then_some(text.len())
}

pub fn validate(text: &str, mentions: &[Mention]) -> anyhow::Result<()> {
    anyhow::ensure!(mentions.len() <= 100, "Quá nhiều mention");
    let mut end = 0;
    for m in mentions {
        anyhow::ensure!(
            (m.uid == "-1"
                || (!m.uid.is_empty()
                    && m.uid != "0"
                    && m.uid.len() <= 32
                    && m.uid.bytes().all(|b| b.is_ascii_digit())))
                && m.kind == u8::from(m.uid == "-1")
                && m.len > 1
                && m.pos >= end,
            "Mention không hợp lệ"
        );
        end = m
            .pos
            .checked_add(m.len)
            .ok_or_else(|| anyhow::anyhow!("Mention vượt giới hạn"))?;
        let start =
            byte_at(text, m.pos).ok_or_else(|| anyhow::anyhow!("Vị trí mention không hợp lệ"))?;
        let stop =
            byte_at(text, end).ok_or_else(|| anyhow::anyhow!("Độ dài mention không hợp lệ"))?;
        anyhow::ensure!(
            text.get(start..stop).is_some_and(|s| s.starts_with('@')),
            "Nội dung mention đã thay đổi"
        );
    }
    Ok(())
}

impl Draft {
    /// Preserve identities only when their entire literal token survives a text edit.
    pub fn edit(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        let prefix = self
            .text
            .chars()
            .zip(text.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let suffix = self.text[prefix..]
            .chars()
            .rev()
            .zip(text[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let old_end = self.text.len() - suffix;
        let new_end = text.len() - suffix;
        let left = self.text[..prefix].encode_utf16().count();
        let right = self.text[..old_end].encode_utf16().count();
        let delta = text[..new_end].encode_utf16().count() as isize - right as isize;
        self.mentions.retain_mut(|m| {
            if m.pos.saturating_add(m.len) <= left {
                true
            } else if m.pos >= right {
                m.pos = m.pos.saturating_add_signed(delta);
                true
            } else {
                false
            }
        });
        self.text = text.to_owned();
    }

    pub fn insert(&mut self, start: usize, end: usize, uid: &str, name: &str) {
        if !self.text.is_char_boundary(start) || !self.text.is_char_boundary(end) || start > end {
            return;
        }
        if self.mentions.len() >= 100
            || !(uid == "-1"
                || (!uid.is_empty()
                    && uid != "0"
                    && uid.len() <= 32
                    && uid.bytes().all(|b| b.is_ascii_digit())))
        {
            return;
        }
        let name = name.replace(['\n', '\r'], " ");
        let name = name.trim();
        let label = format!("@{}", if name.is_empty() { uid } else { name });
        let pos = self.text[..start].encode_utf16().count();
        let mut text = self.text.clone();
        text.replace_range(start..end, &format!("{label} "));
        self.edit(&text);
        self.mentions.push(Mention {
            uid: uid.into(),
            pos,
            len: label.encode_utf16().count(),
            kind: u8::from(uid == "-1"),
        });
        self.mentions.sort_by_key(|m| m.pos);
    }

    pub fn trimmed(&self) -> Self {
        let leading = self.text.len() - self.text.trim_start().len();
        let shift = self.text[..leading].encode_utf16().count();
        let text = self.text.trim().to_owned();
        let mentions = self
            .mentions
            .iter()
            .filter(|m| m.pos >= shift)
            .cloned()
            .map(|mut m| {
                m.pos -= shift;
                m
            })
            .collect();
        Self { text, mentions }
    }
}

/// Locate an unfinished @query immediately before a character cursor.
pub fn query(text: &str, cursor: usize) -> Option<(usize, usize, &str)> {
    let end = text
        .char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let before = &text[..end];
    let start = before.rfind('@')?;
    if start > 0 && !before[..start].chars().next_back()?.is_whitespace() {
        return None;
    }
    let name = &before[start + 1..];
    if name.len() > 128 || name.contains(['\n', '\r', '@']) {
        return None;
    }
    Some((start, end, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_edits_trim_and_identity() {
        let mut d = Draft {
            text: " 😀 hi @an".into(),
            ..Default::default()
        };
        d.insert(9, 12, "42", "An");
        validate(&d.text, &d.mentions).unwrap();
        let trimmed = d.trimmed();
        assert_eq!(trimmed.mentions[0].pos, 6);
        d.edit(&format!("x{}", d.text));
        assert_eq!(d.mentions[0].pos, 8);
        let changed = d.text.replace("@An", "@Anh");
        d.edit(&changed);
        // Inserting at the end leaves the existing token; changing inside removes it.
        d.edit(&d.text.replace("@An", "@Bn"));
        assert!(d.mentions.is_empty());
    }
    #[test]
    fn same_names_keep_distinct_uids_and_edits_do_not_invent_recipients() {
        let mut d = Draft::default();
        d.insert(0, 0, "42", "An");
        let end = d.text.len();
        d.insert(end, end, "43", "An");
        assert_eq!(
            d.mentions
                .iter()
                .map(|m| m.uid.as_str())
                .collect::<Vec<_>>(),
            vec!["42", "43"]
        );
        d.edit("😀 @An @An ");
        validate(&d.text, &d.mentions).unwrap();
        assert_eq!(d.mentions[0].pos, 3);
        d.edit("😀 @Bn @An ");
        assert_eq!(d.mentions.len(), 1);
        assert_eq!(d.mentions[0].uid, "43");
        d.edit("@Unknown");
        assert!(d.mentions.is_empty());
        let malformed = Mention {
            uid: "42".into(),
            pos: usize::MAX,
            len: 2,
            kind: 0,
        };
        assert!(validate("@A", &[malformed]).is_err());
    }

    #[test]
    fn rejects_surrogate_split_and_unsafe_ranges() {
        let m = Mention {
            uid: "42".into(),
            pos: 1,
            len: 2,
            kind: 0,
        };
        assert!(validate("😀 @A", &[m]).is_err());
        assert!(query("email@a", 7).is_none());
        assert_eq!(query("hi @An", 6), Some((3, 6, "An")));
    }
}
