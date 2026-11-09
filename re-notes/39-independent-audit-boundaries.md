# Independent audit remediation boundaries — 2026-11-09

## Evidence and comparison

**Unknown — AMBIGUOUS.** `36-zalo-e2ee-signal-protocol-analysis.md` section 8
leaves the official ciphertext container and prekey server schema unverified.
`38-e2ee-chat-integration.md` describes the previous Rust implementation; it is
not an E1 capture or an independently decoded official request/response fixture.
The route index remains E2 evidence of existence only.

The remediation consequently blocks production E2EE send/decrypt/verification
instead of inferring a server contract. Speculative prekey HTTP adapters have
been removed. No plaintext fallback is permitted when encrypted policy exists
or its storage cannot be read. See ADR-0035.

## Local model only

Synthetic tests validate atomic session/prekey/replay-marker persistence,
lost/reordered initial packets, replay rejection, exact displayed-key verification,
pool refill and monotonically allocated prekey identifiers. The retained custom
binary envelope is a local model and is not sent to the official service.

## Other protocol boundaries

Endpoint literals were moved into `src/endpoints.rs` without changing their
values, request methods, params, signing, frame identifiers or cipher flow.
2FA/catalog parsing now rejects refusal or incomplete required data; this does
not independently verify those endpoints' official entitlement or schema.

No authenticated account, phone, cookie or live message was used for these tests.
Native Windows ARM64 and real Zalo PC/Web acceptance remain separate work.

## Native H.264 follow-up

The explicitly enabled `native_h264_roundtrip` test on the Windows x64 development
host produced zero decoded frames before remediation. This is local runtime
evidence, not a Zalo capture. The inbox MFT encoder/decoder are configured through
`ICodecAPI::SetValue(CODECAPI_AVLowLatencyMode)`: `VT_BOOL` for the encoder and
`VT_UI4` for the decoder, following Microsoft's documented type distinction.

Fork implementation choice for live capture: use the documented low-latency
mode rather than buffering frames across encoder recreation. Resolution,
baseline profile, timestamps, Annex-B payload handling and signaling parameters
are retained. Official Zalo encoder configuration remains unknown.

Primary references:
- [CODECAPI_AVLowLatencyMode](https://learn.microsoft.com/en-us/windows/win32/medfound/codecapi-avlowlatencymode)
- [Microsoft H.264 encoder](https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-encoder)
