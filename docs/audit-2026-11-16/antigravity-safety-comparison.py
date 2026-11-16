"""Historical audit baseline f0d3206 safety.rs mirror; public Signal vector comparison.
No Signal implementation is copied. This script reproduces the audited baseline algorithm, NOT the repaired implementation,
not the algorithm a future compatible implementation must use.
"""
import hashlib
import json
from pathlib import Path

SOURCE = 'https://raw.githubusercontent.com/signalapp/libsignal/main/rust/protocol/src/fingerprint.rs'
ALICE_DJB = '0506863bc66d02b40d27b8d49ca7c09e9239236f9d7d25d6fcca5ce13c7064d868'
BOB_DJB = '05f781b6fb32fed9ba1cf2de978d4d5da28dc34046ae814402b5c0dbd96fda907b'
EXPECTED = '300354477692869396892869876765458257569162576843440918079131'
ITERATIONS = 5200


def mirror_local_algorithm(first_key, second_key):
    first_key, second_key = sorted((first_key, second_key))
    digest = bytes(64)
    for _ in range(ITERATIONS):
        digest = hashlib.sha512(digest + first_key + second_key).digest()
    return ''.join(f'{int.from_bytes(digest[i:i + 5], "big") % 100000:05d}'
                   for i in range(0, 60, 5))


def main():
    # Our API expects raw 32-byte Curve25519 keys, not DJB's type prefix.
    alice_raw = bytes.fromhex(ALICE_DJB)[1:]
    bob_raw = bytes.fromhex(BOB_DJB)[1:]
    assert len(alice_raw) == len(bob_raw) == 32
    actual = mirror_local_algorithm(alice_raw, bob_raw)
    result = {
        'snapshot': 'f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9',
        'method': 'Python mirror of historical f0d3206 src/crypto/safety.rs, not the repaired code or Rust binary execution',
        'primary_source': SOURCE,
        'primary_source_lines': '148-190 algorithm; 205-217 keys/IDs/expected; 233-278 test',
        'public_test_vector': {
            'alice_djb_hex': ALICE_DJB,
            'bob_djb_hex': BOB_DJB,
            'alice_stable_id': '+14152222222',
            'bob_stable_id': '+14153333333',
            'iterations': ITERATIONS,
            'normalization': 'Strip initial 0x05 DJB type from each 33-byte public key to raw32 for current local API',
            'ids_used_by_local_algorithm': False,
        },
        'current_local_algorithm_output': actual,
        'signal_official_expected': EXPECTED,
        'matches': actual == EXPECTED,
        'scope': 'Proves difference from published Signal vector; does not establish Zalo native expected output or a live exploit',
    }
    destination = Path(__file__).with_name('antigravity-safety-comparison.json')
    destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'matches': result['matches'], 'output': str(destination)}, ensure_ascii=False))


if __name__ == '__main__':
    main()
