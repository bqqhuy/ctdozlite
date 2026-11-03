//! Size-bounded protobuf decoding for the official Sync2 conversation/message schemas.
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Map, Value};
use std::io::Read;

pub(super) const PLAIN_LIMIT: usize = 16 * 1024 * 1024;
const ITEM_LIMIT: usize = 100_000;

/// Decode compression only after authentication, bounding the expanded plaintext.
pub(super) fn expand(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(bytes.len() <= PLAIN_LIMIT, "Sync chunk exceeds limit");
    let reader: Box<dyn Read + '_> = if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        Box::new(zstd::stream::read::Decoder::new(bytes)?)
    } else if bytes.starts_with(&[0x1f, 0x8b]) {
        Box::new(flate2::read::GzDecoder::new(bytes))
    } else if bytes.first() == Some(&0x78) {
        Box::new(flate2::read::ZlibDecoder::new(bytes))
    } else {
        return Ok(bytes.to_vec());
    };
    let mut out = Vec::new();
    reader.take(PLAIN_LIMIT as u64 + 1).read_to_end(&mut out)?;
    ensure!(
        out.len() <= PLAIN_LIMIT,
        "Expanded sync chunk exceeds limit"
    );
    Ok(out)
}

/// Split the length-prefixed transfer container without accepting partial trailing chunks.
pub(super) fn frames(mut bytes: &[u8]) -> Result<Vec<&[u8]>> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        ensure!(bytes.len() >= 4, "Truncated sync frame header");
        let n = u32::from_le_bytes(bytes[..4].try_into()?) as usize;
        bytes = &bytes[4..];
        ensure!(
            n > 0 && n <= PLAIN_LIMIT && n <= bytes.len(),
            "Invalid sync frame length"
        );
        ensure!(out.len() < 4096, "Too many sync frames");
        out.push(&bytes[..n]);
        bytes = &bytes[n..];
    }
    ensure!(!out.is_empty(), "Empty sync container");
    Ok(out)
}

fn varint(bytes: &mut &[u8]) -> Result<u64> {
    let mut out = 0;
    for shift in (0..70).step_by(7) {
        let byte = *bytes.first().context("Truncated protobuf varint")?;
        *bytes = &bytes[1..];
        ensure!(shift != 63 || byte <= 1, "Overflowing protobuf varint");
        out |= ((byte & 127) as u64) << shift;
        if byte & 128 == 0 {
            return Ok(out);
        }
    }
    bail!("Invalid protobuf varint")
}
#[derive(Clone, Copy)]
enum Kind {
    Text,
    Number,
    Large,
    Signed,
    Nested(&'static [Field]),
}
#[derive(Clone, Copy)]
struct Field(u32, &'static str, Kind, bool);
use Kind::*;
const CONVERSATION: &[Field] = &[
    Field(1, "convId", Text, false),
    Field(2, "convType", Number, false),
    Field(3, "respondedByMe", Number, false),
    Field(4, "lastTs", Large, false),
    Field(5, "lastGlobalId", Large, false),
    Field(6, "lastClientId", Large, false),
];
const ATTACH: &[Field] = &[
    Field(1, "type", Text, false),
    Field(2, "extInfo", Text, false),
    Field(3, "action", Text, false),
    Field(4, "params", Text, false),
    Field(5, "title", Text, false),
    Field(6, "href", Text, false),
    Field(7, "thumb", Text, false),
    Field(8, "description", Text, false),
    Field(9, "catId", Number, false),
    Field(10, "id", Number, false),
    Field(11, "childNumber", Number, false),
    Field(12, "remains", Text, false),
    Field(13, "zinstantData", Text, false),
    Field(14, "zinstantMsg", Text, false),
];
const MENTION: &[Field] = &[
    Field(1, "uid", Text, false),
    Field(2, "pos", Number, false),
    Field(3, "len", Number, false),
    Field(4, "type", Number, false),
];
const QUOTE: &[Field] = &[
    Field(1, "attach", Text, false),
    Field(2, "ownerId", Text, false),
    Field(3, "ts", Signed, false),
    Field(4, "fromD", Text, false),
    Field(5, "cliMsgId", Large, false),
    Field(6, "globalMsgId", Large, false),
    Field(7, "msg", Text, false),
    Field(8, "cliMsgType", Number, false),
    Field(9, "ttl", Large, false),
    Field(10, "partialQuote", Text, false),
];
const PROPERTIES: &[Field] = &[
    Field(1, "size", Number, false),
    Field(2, "color", Number, false),
    Field(3, "type", Number, false),
    Field(4, "subType", Number, false),
    Field(5, "ext", Text, false),
];
const REFERENCE: &[Field] = &[
    Field(1, "type", Number, false),
    Field(2, "data", Text, false),
];
const META: &[Field] = &[
    Field(1, "attachsList", Nested(ATTACH), true),
    Field(2, "mentionsList", Nested(MENTION), true),
    Field(3, "reference", Nested(REFERENCE), false),
    Field(4, "quote", Nested(QUOTE), false),
    Field(5, "properties", Nested(PROPERTIES), false),
    Field(6, "ttl", Large, false),
];
const MESSAGE: &[Field] = &[
    Field(1, "senderId", Text, false),
    Field(2, "clientId", Large, false),
    Field(3, "msgStatus", Number, false),
    Field(4, "msgType", Number, false),
    Field(5, "timestamp", Large, false),
    Field(6, "globalId", Signed, false),
    Field(7, "content", Text, false),
    Field(8, "meta", Nested(META), false),
];
const PARTITION: &[Field] = &[
    Field(1, "id", Text, false),
    Field(2, "messages", Nested(MESSAGE), true),
];
const CONVERSATIONS: &[Field] = &[
    Field(1, "platform", Number, false),
    Field(2, "conversations", Nested(CONVERSATION), true),
];
const MESSAGES: &[Field] = &[
    Field(1, "platform", Number, false),
    Field(2, "partitions", Nested(PARTITION), true),
];

fn decode(mut bytes: &[u8], fields: &'static [Field], budget: &mut usize) -> Result<Value> {
    let mut out = Map::new();
    while !bytes.is_empty() {
        ensure!(*budget > 0, "Protobuf item limit");
        *budget -= 1;
        let tag = varint(&mut bytes)?;
        let number = tag >> 3;
        ensure!(number > 0 && number <= 536_870_911, "Invalid protobuf tag");
        let field = fields.iter().find(|f| f.0 as u64 == number);
        let value = match tag & 7 {
            0 => {
                let v = varint(&mut bytes)?;
                match field.map(|f| f.2) {
                    Some(Large) => Value::String(v.to_string()),
                    Some(Signed) => Value::String((v as i64).to_string()),
                    Some(Number) => json!(v as i64),
                    None => Value::Null,
                    _ => bail!("Wrong protobuf scalar wire type"),
                }
            }
            2 => {
                let n = usize::try_from(varint(&mut bytes)?)?;
                ensure!(n <= bytes.len(), "Truncated protobuf field");
                let b = &bytes[..n];
                bytes = &bytes[n..];
                match field.map(|f| f.2) {
                    Some(Text) => json!(std::str::from_utf8(b)?),
                    Some(Nested(schema)) => decode(b, schema, budget)?,
                    None => Value::Null,
                    _ => bail!("Wrong protobuf nested wire type"),
                }
            }
            1 | 5 if field.is_none() => {
                let n = if tag & 7 == 1 { 8 } else { 4 };
                ensure!(bytes.len() >= n, "Truncated unknown protobuf field");
                bytes = &bytes[n..];
                Value::Null
            }
            _ => bail!("Unsupported protobuf wire type"),
        };
        if let Some(Field(_, name, _, repeated)) = field {
            if *repeated {
                out.entry(*name)
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .context("Invalid repeated field")?
                    .push(value);
            } else {
                out.insert((*name).into(), value);
            }
        }
    }
    Ok(Value::Object(out))
}
/// Preserve uint64 identifiers as strings and reject malformed or oversized protobufs.
pub(super) fn chunk(bytes: &[u8], conversations: bool) -> Result<Value> {
    ensure!(bytes.len() <= PLAIN_LIMIT, "Protobuf exceeds limit");
    decode(
        bytes,
        if conversations {
            CONVERSATIONS
        } else {
            MESSAGES
        },
        &mut { ITEM_LIMIT },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncation_and_overflow() {
        assert!(frames(&[4, 0, 0, 0, 1]).is_err());
        assert!(chunk(&[0x12, 0x08, 1], true).is_err());
        assert!(varint(&mut &[0xff; 10][..]).is_err());
        assert!(chunk(&[0], false).is_err());
    }
    #[test]
    fn decodes_official_fields_without_float_id_loss() {
        // Synthetic Message{globalId:u64::MAX,content:"hello"} in partition oneone/fake.
        let mut message = vec![0x30];
        message.extend([0xff; 9]);
        message.push(1);
        message.extend([0x3a, 5]);
        message.extend(b"hello");
        let mut partition = vec![0x0a, 11];
        partition.extend(b"oneone/fake");
        partition.extend([0x12, message.len() as u8]);
        partition.extend(message);
        let mut body = vec![0x12, partition.len() as u8];
        body.extend(partition);
        let decoded = chunk(&body, false).unwrap();
        assert_eq!(decoded["partitions"][0]["messages"][0]["globalId"], "-1");
        assert_eq!(decoded["partitions"][0]["messages"][0]["content"], "hello");
        let compressed = zstd::stream::encode_all(body.as_slice(), 1).unwrap();
        assert_eq!(expand(&compressed).unwrap(), body);
    }
    #[test]
    fn rejects_decompression_bombs() {
        let bomb = zstd::stream::encode_all(vec![0u8; PLAIN_LIMIT + 1].as_slice(), 1).unwrap();
        assert!(expand(&bomb).is_err());
    }
}
