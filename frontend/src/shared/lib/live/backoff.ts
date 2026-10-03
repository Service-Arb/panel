/**
 * The wait before reconnect attempt `attempt` (0-based): exponential, capped,
 * with "equal jitter" — half the step for certain and the other half at random.
 * Never zero, so a server restart does not get every open tab back in the same
 * millisecond, and never longer than the cap.
 */
export function backoffDelay(attempt: number, random: () => number, baseMs = 1_000, capMs = 30_000): number {
  const step = Math.min(capMs, baseMs * 2 ** Math.max(0, attempt));
  return Math.round(step / 2 + random() * (step / 2));
}
