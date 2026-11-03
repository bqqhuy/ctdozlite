# Local compatibility export

This unpublished package satisfies ratatui 0.29's `lru ^0.12` requirement while
re-exporting the public API from upstream `lru =0.18.2`. It contains no cache
implementation or unsafe code. Version 0.12.6 is a local compatibility version,
not an assertion that the upstream project published this release.

The old 0.12.5 implementation is removed from the resolved dependency graph.
Upstream 0.18.2 fixes RUSTSEC-2026-0002 (mutable iterator aliasing) and
RUSTSEC-2026-0253 (pop panic safety). Normal ratatui builds verify the API used by
its layout cache. Review this patch when upgrading ratatui and remove it when
that dependency accepts a fixed lru directly.
