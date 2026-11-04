# Group mentions and asynchronous upload controls — 2026-11-04

Evidence: locally extracted Zalo PC 26.11.10, file
`pc-dist/compact-app-pc.5c0110a3a8749b3aacde.js`.

- `sendMentionMessage` near character offset 15091100 sends GROUP
  `/api/group/mention`, with `grid`, `message`, `clientId`, `language`,
  `visibility`, and `mentionInfo = JSON.stringify(mentions)`.
- Command table near offset 2987034 maps 11198 to that endpoint and 11197
  to `/api/group/quote`. `transformQuoteMessage` near 15088631 adds
  `mentionInfo = JSON.stringify(s)` when supplied with mention recipients.
- Independent reference: `zca-js/src/apis/sendMessage.ts`, handleMentions:
  `{uid,pos,len,type}`, type 1 for UID `-1` (@All), otherwise 0. Positions
  use JavaScript string offsets (UTF-16 code units), not Rust UTF-8 bytes.
- The existing session encoding, signing and server-confirmation paths are retained.
  No E2EE/Signal mention path is inferred. Text with selected mentions bypasses
  automatic link-card conversion to preserve explicit recipient metadata.
- Async upload parsing is inspired by upstream PR #85, head e79c95cf:
  direct controls, batched arrays and data.url/data.fileUrl. The fork retains
  its account-scoped bounded registry and cancellation guards. No blind
  replay of upload/send is introduced; timeout is an explicit unsent failure.

Verification: synthetic loopback tests decrypt outbound params and assert
mention and quote endpoints, recipient ranges and stable clientId. Parser
fixtures cover direct/batched controls, invalid file IDs, insecure URLs and bounds.
Actual mention delivery and delayed upload require the Windows/account smoke test
before release; unit tests do not establish server-side delivery.
