const writers = new Set<() => Promise<void>>();

export function registerOfflineWriter(flush: () => Promise<void>): () => void {
  writers.add(flush);
  return () => {
    writers.delete(flush);
  };
}

export async function flushOfflineWriters(): Promise<void> {
  await Promise.all([...writers].map((flush) => flush()));
}
