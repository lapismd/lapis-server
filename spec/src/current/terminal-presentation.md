# Server Terminal presentation

Server owns its Terminal appearance through the plugin's public optional font
contract. It bundles JetBrains Mono regular/bold at 14px; other hosts keep their
own defaults. The Server Terminal catalog mounts the production hosted workspace,
host registration, transport adapter and Ghostty renderer in light/dark mode.
Its disposable ANSI transcripts preview compact coloured prompts, successful and
failed commands, listings and nested directories without spawning a process.
These stories qualify presentation only; native Bash profile work and actual PTY
acceptance remain separate. Desktop keeps the user's own shell and appearance.

Server Terminal stories start with both sidebars collapsed and check the actual
canvas background against the workspace theme. Dedicated cold Storybook reload
acceptance checks glyph integrity and the collapsed left sidebar repeatedly in
light and dark mode. Runtime default-colour changes use Terminal's qualified
private Ghostty build; registry adoption requires Terminal 0.1.1 or newer.

The Server Terminal review includes independent numbered terminal tabs. Open
Terminal reveals a retained terminal when no terminal is visible and creates a
new one when a terminal is already visible. The live panel offers an accessible
New Terminal plus button. Canvas geometry fills the panel width, including space
smaller than a character cell, with the overlay scrollbar at the right edge.
Terminal fixture backends belong to each rendered App instance; shared loader or
constant backend state must not survive replacement of the owning fixture.

Terminal now ships a private bundled Ghostty Web 0.4.0 runtime, matching WASM
and patch provenance. Server therefore has no nested ghostty-web optimizer input
or Ghostty archive override. The user-authorized maintained patch removes the
Ghostty release prerequisite. Registry adoption must consume the released Terminal
package through ordinary registry ranges and a frozen lockfile, without archive hooks.
