import test from "node:test";
import assert from "node:assert/strict";
import { backupRetentionMessage } from "./backupRetention.ts";
import type { BackupRetentionReport } from "../backend.ts";

const report: BackupRetentionReport = { entries: [], eligibleBackups: 1, protectedBackups: 2, retiredBackups: 1, verifiedBackupBytesBefore: 300, reclaimedBytes: 100, verifiedBackupBytesAfter: 200, validationMs: 1, elapsedMs: 2, skipped: null };
test("backup maintenance explains policy without changing readiness", () => {
  assert.match(backupRetentionMessage(report), /1 older backups retired; 2 backups protected/);
  assert.match(backupRetentionMessage(report), /30-day/);
});
test("manual repair, safety skips, unknown ownership and retryable failure stay distinct", () => {
  assert.match(backupRetentionMessage({ ...report, skipped: "retainedManualRepair" }), /needs inspection/);
  assert.match(backupRetentionMessage({ ...report, skipped: "retainedRecovery" }), /skipped for safety/);
  assert.match(backupRetentionMessage({ ...report, entries: [{ transactionId: "fixture", outcome: "retainedUnknownOwnership" }] }), /ownership or recovery evidence/);
  for (const outcome of ["cleanupInterrupted", "cleanupFailedSafely"] as const) assert.match(backupRetentionMessage({ ...report, entries: [{ transactionId: "fixture", outcome }] }), /Try again/);
});
