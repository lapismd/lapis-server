import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-700.css";
import type { TerminalAppearanceOptions } from "@lapis-notes/lapis-plugin-terminal";

/** Server-owned presentation. Other terminal hosts keep their own defaults. */
export const HOSTED_TERMINAL_APPEARANCE = {
  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
  fontSize: 14,
} satisfies TerminalAppearanceOptions;
