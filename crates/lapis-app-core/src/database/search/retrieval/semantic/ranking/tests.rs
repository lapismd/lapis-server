use super::*;
fn entry(path: String) -> Entry {
    Entry {
        path,
        lexical: 0.,
        vector: 0.,
        chunks: vec![],
        lexical_rank: None,
        vector_rank: None,
    }
}
#[test]
fn candidate_scores_are_bounded_without_retaining_document_bodies() {
    let mut bytes = 0;
    let record = entry("x".repeat(12 * 1024 * 1024));
    admit(&mut bytes, &record).unwrap();
    admit(&mut bytes, &record).unwrap();
    assert_eq!(
        admit(&mut bytes, &record).unwrap_err().code(),
        lapis_app_protocol::ErrorCode::InvalidRequest
    );
}
