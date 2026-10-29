import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  symlinkSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createRequire } from "node:module";
const desktop = resolve(process.argv[2] ?? "../lapis-desktop");
const revision = "f276120b30395860aebed67362bfb320ded6f22c";
const output = resolve(
  "crates/lapis-app-core/tests/fixtures/queue-capture-v1.json",
);
const temporary = mkdtempSync(join(tmpdir(), "lapis-queue-source-"));
const hash = (v) => createHash("sha256").update(v).digest("hex");
const require = createRequire(join(desktop, "package.json"));
const ts = require("typescript");
const files = new Map();
const exportSource = (path, execute = true) => {
  if (files.has(path)) return;
  const bytes = execFileSync(
    "jj",
    ["--no-pager", "file", "show", "-r", revision, "--", path],
    { cwd: desktop },
  );
  files.set(path, hash(bytes));
  if (!execute) return;
  const destination = join(temporary, path);
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, bytes);
  const js = ts.transpileModule(bytes.toString(), {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  for (const match of js.matchAll(/from\s+["'](\.[^"']+)["']/g))
    exportSource(join(dirname(path), match[1]));
};
try {
  for (const path of [
    "src/lib/ai/conversations/queued-message.ts",
    "src/lib/ai/conversations/transcript-validation.ts",
    "src/lib/ai/chat/controller/queue-capture.ts",
    "src/lib/ai/runtimes/acp/configuration.ts",
  ])
    exportSource(path);
  exportSource("src/lib/ai/conversations/conversation-message-queue.ts", false);
  symlinkSync(
    join(desktop, "node_modules"),
    join(temporary, "node_modules"),
    "dir",
  );
  writeFileSync(join(temporary, "package.json"), '{"type":"module"}');
  const fixtureSources = [
    "cases.mjs",
    "fixtures.mjs",
    "boundaries.mjs",
    "limits.mjs",
  ];
  for (const name of fixtureSources)
    writeFileSync(
      join(temporary, name),
      readFileSync(new URL(name, import.meta.url)),
    );
  writeFileSync(
    join(temporary, "run.mjs"),
    `
import { cases, queued, time, uuid } from './cases.mjs';
import { validateQueuedMessage, validateQueuedMessages, queueReceipt } from './src/lib/ai/conversations/queued-message.ts';
import { validateTranscriptEntry } from './src/lib/ai/conversations/transcript-validation.ts';
import { captureQueuedMessage } from './src/lib/ai/chat/controller/queue-capture.ts';
import { configureAcpSession } from './src/lib/ai/runtimes/acp/configuration.ts';
const OriginalDate=Date;
globalThis.Date=class extends OriginalDate { constructor(...args) { super(...(args.length?args:[time])); } };
Object.defineProperty(globalThis.crypto,'randomUUID',{value:()=>uuid});
const functions={message:validateQueuedMessage,queue:validateQueuedMessages,receipt:validateTranscriptEntry};
const inputs=cases();
for(const outcome of ['completed','cancelled','rejected','not_sent','unknown']) inputs.push({name:'receipt-'+outcome,action:'receipt',input:queueReceipt(queued(),outcome)});
for(const [key,value] of [['parentId','different-parent'],['id','different-receipt'],['outcome','invalid'],['schemaVersion',2],['source',{sessionId:'s',runId:'r',sequence:1}],['extra','retained']]) {const input=queueReceipt(queued(),'completed');input[key]=value;inputs.push({name:'receipt-field-'+key,action:'receipt',input});}
const capture=captureQueuedMessage('  Captured actual user  ',{agent:'codex',thinking:'default',metadata:{attachments:['Note.md']},executionMode:'read-only',agentProfile:{instructions:'Captured instructions'}});
inputs.push({name:'actual-capture-function',action:'message',input:capture});
const results=inputs.map(({input,...row})=>{try{return{...row,...(input===undefined?{}:{input}),expected:{ok:true,value:functions[row.action](input)}}}catch(error){return{...row,...(input===undefined?{}:{input}),expected:{ok:false,error:{name:error.name,message:error.message}}}}});
const frames=[];
for(const thinking of [undefined,null,'default','off','low','medium','high']) {let frame;const input={executionMode:'read-only',...(thinking===undefined?{}:{thinking})};const value=await configureAcpSession({info:{targetId:'codex'},send:async(_,v)=>{frame=v;return{type:'configured',modeReady:true,configuration:{mode:{status:'applied'},effort:{status:'applied'}}}}},input);frames.push({input,frame,sourceResult:value});}
process.stdout.write(JSON.stringify({cases:results,configurationFrames:frames}));
`,
  );
  const results = JSON.parse(
    execFileSync(
      process.execPath,
      ["--import", require.resolve("tsx"), join(temporary, "run.mjs")],
      { maxBuffer: 32 * 1024 * 1024, encoding: "utf8" },
    ),
  );
  const codecRoot = join(desktop, "node_modules/@lapismd/jj-conversations");
  const codec = join(
    codecRoot,
    JSON.parse(readFileSync(join(codecRoot, "package.json"))).exports["."]
      .import,
  );
  const fixture = {
    schema: 1,
    provenance: {
      repository: "lapis-desktop",
      revision,
      sourceFiles: Object.fromEntries([...files].sort()),
      publicCodec: {
        package: "@lapismd/jj-conversations",
        entrySha256: hash(readFileSync(codec)),
        mentionsSha256: hash(
          readFileSync(join(dirname(codec), "agent-mentions.js")),
        ),
      },
      generatorSha256: hash(
        readFileSync(new URL("./capture.mjs", import.meta.url)),
      ),
      fixtureSources: Object.fromEntries(
        fixtureSources.map((name) => [
          name,
          hash(readFileSync(new URL(name, import.meta.url))),
        ]),
      ),
    },
    ...results,
  };
  const bytes = JSON.stringify(fixture, null, 2) + "\n";
  if (process.argv.includes("--check")) {
    if (readFileSync(output, "utf8") !== bytes)
      throw Error("Queue source fixture drift");
  } else {
    mkdirSync(dirname(output), { recursive: true });
    writeFileSync(output, bytes);
  }
  console.log(
    JSON.stringify({
      output,
      cases: fixture.cases.length,
      accepted: fixture.cases.filter((x) => x.expected.ok).length,
      configurationFrames: fixture.configurationFrames.length,
      sha256: hash(bytes),
    }),
  );
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
