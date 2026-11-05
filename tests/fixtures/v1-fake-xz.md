# Synthetic XZ stream

Generated 2026-11-05 with Python's standard `lzma.compress`, FORMAT_XZ,
default preset. Input is exactly `b"fake-v1-sqlite-stream\n" * 32`.
Contains no database or account data. Tests decoder memory limits and truncation,
not the unverified real-device V1 archive/key compatibility.

`v1-fake-aes.bin` is an independent OpenSSL AES-256-CBC known-answer vector.
Plaintext: ASCII `fake-v1-block!!!` (16 bytes). IV: 16 zero bytes. No padding.
AES key: ASCII `000102030405060708090A0B0C0D0E0F`, derived from fake bytes 00..0F.
The same vector is decrypted twice to check segment IV reset. These are public
synthetic test values, not credentials or material from a user's archive.

RSA test-only fixtures were generated with OpenSSL on 2026-11-05.
`v1-test-only.pk8` is a deliberately public, throwaway 2048-bit PKCS#8 test key,
not a credential and never used by runtime code. `v1-test-only-spki.der` is its
SPKI public key. `v1-test-only-wrapped.bin` encrypts ASCII `0123456789ABCDEF`
with RSA PKCS#1 v1.5. The test compares AWS-LC SPKI and decryption with these
independent known answers, and rejects the ciphertext under another generated key.
