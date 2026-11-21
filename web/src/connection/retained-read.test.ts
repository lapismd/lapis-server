import { expect, it, vi } from "vitest";
import { HostedConnection, HostedTransportError } from "./connection";
import { retainHostedRead } from "./retained-read";

it("holds the previous status during an outage and reports real service failures", async () => {
  const connection = new HostedConnection();
  connection.reached("connected");
  const unavailable = new HostedTransportError(503);
  const failure = Error("Invalid status snapshot");
  const source = vi
    .fn()
    .mockResolvedValueOnce("observed")
    .mockRejectedValueOnce(unavailable)
    .mockResolvedValueOnce("recovered")
    .mockRejectedValueOnce(failure);
  const read = retainHostedRead(connection, source);
  await expect(read()).resolves.toBe("observed");
  await expect(read()).resolves.toBe("observed");
  connection.lost();
  await expect(read()).resolves.toBe("observed");
  expect(source).toHaveBeenCalledTimes(2);
  connection.reached("connected");
  await expect(read()).resolves.toBe("recovered");
  await expect(read()).rejects.toBe(failure);
});
