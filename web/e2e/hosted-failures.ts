/** Preserve scenario ownership while attempting every independent owned release. */
export async function withHostedCleanup(
  scenario: () => Promise<void>,
  cleanup: Array<() => Promise<void>>,
  onFailure?: (error: unknown) => Promise<void>,
): Promise<void> {
  const failures: unknown[] = [];
  const attempt = async (action: () => Promise<void>) => {
    try {
      await action();
    } catch (error) {
      failures.push(error);
    }
  };
  try {
    await scenario();
  } catch (error) {
    failures.push(error);
    if (onFailure) await attempt(() => onFailure(error));
  }
  for (const release of cleanup) await attempt(release);
  if (failures.length === 1) throw failures[0];
  if (failures.length > 1)
    throw new AggregateError(
      failures,
      "Hosted scenario and owned cleanup failures",
      {
        cause: failures[0],
      },
    );
}
