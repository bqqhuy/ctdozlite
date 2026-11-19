# Unverified feature gates

Evidence: E2, static inspection of the PC 26.10.20 and 26.11.12 bundles.

The previous device and 2FA routes (`/api/login/getdevicelist`,
`/api/login/kickdevice`, `/api/login/check2fa`) were not located in either
bundle. Absence alone does not prove server nonexistence, but provides no basis
for sending these guessed contracts. Product catalog and group Todo requests
also lack verified service/schema parity. Route existence does not establish
their previous payloads or entitlement.

Fork deviation: related network helpers now return explicit unavailable errors
before any network or credential use. Their UI controls are disabled with an
unknown/unavailable status. Offline parsers and models remain for research;
their fixtures do not establish a server contract. In particular, an unavailable
2FA check must never be displayed as disabled 2FA, and an empty device model
must not be presented as a server-confirmed current-device list.

Enable these features only after documenting method, service identity, payload,
response schema, and entitlement from native evidence and relevant verification.
