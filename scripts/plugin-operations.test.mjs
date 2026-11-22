import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, execFile } from "node:child_process";
import { once } from "node:events";
import { promisify } from "node:util";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { createServer } from "node:net";
import { randomUUID } from "node:crypto";
import {
  Client,
  StreamableHTTPClientTransport,
} from "@modelcontextprotocol/client";

const run = promisify(execFile);
const target = resolve(
  process.env.CARGO_TARGET_DIR ?? ".local/rust-core-target",
  "debug",
);
const owner = "isolated-task-operation-owner-token-0123456789";

test(
  "cold hosted Tasks and note operations share HTTP, MCP and CLI authority",
  { timeout: process.env.LAPIS_TEST_HOSTED_ONNX === "1" ? 300000 : 90000 },
  async () => {
    const root = await mkdtemp(join(tmpdir(), "lapis-operation-acceptance-"));
    const vault = join(root, "vault");
    const profile = join(root, "profile");
    await mkdir(vault);
    const id = randomUUID();
    const config = join(root, "server.json");
    await writeFile(
      config,
      JSON.stringify({ vaults: [{ id, name: "Tasks", root: vault }] }),
    );
    await writeFile(
      join(vault, "Task.md"),
      "---\ntype: task\nid: task-one\ntask:\n  status: open\n  inbox: false\n  start: anytime\n---\n\n# Task one\n",
    );
    let child;
    let base;
    const start = async () => {
      const socket = createServer().listen(0, "127.0.0.1");
      await once(socket, "listening");
      const address = `127.0.0.1:${socket.address().port}`;
      await new Promise((resolve) => socket.close(resolve));
      base = `http://localhost:${address.split(":").at(-1)}`;
      const env = {
        ...process.env,
        LAPIS_SERVER_TOKEN: owner,
        LAPIS_SERVER_DATA_ROOT: profile,
        LAPIS_SERVER_ORIGIN: base,
        LAPIS_SERVER_BOOTSTRAP_USERNAME: "owner",
        LAPIS_SERVER_BOOTSTRAP_PASSWORD: "isolated-operation-test-password",
        LAPIS_SERVER_INSECURE_COOKIE: "1",
      };
      for (const key of [
        "LAPIS_SERVER_STATIC_ROOT",
        "LAPIS_SERVER_AGENTS_CONFIG",
        "LAPIS_SERVER_NATIVE_HISTORY",
        "LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE",
      ])
        delete env[key];
      child = spawn(join(target, "lapis-server"), [config, address], {
        env,
        stdio: ["ignore", "pipe", "pipe"],
      });
      let errors = "";
      child.stderr.on("data", (chunk) => {
        errors += chunk;
      });
      await new Promise((resolve, reject) => {
        const deadline = setTimeout(
          () => reject(new Error(`Server startup timed out: ${errors}`)),
          15000,
        );
        child.stdout.on("data", (data) => {
          if (data.toString().includes("Lapis file service listening on")) {
            clearTimeout(deadline);
            resolve();
          }
        });
        child.once("exit", (code) => {
          clearTimeout(deadline);
          reject(new Error(`Server exited ${code}: ${errors}`));
        });
        child.once("error", reject);
      });
    };
    const stop = async () => {
      const stopped = once(child, "exit");
      child.kill("SIGTERM");
      const [code] = await stopped;
      assert.equal(code, 0);
      child = undefined;
    };
    const request = async (path, body, token = owner, content = false) => {
      const response = await fetch(base + path, {
        method: body === undefined ? "GET" : "POST",
        headers: {
          authorization: `Bearer ${token}`,
          [content ? "x-lapis-content-api" : "x-lapis-protocol"]: content
            ? "1"
            : "3",
          ...(body === undefined ? {} : { "content-type": "application/json" }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(15000),
      });
      return { status: response.status, body: await response.json() };
    };
    const issue = async (permissions) => {
      const response = await request("/api/v3/content-access/grants", {
        label: "Tasks fixture",
        vault_ids: [id],
        lifetime_seconds: 3600,
        permissions,
      });
      assert.equal(response.status, 201, JSON.stringify(response.body));
      return response.body.token;
    };
    const invoke = (token, operation, input, requestId = randomUUID()) =>
      request(
        `/api/content/v1/vaults/${id}/plugins/tasks/operations/${operation}`,
        { protocol_version: 1, version: 1, request_id: requestId, input },
        token,
        true,
      );
    try {
      await start();
      const attached = await request(`/api/v3/vaults/${id}/clients`, {
        protocol_version: 3,
      });
      assert.equal(attached.status, 200);
      const client = attached.body.client_id;
      const configuration = {
        day_parts: { afternoonStartsAt: "12:00", eveningStartsAt: "17:00" },
        daily: { folder: "daily", date_format: "yyyy-LL-dd" },
        format: { locale: "en-US", firstDay: 7, minimalDays: 4 },
        provider: { kind: "default" },
      };
      assert.equal(
        (
          await request(`/api/v3/clients/${client}/tasks`, {
            kind: "configure",
            configuration,
          })
        ).status,
        200,
      );
      assert.equal(
        (
          await request(`/api/v3/clients/${client}/indexing`, {
            kind: "refresh",
            rebuild: false,
          })
        ).status,
        200,
      );
      const ownerOperations = (
        await request(`/api/v3/clients/${client}/operations`)
      ).body.operations;
      assert.equal(
        ownerOperations.filter((operation) => operation.plugin_id === "tasks")
          .length,
        17,
      );
      assert.deepEqual(
        ownerOperations
          .filter((operation) => operation.plugin_id === "skills")
          .map((operation) => operation.id)
          .sort(),
        ["install", "list", "update"],
      );
      assert.deepEqual(
        ownerOperations
          .filter((operation) => operation.plugin_id === "notes")
          .map((operation) => operation.id),
        ["create", "frontmatter-get", "frontmatter-patch", "replace", "search"],
      );
      assert.equal(ownerOperations.length, 25);
      assert.equal(
        (await request("/api/v3/content-access", { enabled: true })).status,
        200,
      );
      const legacy = await issue([]);
      assert.deepEqual(
        (
          await request(
            `/api/content/v1/operations?vault_id=${id}`,
            undefined,
            legacy,
            true,
          )
        ).body.operations,
        [],
      );
      const read = await issue(["tasks:read"]);
      assert.equal(
        (
          await invoke(read, "cancel", {
            path: "Task.md",
            expected_version: "stale",
            at: "2026-11-14T12:00:00Z",
          })
        ).status,
        403,
      );
      const token = await issue(["tasks:read", "tasks:write"]);
      assert.equal(
        (
          await request(
            `/api/content/v1/vaults/${randomUUID()}/plugins/tasks/operations/get`,
            {
              protocol_version: 1,
              version: 1,
              request_id: randomUUID(),
              input: { path: "Task.md" },
            },
            token,
            true,
          )
        ).status,
        403,
      );
      assert.equal(
        (
          await request(
            `/api/content/v1/operations?vault_id=${id}`,
            undefined,
            token,
            true,
          )
        ).body.operations.length,
        17,
      );
      const initial = (await invoke(token, "get", { path: "Task.md" })).body
        .outcome.value.snapshot;
      const listed = await invoke(token, "list", { limit: 50 });
      assert.equal(listed.status, 200, JSON.stringify(listed.body));
      assert.equal(listed.body.outcome.value.items.length, 1);
      const identity = randomUUID();
      const input = {
        path: "Task.md",
        expected_version: initial.version,
        at: "2026-11-14T12:00:00Z",
        localDate: "2026-11-14",
      };
      const complete = await invoke(token, "complete", input, identity);
      assert.equal(complete.status, 200, JSON.stringify(complete.body));
      assert.equal(
        complete.body.outcome.value.operation.disposition,
        "completed",
      );
      assert.equal(
        (await invoke(token, "complete", input, identity)).status,
        200,
      );
      assert.equal(
        (await invoke(token, "inspect", { request_id: identity })).body.outcome
          .value.operation.disposition,
        "completed",
      );
      const saved = (await invoke(token, "get", { path: "Task.md" })).body
        .outcome.value.snapshot;
      assert.notEqual(saved.version, initial.version);
      const mcpCall = async (name, args, credential = token) => {
        const mcp = await fetch(base + "/mcp", {
          method: "POST",
          headers: {
            authorization: `Bearer ${credential}`,
            "content-type": "application/json",
            accept: "application/json, text/event-stream",
            "mcp-protocol-version": "2026-07-28",
            "mcp-method": "tools/call",
            "mcp-name": name,
          },
          body: JSON.stringify({
            jsonrpc: "2.0",
            id: 1,
            method: "tools/call",
            params: {
              name,
              arguments: { vault_id: id, ...args },
              _meta: {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
              },
            },
          }),
        });
        return mcp.json();
      };
      const notePath = "Meeting.md";
      const originalNote = { path: notePath, text: "# Initial notes" };
      const noteWrite = await issue(["notes:write"]);
      assert.equal(
        (
          await request(
            `/api/content/v1/vaults/${id}/plugins/notes/operations/create`,
            {
              protocol_version: 1,
              version: 1,
              request_id: randomUUID(),
              input: originalNote,
            },
            legacy,
            true,
          )
        ).status,
        403,
      );
      assert.equal(
        (await mcpCall("notes_create", originalNote, noteWrite)).error.code,
        -32602,
      );
      const noteInvocation = randomUUID();
      const noteCreate = {
        ...originalNote,
        invocation_id: noteInvocation,
      };
      const created = (await mcpCall("notes_create", noteCreate, noteWrite))
        .result.structuredContent;
      assert.equal(created.request_id, noteInvocation);
      assert.equal(created.outcome.status, "ok", JSON.stringify(created));
      assert.equal(created.outcome.value.path, notePath);
      assert.equal(
        (await mcpCall("notes_create", noteCreate, noteWrite)).result
          .structuredContent.outcome.error.code,
        "CONFLICT",
      );
      const noteTokenFile = join(root, "note-write-token");
      await writeFile(noteTokenFile, noteWrite, { mode: 0o600 });
      const replaceIdentity = randomUUID();
      const cliReplace = await run(join(target, "lapis"), [
        "--target",
        base,
        "--token-file",
        noteTokenFile,
        "--json",
        "notes",
        "replace",
        "--vault",
        id,
        "--invocation-id",
        replaceIdentity,
        "--path",
        notePath,
        "--if-version",
        created.outcome.value.version,
        "--text",
        "# Updated notes",
      ]);
      const replaced = JSON.parse(cliReplace.stdout);
      assert.equal(replaced.request_id, replaceIdentity);
      assert.notEqual(
        replaced.outcome.value.version,
        created.outcome.value.version,
      );
      const stale = await mcpCall(
        "notes_replace",
        {
          path: notePath,
          expected_version: created.outcome.value.version,
          text: "# Stale edit",
          invocation_id: randomUUID(),
        },
        noteWrite,
      );
      assert.equal(
        stale.result.structuredContent.outcome.error.code,
        "CONFLICT",
      );
      const savedNote = await mcpCall("read_note", { path: notePath }, legacy);
      assert.equal(
        savedNote.result.structuredContent.outcome.value.text,
        "# Updated notes",
      );
      const noteRead = await issue(["notes:read"]);
      const invokeNote = (token, operation, input) =>
        request(
          `/api/content/v1/vaults/${id}/plugins/notes/operations/${operation}`,
          { protocol_version: 1, version: 1, request_id: randomUUID(), input },
          token,
          true,
        );
      const patch = {
        path: notePath,
        expected_version: replaced.outcome.value.version,
        set: { tags: ["reviewed"], status: null },
      };
      assert.equal(
        (await invokeNote(noteRead, "frontmatter-patch", patch)).status,
        403,
      );
      const patched = await invokeNote(noteWrite, "frontmatter-patch", patch);
      assert.equal(
        patched.body.outcome.value.changed,
        true,
        JSON.stringify(patched),
      );
      assert.equal(
        (await invokeNote(noteWrite, "frontmatter-patch", patch)).status,
        409,
      );
      const properties = (
        await invokeNote(noteRead, "frontmatter-get", { path: notePath })
      ).body.outcome.value;
      assert.deepEqual(properties.properties, {
        tags: ["reviewed"],
        status: null,
      });
      const throughMcp = (
        await mcpCall("notes_frontmatter-get", { path: notePath }, noteRead)
      ).result.structuredContent.outcome.value;
      assert.deepEqual(throughMcp, properties);
      const readTokenFile = join(root, "note-read-token");
      await writeFile(readTokenFile, noteRead, { mode: 0o600 });
      const notesCli = (...args) =>
        run(join(target, "lapis"), [
          "--target",
          base,
          "--token-file",
          readTokenFile,
          "--json",
          "notes",
          ...args,
          "--vault",
          id,
        ]);
      const cliFrontmatter = JSON.parse(
        (await notesCli("frontmatter-get", "--path", notePath)).stdout,
      );
      assert.deepEqual(cliFrontmatter.outcome.value, properties);
      const search = { query: "Updated notes", mode: "auto" };
      assert.equal((await invokeNote(legacy, "search", search)).status, 403);
      assert.equal(
        (
          await invokeNote(noteRead, "search", {
            query: search.query,
            folder: ".hidden",
          })
        ).status,
        400,
      );
      let found;
      for (let attempt = 0; attempt < 40; attempt++) {
        const result = await invokeNote(noteRead, "search", search);
        assert.equal(result.status, 200, JSON.stringify(result.body));
        found = result.body.outcome.value;
        if (
          found.index_status === "ready" &&
          found.hits.some((hit) => hit.path === notePath)
        )
          break;
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      assert.equal(found.mode, "keyword");
      assert.equal(found.semantic_status, "disabled");
      assert.equal(found.hits[0]?.path, notePath);
      const sdk = new Client(
        { name: "Hosted Content acceptance", version: "1.0.0" },
        { versionNegotiation: { mode: "auto" } },
      );
      await sdk.connect(
        new StreamableHTTPClientTransport(new URL("/mcp", base), {
          requestInit: { headers: { authorization: `Bearer ${noteRead}` } },
        }),
      );
      try {
        const { tools } = await sdk.listTools();
        const names = tools.map(({ name }) => name);
        assert.ok(names.includes("notes_search"));
        assert.ok(names.includes("notes_frontmatter-get"));
        assert.ok(!names.includes("notes_frontmatter-patch"));
        const mcpSearch = await sdk.callTool({
          name: "notes_search",
          arguments: { vault_id: id, ...search },
        });
        assert.deepEqual(mcpSearch.structuredContent.outcome.value, found);
      } finally {
        await sdk.close();
      }
      const cliSearch = JSON.parse(
        (await notesCli("search", "--query", search.query)).stdout,
      );
      assert.deepEqual(cliSearch.outcome.value, found);

      const realModel = process.env.LAPIS_TEST_HOSTED_ONNX === "1";
      const assertSemantic = async (credential) => {
        const query = "Damaged car wheel assistance";
        const search = (mode, folder) =>
          invokeNote(credential, "search", { query, mode, folder });
        let semantic;
        for (let attempt = 0; attempt < 100; attempt++) {
          const response = await search("semantic");
          if (response.status === 200) {
            semantic = response.body.outcome.value;
            if (semantic.hits.some((hit) => hit.path === "Transport.md")) break;
          } else {
            assert.equal(
              response.body.outcome.error.code,
              "BUSY",
              JSON.stringify(response.body),
            );
          }
          await new Promise((resolve) => setTimeout(resolve, 100));
        }
        assert.equal(semantic?.mode, "semantic");
        assert.equal(semantic.semantic_status, "ready");
        assert.ok(semantic.hits.some((hit) => hit.path === "Transport.md"));
        assert.equal(
          (await search("keyword")).body.outcome.value.hits.length,
          0,
        );
        const hybrid = (await search("hybrid")).body.outcome.value;
        assert.equal(hybrid.mode, "hybrid");
        assert.ok(hybrid.hits.some((hit) => hit.path === "Transport.md"));
        assert.deepEqual(
          (await search("semantic", "Missing")).body.outcome.value.hits,
          [],
        );
      };
      if (realModel) {
        const fixture = JSON.parse(
          await readFile(
            new URL(
              "../crates/lapis-app-core/tests/fixtures/search-semantic-model.json",
              import.meta.url,
            ),
            "utf8",
          ),
        );
        const configure = async (allowRemoteModels) => {
          const response = await request(`/api/v3/clients/${client}/database`, {
            method: "configureSearchEmbeddingProvider",
            args: [
              {
                kind: "value",
                value: {
                  kind: "transformers-js",
                  modelId: fixture.modelId,
                  revision: fixture.revision,
                  dtype: fixture.dtype,
                  dimensions: 384,
                  normalize: true,
                  allowRemoteModels,
                },
              },
            ],
          });
          assert.equal(response.status, 200, JSON.stringify(response.body));
        };
        const rebuild = async () => {
          const response = await request(`/api/v3/clients/${client}/indexing`, {
            kind: "refresh",
            rebuild: true,
          });
          assert.equal(response.status, 200, JSON.stringify(response.body));
        };
        await writeFile(
          join(vault, "Transport.md"),
          "# Maintenance\n\nAn automobile needs a mechanic to replace a flat tire.",
        );
        await configure(true);
        await rebuild();
        await assertSemantic(noteRead);
        await configure(false);
        await rebuild();
      }
      const result = await mcpCall("tasks_get", { path: "Task.md" });
      assert.equal(
        result.result.structuredContent.outcome.value.snapshot.version,
        saved.version,
      );
      assert.equal((await mcpCall("tasks_complete", input)).error.code, -32602);
      const duplicate = await mcpCall("tasks_complete", {
        ...input,
        invocation_id: identity,
      });
      assert.equal(duplicate.result.structuredContent.request_id, identity);
      assert.equal(
        duplicate.result.structuredContent.outcome.value.operation.disposition,
        "completed",
      );
      const tokenFile = join(root, "token");
      await writeFile(tokenFile, token, { mode: 0o600 });
      const cli = await run(join(target, "lapis"), [
        "--target",
        base,
        "--token-file",
        tokenFile,
        "--json",
        "tasks",
        "get",
        "--vault",
        id,
        "--path",
        "Task.md",
      ]);
      assert.equal(
        JSON.parse(cli.stdout).outcome.value.snapshot.version,
        saved.version,
      );
      const inputFile = join(root, "complete.json");
      await writeFile(inputFile, JSON.stringify(input));
      const completed = await run(join(target, "lapis"), [
        "--target",
        base,
        "--token-file",
        tokenFile,
        "--json",
        "tasks",
        "complete",
        "--vault",
        id,
        "--invocation-id",
        identity,
        "--input-file",
        inputFile,
      ]);
      assert.equal(JSON.parse(completed.stdout).request_id, identity);
      assert.equal(
        JSON.parse(completed.stdout).outcome.value.operation.disposition,
        "completed",
      );
      await stop();
      await start();
      assert.equal(
        (await invoke(token, "get", { path: "Task.md" })).status,
        401,
      );
      const fresh = await issue(["tasks:read", "tasks:write"]);
      if (realModel) await assertSemantic(await issue(["notes:read"]));
      const coldList = await invoke(fresh, "list", { limit: 50 });
      assert.equal(coldList.status, 200, JSON.stringify(coldList.body));
      assert.ok(Array.isArray(coldList.body.outcome.value.items));
      assert.equal(
        (await invoke(fresh, "get", { path: "Task.md" })).body.outcome.value
          .snapshot.version,
        saved.version,
      );
      assert.equal(
        (await invoke(fresh, "inspect", { request_id: identity })).body.outcome
          .value.operation.disposition,
        "completed",
      );
      await stop();
    } finally {
      if (child && child.exitCode === null && child.signalCode === null) {
        const exit = once(child, "exit");
        child.kill("SIGKILL");
        await exit.catch(() => {});
      }
      await rm(root, { recursive: true, force: true });
    }
  },
);
