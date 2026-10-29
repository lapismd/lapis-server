use std::collections::HashSet;
use std::sync::OnceLock;

pub(super) fn normalized(text: &str) -> String {
    crate::text::trim(text).to_lowercase()
}
fn hash(units: impl IntoIterator<Item = u16>) -> u32 {
    units.into_iter().fold(2166136261, |hash, n| {
        (hash ^ n as u32).wrapping_mul(16777619)
    })
}
pub(super) fn fingerprint(text: &str) -> String {
    format!("{:x}", hash(normalized(text).encode_utf16()))
}
fn words() -> &'static regress::Regex {
    static WORDS: OnceLock<regress::Regex> = OnceLock::new();
    WORDS.get_or_init(|| {
        regress::Regex::with_flags(r"[\p{L}\p{N}_-]+", "u").expect("retained feature expression")
    })
}
pub(super) fn has_features(text: &str) -> bool {
    words().find_iter(text).next().is_some()
}
pub(super) fn vector(text: &str, dimensions: usize) -> Vec<f64> {
    let words = words();
    let text = normalized(text);
    let mut features = HashSet::<Vec<u16>>::new();
    for word in words.find_iter(&text) {
        let token = &text[word.range()];
        features.insert(format!("tok:{token}").encode_utf16().collect());
        for tri in token.encode_utf16().collect::<Vec<_>>().windows(3) {
            features.insert("tri:".encode_utf16().chain(tri.iter().copied()).collect());
        }
    }
    let mut vector = vec![0.0; dimensions];
    for feature in features {
        let h = hash(feature);
        vector[h as usize % dimensions] += if h & 1 == 0 { 1.0 } else { -1.0 };
    }
    let magnitude = vector.iter().map(|n| n * n).sum::<f64>().sqrt();
    if magnitude > 0.0 {
        for n in &mut vector {
            *n /= magnitude;
        }
    }
    vector
}
