# Outgoing composer formatting and urgency

Parity with Zalo PC/Web (source: installed Zalo PC 26.11.12, E2 static read on 2026-11-18).
This is static contract evidence and loopback verification, not authenticated interoperability acceptance.

Source: `Zalo-26.11.12/resources/app.asar`, member
`pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`.
Extracted member SHA-256: `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.
Offsets below count JavaScript UTF-16 code units, zero-based.

* At offset 15086010, `static buildParams` places
  `textProperties=JSON.stringify({styles:r.styles,ver:r.ver||0})` on the request
  when styles exist, and `metaData={urgency:r.urgency}` when urgency is supplied.
* At offset 15087284, `static sendZText` builds `message`, `clientId`, `imei`,
  calls `buildParams`, then chooses direct `/api/message/sms` (`toid`) or group
  `/api/group/sendmsg` (`grid`, `visibility`). These are the existing fork paths.
* At offset 1726197 the exported `URGENCY_MODE` is
  `{NORMAL:-1,IMPORTANT:1,URGENT:2,MENTION:3}`. The fork only sends 1 or 2;
  normal mode omits metadata and never prepends fake text.
* `updateMessageFormat` stores the same `styles` in an RTF content object;
  `getAdditionalParams` retains styles and urgency for retries. Existing note 13
  records `{start,len,st}` spans and UTF-16 offsets. `buildParams` is shared by
  send variants, so quote and mention preserve their existing identity fields
  and receive the same metadata.
* At offset 941031 server `specialMsgType` becomes message `urgency`; the fork
  renders the corresponding badge from preserved wire fields.

Implementation: durable `Payload::Formatted` carries options, mentions and quote;
unsupported E2EE metadata fails closed rather than sending plaintext or dropping
formatting. The composer explicitly applies its B/I/U/strike toggles to the whole
draft, keyed by account runtime and conversation. No selection-format editing is
claimed. Link preview cards are bypassed for formatted text to retain metadata.

Verification: `request_client_id_survives_verified_send_variants` decrypts
loopback requests for direct text, group mentions, direct quote and group quote,
checking metadata, mention identity and the original client ID. Rich-text tests
check UTF-16 emoji boundaries and inbound-style round trip; worker tests check
durable retry metadata and fail-closed draft-preview policy. Real PC/phone receipt
and special notification entitlement remain native acceptance checks.
