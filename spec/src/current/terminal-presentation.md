# Server Terminal presentation

Server owns its Terminal appearance through the plugin's public optional font
contract. It bundles JetBrains Mono regular/bold at 14px; other hosts keep their
own defaults. The Server Terminal catalog mounts the production hosted workspace,
host registration, transport adapter and Ghostty renderer in light/dark mode.
Its disposable ANSI transcripts preview compact coloured prompts, successful and
failed commands, listings and nested directories without spawning a process.
These stories qualify presentation; the native application profile has separate
real-PTY acceptance below. Desktop keeps the user's own shell and appearance.

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
Server locks published Terminal 0.1.1; [registry qualification](../../records/2026-11-20-terminal-registry-adoption.md)
records the original two missing-contract errors, their resolution and the
complete catalog/cold reload acceptance through the public package.

Canvas acceptance requires separate visible cyan path, magenta UUID, blue
directory and green/red status pixels, plus the existing aggregate pixel minimum.
A saturation cutoff must not exclude the renderer's muted ANSI palette or
reduce the colour check to the rasterized area of one prompt glyph.

## Native Server shell profile

Server's owning Host selects a managed login-interactive Bash command through
Core's optional, immutable per-vault launch factory. The private PTY engine
provides generic argv/environment configuration; its original constructor and
other hosts retain their existing defaults. Both configured and managed-vault
Server startup paths use the same Host policy.

Before the first prompt, the managed shell loads `/etc/profile` and the first
readable user `.bash_profile`, `.bash_login` or `.profile`. When none exists it
loads `.bashrc`. It preserves scalar prompt commands and supported native array
hooks, their order and command exit status. Private composed hooks are not
exported into nested shells. Prompt paths are cyan, the admitted
managed vault UUID is magenta and shortened to eight characters, and the arrow
is green after success or red after failure. GNU listings use `--color=auto`;
native macOS development retains a colour-capable listing fallback.

Only the displayed prompt abbreviates the admitted vault identity. Real cwd,
`pwd`, file paths, output cursors and protocol/session identities remain full.
Physical root/home aliases are resolved for display comparisons; HOME itself
is unchanged. Cwd bytes are expanded once as data and control characters are
sanitized in the prompt. The bootstrap writes no startup files and sends no
initialization commands through the user's terminal input stream.

Set `LAPIS_SERVER_TERMINAL_PROFILE=0` before Server startup to retain the original
unmanaged login shell. This is a Server-only setting and does not select Desktop
shells or overwrite account configuration. [Native application qualification](../../records/2026-11-20-native-terminal-profile.md)
records real Host/Core/PTY tests; the existing v2.5.1 image predates this follow-up.
