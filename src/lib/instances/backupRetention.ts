import type { BackupRetentionReport } from "../backend";

export function backupRetentionMessage(report: BackupRetentionReport): string {
  if (report.skipped === "retainedManualRepair") return "Recovery data needs inspection. Backups were preserved; resolve the installation recovery issue first.";
  if (report.skipped) return "Backup maintenance was skipped for safety. Finish recovery or validate the installation before trying again.";
  const failures = report.entries.some(entry => entry.outcome === "cleanupInterrupted" || entry.outcome === "cleanupFailedSafely");
  if (failures) return "Backup maintenance could not finish. Recovery data remains protected. Try again; if this repeats, inspection is needed.";
  const uncertain = report.entries.some(entry => entry.outcome === "retainedUnknownOwnership" || entry.outcome === "retainedRecovery" || entry.outcome === "retainedManualRepair");
  return `${report.retiredBackups} older backups retired; ${report.protectedBackups} backups protected.${uncertain ? " Some backups were preserved because ownership or recovery evidence needs inspection." : " The latest backups and the 30-day grace period remain protected."}`;
}
