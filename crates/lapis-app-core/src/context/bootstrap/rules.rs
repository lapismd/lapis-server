// SPDX-License-Identifier: Apache-2.0
// Retained Desktop bootstrap policy; never executable or filesystem authority.
use regress::Regex;
use std::sync::OnceLock;

pub(super) fn has_host_path(value: &str) -> bool {
    static PATHS: OnceLock<Regex> = OnceLock::new();
    PATHS.get_or_init(|| {
        Regex::with_flags(r#"(?:(?:^|[\s"'>=])(?:/|[A-Za-z]:\\|file://)|\.lapis/skills/|\.agents/(?:user/)?skills/)"#, "u")
            .expect("retained bootstrap path expression")
    }).find(value).is_some()
}

pub(super) fn instructions() -> Vec<String> {
    [
        "You are operating as an AI assistant inside Lapis Notes.",
        "Use application-provided tools when interacting with notes. Use note search tools when the user asks you to find, remember, locate, compare, or research information in their notes.",
        "Do not manually inspect application storage when an equivalent Lapis tool exists.",
        "The host controls your accessible note scope. Treat the supplied scope as authoritative.",
        "Skills describe repeatable workflows. When a relevant skill is available, load its instructions with skills_read before following that workflow.",
        "Keep note operations scoped to the current conversation unless the user explicitly requests a broader scope and the host permits it.",
    ].into_iter().map(str::to_owned).collect()
}
