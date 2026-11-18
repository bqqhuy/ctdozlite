# MAP-14 — Settings control coverage

Source inspection updated 2026-11-18. This matrix records implementation paths;
it does not certify Windows hardware or live Zalo acceptance. All local settings
serialize through `save_settings` → `SaveAppSettings` → worker `storage::save_settings`.
Closing/Escape saves the modal; device and account-lock changes also save explicitly.

| Page | Controls and effective path |
| --- | --- |
| General | Language → `i18n::set_language`; remember-login → session persistence; download folder → native picker/validation and download consumers; microphone/camera → async device enumeration and media device selectors; autostart → worker → native HKCU Run registry entry. Contacts filter is consumed by `visible_contact_sections`. |
| Account/security | App lock/PIN/Windows Hello → existing PIN verification, lock timeout and biometric worker paths. This page does not claim server device-session management. |
| Privacy | `render_server_privacy` → typed `privacy::Action` worker API → refreshed server snapshot; errors preserve the prior snapshot. Legacy serialized local privacy fields are not exposed as fake server controls. |
| Sync | Recent threads → refresh; phone range → Sync2 commands; stop/rescan/wake → existing phone history flows; explicit V1 fallback; native backup import and history tools. |
| Data | Import/export → tools tab 1; media/zCloud → tools tab 8; account scope retained by existing data-management commands. |
| Appearance | Theme/system/density/color controls → effective palette; avatar wallpaper → existing async avatar image worker/cache → history canvas tint, with normal background when absent. |
| Notifications | Enable → incoming alerts/desktop notice policy; sound toggle/test → notification sound preparation and playback. |
| Messages | Quick messages → composer menu and slash expansion; incoming/outgoing typing → render/typing commands; priority split → two local tabs based on existing server/local pinned conversations; Excel mode → clipboard branch/ask; double-click reply → actual quote composer; typing-indicator toggle → visible typing label. |
| Calls | Async microphone/speaker/camera enumeration; persisted device choices are consumed by capture, output playback, and video modules. Incoming-call notification toggle gates the existing in-app status and OS attention request. No unsupported codec/capability switch is presented. |
| Utilities | Sticker suggestions/search, animation, mention suggestions and local GIF filtering → respective composer/picker/render consumers. |

### Coverage additions (2026-11-18)

* Link suggestions now have a persisted `link_suggestions_enabled` control. The
  existing viewport-bounded link metadata worker request is skipped when disabled;
  the already stored card remains renderable.
* `file_thumbnails_enabled` gates media thumbnail fetches in the bubble renderer;
  disabled state keeps the existing fallback icon/text and performs no worker
  request. This is a local presentation preference, not a server-side download
  policy.
* The requested preload-under-30MB setting remains unimplemented because current
  message projections do not carry a verified content length and the prefetch
  worker has no authoritative size gate. No fake toggle was exposed. Video mute
  and capture shortcut/window-scope controls remain native/source gaps pending a
  verified existing consumer.

## Explicit fork behavior

* “Bạn bè và người đã liên hệ” includes known local conversations, while the
  friends option applies the authoritative friend set when available. It does
  not claim to import phone contacts who do not use Zalo. The installed PC E2
  `getPhonebookV2` consumer is a UID-to-display-name map (`{version,phonebook}`),
  not evidence for a non-Zalo phone-contact record schema.
* “Chia mục Ưu tiên theo hội thoại đã ghim” is a local view over existing pin
  identities. No server priority-classification contract is inferred.
* Avatar wallpaper is local presentation and is not synced to the other party.
* Whole-message B/I/U/strike formatting is explicit in the composer. Selection
  formatting is not claimed. Wire metadata follows re-notes/46.

## Verification boundaries

Compiler checks, regression results and two independent audits are recorded by
the phase coordinator. Native confirmation remains needed for actual Windows
Run-key launch, device enumeration/capture, Windows Hello, notification sound,
and Zalo server privacy/phone-history acceptance. None is marked tested here
from source inspection alone.
