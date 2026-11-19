# Desktop backup WAL-safe batch rollback — 2026-11-19

## Bằng chứng và phạm vi

Fork deviation: local import transaction policy follows ADR-0031, not an inferred
Zalo server contract. Desktop V2 format/asset evidence remains in re-notes/12 and
the existing format reader; this repair does not change the format or any request.

ADR-0031 specifies short transactions: committed batches survive cancellation or
error, and only the current batch rolls back. The later `AtomicRestoreSession`
main-file copy contradicted that policy. In WAL mode a main-file snapshot may omit
committed live rows; overwriting it while SQLite is open can lose those rows after
a checkpoint. A byte-file test could not verify SQLite transaction safety.

## Sửa

- Desktop import now relies on SQLite rollback of its current transaction before
  removing only newly created assets from that batch. Earlier committed rows and
  assets stay available for an idempotent rerun.
- Removed the raw-file snapshot helper/export/test after scanning all consumers:
  Desktop V2 was the only production caller; phone import already uses SQLite
  transactions.
- Updated Spec 011 to the canonical ADR-0031 semantics. Historical release notes
  remain historical records rather than current safety guarantees.

## Regression

The isolated Desktop V2 integration fixture builds a synthetic encrypted TAR with
251 records and two media assets. Large records cause the first committed batch
to cross SQLite's WAL auto-checkpoint threshold. A SQL trigger fails the second
batch. Assertions preserve the preexisting live row and committed batch/media,
remove the current batch/media, and verify cancellation before import plus an
idempotent rerun. Reading a copied main DB without its WAL confirms the checkpoint
occurred. No real account, session or network is used. Execution results are kept
in the audit report/Spec 011 verification by the coordinating agent.
