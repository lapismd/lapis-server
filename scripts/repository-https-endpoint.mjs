// Adapted from jj-store scripts/vault-https-fixture.mjs at 086def18 (Apache-2.0).
// Disposable smart-HTTP backend. Observations never retain Authorization bytes.
import { createServer } from "node:https";
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export async function httpsFixture(base, environment) {
  const facts = {
    anonymousReads: 0,
    authenticatedReads: 0,
    receives: 0,
    refused: 0,
    leaked: 0,
    redirects: 0,
  };
  const children = new Set();
  const save = () =>
    writeFileSync(join(base, "facts.json"), JSON.stringify(facts));
  const server = createServer(
    {
      key: readFileSync(join(base, "server.key")),
      cert: readFileSync(join(base, "server.crt")),
    },
    (request, response) => {
      const url = new URL(request.url, "https://localhost");
      const repo = url.pathname.split("/")[1];
      if (!["token.git", "anonymous.git", "redirect.git"].includes(repo)) {
        response.writeHead(404).end();
        return;
      }
      const authorization = request.headers.authorization;
      if (repo === "redirect.git") {
        facts.redirects++;
        save();
        response
          .writeHead(302, {
            Location:
              "https://127.0.0.1:" +
              server.address().port +
              "/anonymous.git/info/refs?service=git-upload-pack",
          })
          .end();
        return;
      }
      if (repo === "anonymous.git") {
        if (authorization) facts.leaked++;
        facts.anonymousReads++;
      } else {
        const token = readFileSync(join(base, "active-token"), "utf8");
        const expected =
          "Basic " + Buffer.from("fixture-user:" + token).toString("base64");
        if (authorization !== expected) {
          facts.refused++;
          save();
          response
            .writeHead(401, {
              "WWW-Authenticate": 'Basic realm="Disposable Lapis fixture"',
            })
            .end();
          return;
        }
        facts.authenticatedReads++;
        if (url.pathname.endsWith("/git-receive-pack")) facts.receives++;
      }
      save();
      const child = spawn("git", ["http-backend"], {
        env: {
          ...environment,
          GIT_PROJECT_ROOT: join(base, "exports"),
          GIT_HTTP_EXPORT_ALL: "1",
          PATH_INFO: url.pathname,
          QUERY_STRING: url.search.slice(1),
          REQUEST_METHOD: request.method,
          SERVER_PROTOCOL: "HTTP/1.1",
          HTTPS: "on",
          REMOTE_ADDR: "127.0.0.1",
          REMOTE_USER: repo === "token.git" ? "fixture-user" : "",
          CONTENT_TYPE: request.headers["content-type"] ?? "",
          CONTENT_LENGTH: request.headers["content-length"] ?? "",
        },
        stdio: ["pipe", "pipe", "ignore"],
      });
      children.add(child);
      request.pipe(child.stdin);
      child.stdin.on("error", () => {});
      let pending = Buffer.alloc(0),
        headers = false;
      child.stdout.on("data", (bytes) => {
        if (headers) {
          response.write(bytes);
          return;
        }
        pending = Buffer.concat([pending, bytes]);
        const end = pending.indexOf("\r\n\r\n");
        if (end < 0) {
          if (pending.length > 16_384) child.kill();
          return;
        }
        const fields = {},
          lines = pending.subarray(0, end).toString().split("\r\n");
        let status = 200;
        for (const line of lines) {
          const colon = line.indexOf(":");
          if (colon < 0) continue;
          const key = line.slice(0, colon),
            value = line.slice(colon + 1).trim();
          if (key.toLowerCase() === "status")
            status = Number.parseInt(value, 10);
          else fields[key] = value;
        }
        response.writeHead(status, fields);
        response.write(pending.subarray(end + 4));
        pending = Buffer.alloc(0);
        headers = true;
      });
      child.on("error", () => response.destroy());
      child.on("close", () => {
        children.delete(child);
        response.end();
      });
      response.on("close", () => {
        if (child.exitCode === null) child.kill();
      });
    },
  );
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  save();
  return {
    endpoint: "https://127.0.0.1:" + server.address().port,
    async close() {
      for (const child of children) child.kill();
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
