const c = (code: number, text: string) => `\x1b[${code}m${text}\x1b[0m`;
export type TerminalScenario = "ready" | "failure" | "nested";
export const VAULT_UUID = "92b3bbf0-07f6-468e-a424-06d7bbdba797";

/** Design-review data, not a shell implementation or evidence of PTY acceptance. */
export function terminalTranscript(scenario: TerminalScenario) {
  let directory = scenario === "nested" ? "/notes" : "";
  let failed = false;
  const prompt = () =>
    `${c(36, "~/vaults/")}${c(35, "92b3bbf0")}${c(36, `/content${directory}`)} ${c(failed ? 31 : 32, "❯")} `;
  const listing = () =>
    `${c(34, "attachments/")}  ${c(34, "notes/")}  ${c(34, "scripts/")}\r\n` +
    `${c(32, "backup.sh")}     index.md  Welcome.md\r\n`;
  const initial =
    `${c(1, "Lapis Server")} ${c(90, "· vault shell")}\r\n\r\n` +
    `${prompt()}ls\r\n${listing()}\r\n` +
    (scenario === "failure"
      ? `${prompt()}cat missing.md\r\n${c(31, "cat: missing.md: No such file or directory")}\r\n\r\n`
      : "") +
    ((failed = scenario === "failure"), prompt());
  function run(line: string) {
    failed = false;
    const command = line.trim();
    let output = "";
    if (command === "clear") return `\x1b[2J\x1b[H${prompt()}`;
    if (command === "ls" || command === "ls -la") output = listing();
    else if (command === "pwd")
      output = `/data/vaults/${VAULT_UUID}/content${directory}\r\n`;
    else if (command === "cd notes") directory = "/notes";
    else if (command === "cd .." || command === "cd") directory = "";
    else if (command === "false") failed = true;
    else if (command) {
      failed = true;
      output = `${c(31, `${command}: command not found`)}\r\n`;
    }
    return `\r\n${output}${prompt()}`;
  }
  return { initial, run };
}
