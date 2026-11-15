#!/usr/bin/env node
// Adapted from jj-store scripts/test-vault-ssh.mjs at 2f8f9ec4 (Apache-2.0).
// Real Core/HTTP acceptance; keys, trust, HOME and server state are disposable.
import { spawn, execFileSync } from "node:child_process";
import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  mkdirSync,
  existsSync,
  rmSync,
  realpathSync,
} from "node:fs";
import { homedir, tmpdir, userInfo } from "node:os";
import { join } from "node:path";
import { createServer, connect } from "node:net";
if (!["darwin", "linux"].includes(process.platform))
  throw Error("SSH fixture requires macOS or Linux");
const backupOnly = process.argv[2] === "--backup-only";
const serverOnly = process.argv[2] === "--server-only";
const syncOnly = process.argv[2] === "--sync-only";
if (process.argv.length !== (backupOnly || serverOnly || syncOnly ? 3 : 2))
  throw Error("Use --backup-only, --server-only, --sync-only or no arguments");
// OpenSSH StrictModes rejects a Linux authorized_keys path beneath world-writable /tmp.
// Only fresh disposable fixture state is created here; host profiles are never read.
const base = realpathSync(
  mkdtempSync(
    join(process.platform === "linux" ? homedir() : tmpdir(), ".jj-vault-ssh-"),
  ),
);
const children = [];
const logs = [];
const run = (bin, args, env = process.env) =>
  execFileSync(bin, args, { env, stdio: ["ignore", "pipe", "pipe"] });
const quote = (value) =>
  `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
const pause = () => new Promise((resolve) => setTimeout(resolve, 25));
async function until(check) {
  const end = Date.now() + 5_000;
  while (Date.now() < end) {
    if (await check()) return;
    await pause();
  }
  throw Error("Disposable SSH fixture did not become ready");
}
function daemon(bin, args, env = process.env) {
  const child = spawn(bin, args, { env, stdio: ["ignore", "ignore", "pipe"] });
  child.stderr.on("data", (bytes) => {
    if (logs.join("").length < 16_384) logs.push(bytes.toString());
  });
  children.push(child);
  return child;
}
const listener = createServer();
await new Promise((resolve) => listener.listen(0, "127.0.0.1", resolve));
const port = listener.address().port;
await new Promise((resolve) => listener.close(resolve));
const home = join(base, "home"),
  remote = join(base, "remote.git");
mkdirSync(home, { mode: 0o700 });
const environment = {
  ...process.env,
  HOME: home,
  ZDOTDIR: home,
  BASH_ENV: "/dev/null",
  ENV: "/dev/null",
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
};
try {
  for (const key of ["host", "wrong-host", "identity"])
    run(
      "ssh-keygen",
      ["-q", "-t", "ed25519", "-N", "", "-f", join(base, key)],
      environment,
    );
  run(
    "git",
    ["init", "--quiet", "--bare", "--initial-branch=main", remote],
    environment,
  );
  const host = readFileSync(join(base, "host.pub"), "utf8")
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .join(" ");
  const wrong = readFileSync(join(base, "wrong-host.pub"), "utf8")
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .join(" ");
  writeFileSync(join(base, "known 'hosts'"), `[127.0.0.1]:${port} ${host}\n`, {
    mode: 0o600,
  });
  writeFileSync(
    join(base, "wrong-known-hosts"),
    `[127.0.0.1]:${port} ${wrong}\n`,
    { mode: 0o600 },
  );
  writeFileSync(
    join(base, "authorized_keys"),
    readFileSync(join(base, "identity.pub")),
    { mode: 0o600 },
  );
  const gitExec = run("git", ["--exec-path"], environment).toString().trim();
  const helper = join(base, "git-only.mjs");
  writeFileSync(
    helper,
    `import {spawn} from 'node:child_process';
import {existsSync,unlinkSync,appendFileSync,writeFileSync} from 'node:fs';
const base=${JSON.stringify(base)}, remote=${JSON.stringify(remote)}, tools=${JSON.stringify(gitExec)};
const original=process.env.SSH_ORIGINAL_COMMAND;
const kind=['git-upload-pack','git-receive-pack'].find(kind=>original===kind+" '"+remote+"'");
if(!kind) process.exit(70);
if(kind==='git-upload-pack'&&existsSync(base+'/block-one-observation')) {unlinkSync(base+'/block-one-observation');process.exit(71);}
if(kind==='git-upload-pack'&&existsSync(base+'/hold-next-observation')) {
 writeFileSync(base+'/observation-held','fixture');
 const deadline=Date.now()+15000;
 while(existsSync(base+'/hold-next-observation')&&Date.now()<deadline)await new Promise(resolve=>setTimeout(resolve,25));
 if(existsSync(base+'/hold-next-observation'))process.exit(75);
 unlinkSync(base+'/observation-held');
}
const dropping=kind==='git-receive-pack'&&existsSync(base+'/drop-next-response');
if(kind==='git-receive-pack')appendFileSync(base+'/receive-count','receive\\n');
const child=spawn(tools+'/'+kind,[remote],{stdio:['pipe','pipe','inherit']});
process.stdin.pipe(child.stdin);child.stdin.on('error',()=>{});
let pending=Buffer.alloc(0),advertised=false;
child.stdout.on('data',chunk=>{
 if(!dropping){process.stdout.write(chunk);return;}
 if(advertised)return;
 pending=Buffer.concat([pending,chunk]);
 while(pending.length>=4){const size=parseInt(pending.subarray(0,4).toString(),16);
  if(size===0){process.stdout.write(pending.subarray(0,4));pending=Buffer.alloc(0);advertised=true;break;}
  if(!Number.isInteger(size)||size<4)process.exit(72);
  if(pending.length<size)break;process.stdout.write(pending.subarray(0,size));pending=pending.subarray(size);
 }
});
child.on('close',code=>{if(dropping&&code===0){unlinkSync(base+'/drop-next-response');writeFileSync(base+'/block-one-observation','fixture');process.exitCode=73;}else process.exitCode=code??74;});
`,
    { mode: 0o700 },
  );
  const config = join(base, "sshd_config");
  const values = [
    `Port ${port}`,
    "ListenAddress 127.0.0.1",
    `HostKey ${quote(join(base, "host"))}`,
    `PidFile ${quote(join(base, "sshd.pid"))}`,
    `AuthorizedKeysFile ${quote(join(base, "authorized_keys"))}`,
    `AllowUsers ${userInfo().username}`,
    "AuthenticationMethods publickey",
    "PubkeyAuthentication yes",
    "PasswordAuthentication no",
    "KbdInteractiveAuthentication no",
    "UsePAM no",
    "StrictModes yes",
    "AllowAgentForwarding no",
    "AllowTcpForwarding no",
    "X11Forwarding no",
    "PermitTTY no",
    "UseDNS no",
    "PrintMotd no",
    "PrintLastLog no",
    "LogLevel ERROR",
    "AcceptEnv GIT_PROTOCOL",
    `SetEnv ${quote("HOME=" + home)} ${quote("ZDOTDIR=" + home)} BASH_ENV=/dev/null ENV=/dev/null GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1`,
    `ForceCommand ${quote(process.execPath)} ${quote(helper)}`,
  ];
  writeFileSync(config, values.join("\n") + "\n", { mode: 0o600 });
  const sshd = run("which", ["sshd"], environment).toString().trim();
  daemon(sshd, ["-D", "-e", "-f", config], environment);
  await until(
    () =>
      new Promise((resolve) => {
        const socket = connect({ host: "127.0.0.1", port });
        socket.once("connect", () => {
          socket.destroy();
          resolve(true);
        });
        socket.once("error", () => resolve(false));
      }),
  );
  const socket = join(base, "agent.sock");
  daemon("ssh-agent", ["-D", "-a", socket], environment);
  await until(() => existsSync(socket));
  const testEnv = {
    ...environment,
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? join(homedir(), ".rustup"),
    CARGO_HOME: process.env.CARGO_HOME ?? join(homedir(), ".cargo"),
    SSH_AUTH_SOCK: socket,
    LAPIS_REMOTES_SSH_FIXTURE: base,
    LAPIS_REMOTES_SSH_ENDPOINT: `ssh://${userInfo().username}@127.0.0.1:${port}${remote}`,
  };
  run("ssh-add", [join(base, "identity")], testEnv);
  // Fixture-only preflight retains synthetic diagnostics before sanitized product calls.
  const trust = join(base, "known 'hosts'").replaceAll("'", "'\\''");
  run("git", ["ls-remote", "--", testEnv.LAPIS_REMOTES_SSH_ENDPOINT], {
    ...testEnv,
    GIT_SSH_COMMAND: `ssh -F /dev/null -oBatchMode=yes -oStrictHostKeyChecking=yes -oIdentityFile=none -oGlobalKnownHostsFile=/dev/null '-oUserKnownHostsFile="${trust}"'`,
  });
  const targets = serverOnly
    ? [["lapis-server", "repository_remotes"]]
    : syncOnly
      ? [["lapis-app-core", "repository_synchronization_ssh"]]
      : backupOnly
        ? [["lapis-app-core", "repository_backup_remotes_ssh"]]
        : [
            ["lapis-app-core", "repository_remotes_ssh"],
            ["lapis-app-core", "repository_backup_remotes_ssh"],
            ["lapis-app-core", "repository_synchronization_ssh"],
            ["lapis-server", "repository_remotes"],
          ];
  for (const [owner, target] of targets) {
    const test = spawn(
      "cargo",
      [
        "test",
        "--locked",
        "-p",
        owner,
        "--test",
        target,
        "--",
        "--ignored",
        "--test-threads=1",
      ],
      { env: testEnv, stdio: "inherit" },
    );
    const code = await new Promise((resolve) =>
      test.once("exit", (code) => resolve(code ?? 1)),
    );
    if (code !== 0) throw Error("Real SSH acceptance failed: " + target);
  }
  console.log(
    "Real SSH acceptance passed; disposable agent, host and keys retired.",
  );
} catch (error) {
  console.error(logs.join(""));
  throw error;
} finally {
  for (const child of children) child.kill("SIGTERM");
  await Promise.all(
    children.map((child) =>
      child.exitCode !== null
        ? Promise.resolve()
        : new Promise((resolve) => child.once("exit", resolve)),
    ),
  );
  rmSync(base, { recursive: true, force: true });
}
