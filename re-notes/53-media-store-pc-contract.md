# Media store contract parity note

Evidence level: E2 (static Zalo PC 26.11.12 bundle inspection).

The renderer's `getMediaFromConversation` selects `media_store_send2me` and
`/api/media/oneone/list` only when the conversation id exactly equals
`sendToMeId`. Other conversations use `media_store` and `/api/mediastore/list`.
Both requests are GET requests with common query parameters and an AES-encrypted,
URI-encoded JSON `params` value. The payload contains `media_type`, `limit`,
optional numeric `lastFetchId`, and `last_id`; every non-send-to-me conversation also carries
`group_id` and `imei`. Responses are read from `data.items`, `data.has_more`,
and `data.last_id`, with `last_id` used as the next cursor.

The wrapper adds `lastFetchId` only when the optional caller value is truthy
and numeric (`o && !isNaN(o)`); its `last_id` argument is independent. This fork
omits the optional watermark when no caller value exists and does not invent it
from `last_id`.

The bundle defines `GROUPID_PREFIX` as the literal `g`; the helper removes one
leading `g` only when present and leaves direct ids unchanged. This note does
not establish server acceptance or entitlement; no live capture was available.
