import { isHostedTransportError, type HostedConnection } from "./connection";

/** A passive observer can retain its last confirmed projection during an outage. */
export function retainHostedRead<T>(
  connection: HostedConnection | undefined,
  read: () => Promise<T>,
) {
  let last: { value: T } | undefined;
  return async () => {
    if (last && connection?.phase === "offline") return last.value;
    try {
      const value = await read();
      last = { value };
      return value;
    } catch (error) {
      if (last && isHostedTransportError(error)) return last.value;
      throw error;
    }
  };
}
