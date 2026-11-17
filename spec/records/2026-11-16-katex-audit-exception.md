# Approved scoped KaTeX audit exception

The user explicitly approved adding this exception to unblock the plugin-skill
release after reviewing the distinct KaTeX blocker. Approval is separate from
the existing deepmerge exception and does not expand it.

- Advisory: [GHSA-238p-pmpm-9mq7](https://github.com/KaTeX/KaTeX/security/advisories/GHSA-238p-pmpm-9mq7).
- CVE: CVE-2026-103923; upstream severity Low (CVSS 4.0: 2.1).
- Retained version: KaTeX 0.16.47.
- Path: @lapis-notes/language-service -> markdownlint -> micromark-extension-math -> katex.
- Scope: this advisory in the existing Server dependency path only. No other advisory or functional failure is waived.
- Review by: 15 December 2026; remove earlier when a compatible patched dependency chain is released and qualified.
- Remediation owner: Notes Language Service dependency intake; retain normal registry semver ranges and public package contracts.

The upstream advisory requires pre-existing prototype pollution or influence
over renderer-option prototypes. It can bypass trust restrictions and permit
user-interaction XSS or unwanted resource loading when rendered output is
inserted without separate sanitization. This record accepts the residual risk;
it does not claim the dependency is patched or prove the path unexploitable.
Patched KaTeX begins at 0.18.2, while the published math extension requires
0.16.x. No dependency override or vendored implementation is introduced.

Before this change, `pnpm audit` exits 1 for this one Low advisory after the
existing approved deepmerge exception. The audit configuration adds exactly
CVE-2026-103923; all other findings continue to fail normally. The review date
is a required maintenance follow-up, not an automatically enforced expiry.

Release evidence at c81f6b9e: all eight GitHub jobs pass, including both image
architectures. The complete updated hosted runner passes 43 enabled cases with
five existing opt-ins skipped. Prior 41-case counts in intake history precede
this final count. No new quarantine or assertion change accompanies acceptance.

After adding the exact CVE ignore, `pnpm audit` succeeds and still reports the
one Low finding as ignored. Prettier and all canonical specification lanes,
App ownership audit and release-workflow checks pass. This policy-only slice
changes no runtime, package resolution, test selection or functional assertion.
