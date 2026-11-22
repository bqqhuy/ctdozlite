# 47 — PC voice playback and AI conversion (E2)

Source: installed Zalo PC 26.11.12 `resources/app.asar`, read-only extraction of all `pc-dist/*.js` and `pc-dist/lazy/*.js`. Lazy `main-startup.41abf8e6af2a55551e3b.js` SHA-256 `C29C9F0A7F0718C5B044764666B6FC5DEC8B8EB65E178819B3CEE3B1F93E3426`. These are static bundle findings, not E1 server acceptance.

## AI request

At character offset 2091510, `b7.requestConvert({url})` resolves `getAIFeaturesDomain()`, appends common query params, and POSTs `params=encodeURIComponent(encodeAES(JSON.stringify({url:t})))` with command 13394. The existing authenticated session AES path is required here; adding msg_id, thread_id, IMEI or timestamp to the body and using a newly generated ParamCipher key has no support in this implementation. The native domain manager maps AI_FEATURES to service-map key `ai_features`. A missing discovered service is unsupported; do not silently route this call to chat.

## AI result

At offset 2092997, control `voice2text` calls `onReceiveVoiceToTextResultPushEventControl(array)`. It accepts items with `act == convert_finish`; `data` requires nonempty string `req_id` and numeric `err`. Module `v9mf` projects `{requestId:req_id,isTruncated:has_more,url,errorCode:err,text}`. Results must correlate by req_id; missing err is malformed, not success. Timeouts/config and file recovery are separate application states.

## Playback

PC probes resource duration from the downloaded blob and uses local/blob resource playback. Offset 82795 `getDirectURL` can select a configured voice conversion service `/mp3?url=...&msgid=...` for some sources; that conversion contract is not implemented in the fork. The fork decodes the original source using installed Windows Media Foundation codecs and plays PCM through CPAL inline. Unsupported formats surface a codec error. No fake waveform or default duration is used.

## Verification

- Native Windows ARM MF fixture: 1600 mono PCM samples at 16 kHz round-trip through WAV, decoded duration 0.1 seconds: passed.
- UI/worker binding fixtures: dropping/replacing player signals stop; stale account/runtime/request snapshot cannot replace current snapshot: 2 passed.
- Native control nested-array/error fixtures are code-level checks, not server acceptance. Live AI entitlement, codec coverage and speaker behavior remain native-device acceptance work.

Outgoing recording/MP3 upload is outside this implementation slice.
