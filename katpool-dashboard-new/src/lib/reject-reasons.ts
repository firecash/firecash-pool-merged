/**
 * Human labels for the stratum share-reject reasons.
 *
 * Keys mirror the `share_reject_reason` Postgres enum / the API's
 * `RejectReasonCount.reason` one-for-one (see ADR-0021 wire contract).
 * Unknown reasons fall back to a title-cased form of the raw token so a
 * newly added backend variant degrades gracefully instead of vanishing.
 */
const REJECT_REASON_LABELS: Record<string, string> = {
  stale: "Stale",
  low_difficulty: "Low difficulty",
  bad_pow: "Bad PoW",
  missing_job: "Job not found",
  malformed_frame: "Malformed frame",
  duplicate_submit: "Duplicate",
  bad_address: "Bad address",
};

export function rejectReasonLabel(reason: string): string {
  return (
    REJECT_REASON_LABELS[reason] ??
    reason
      .split("_")
      .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
      .join(" ")
  );
}
