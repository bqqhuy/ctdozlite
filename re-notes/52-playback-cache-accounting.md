# Playback cache admission — 2026-11-19

Fork deviation: the 512 MiB playback budget is a local resource bound; it is not
an inferred Zalo PC/server quota. Existing PC media evidence is in re-notes/15,
while this repair changes no download URL, wire format or player UI.

Separate admitted/pending atomic counters previously checked each other before
independent CAS operations. Concurrent reserve and unreserved admit could each
observe stale capacity and exceed their combined limit. Moving pending bytes to
admitted also briefly exposed false free capacity.

One mutex now protects the combined totals and per-lease accounting updates.
An admission checks the entire request before consuming any reservation. Drop
releases only that lease's admitted/pending units, before filesystem cleanup.
No disk/network/logging operation runs under the budget lock. The two public
mutable atomic counters had no repository consumer outside this module's tests;
they were replaced by private budget state and a test-only snapshot.

Isolated budget tests synchronize competing reserve/admit attempts and a full
reservation transfer. They assert the combined bound, no partial mutation on
rejection, no capacity leak during transfer and exact release. Existing playback
file-lifecycle tests remain. Runtime results are recorded in the audit report;
these tests do not certify native playback or Zalo interoperability.
