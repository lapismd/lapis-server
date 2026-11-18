import type { TerminalOutput } from "../../../../crates/lapis-app-protocol/bindings/protocol";

/** A bounded byte stream, independent of database polling and command queues. */
export async function readTerminalStream(
  response: Response,
  signal: AbortSignal,
  accept: (output: TerminalOutput) => void,
  initialCursor = 0,
): Promise<void> {
  if (
    !response.body ||
    !response.headers.get("content-type")?.startsWith("text/event-stream")
  )
    throw Error("Terminal stream is unavailable");
  const reader = response.body.getReader();
  const abort = () => {
    void reader.cancel().catch(() => {});
  };
  signal.addEventListener("abort", abort, { once: true });
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let buffer = "";
  let cursor = initialCursor;
  try {
    while (!signal.aborted) {
      const { value, done } = await reader.read();
      if (signal.aborted) return;
      buffer += decoder.decode(value, { stream: !done });
      buffer = buffer.replaceAll("\r\n", "\n");
      let end: number;
      while ((end = buffer.indexOf("\n\n")) !== -1) {
        if (end > 256 * 1024) throw Error("Terminal frame exceeds limit");
        const frame = buffer.slice(0, end);
        buffer = buffer.slice(end + 2);
        const lines = frame.split("\n");
        if (!lines.some((line) => line === "event: output"))
          throw Error("Invalid terminal event");
        const output: TerminalOutput = JSON.parse(
          lines
            .filter((line) => line.startsWith("data:"))
            .map((line) => line.slice(5).trimStart())
            .join("\n"),
        );
        if (
          !Number.isSafeInteger(output.cursor) ||
          output.cursor < cursor ||
          !Array.isArray(output.bytes) ||
          output.bytes.length > 32768 ||
          !output.bytes.every(
            (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255,
          ) ||
          typeof output.running !== "boolean" ||
          typeof output.truncated !== "boolean" ||
          (output.exit_code !== null && !Number.isInteger(output.exit_code))
        )
          throw Error("Invalid terminal output");
        cursor = output.cursor;
        accept(output);
      }
      if (buffer.length > 256 * 1024)
        throw Error("Terminal frame exceeds limit");
      if (done) {
        if (buffer.trim()) throw Error("Truncated terminal output");
        return;
      }
    }
  } finally {
    signal.removeEventListener("abort", abort);
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}
