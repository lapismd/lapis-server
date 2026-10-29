use lapis_app_protocol::database::projections::queries::*;
use serde_json::json;

#[test]
fn projection_query_contracts_retain_ast_fields_and_reject_unknown_transport_inputs() {
    let query = json!({"where":{"op":"and","operands":[{"op":"not","operand":{"op":"exists","field":"literal.key"}},{"op":"in","field":"rank","values":[null,1.0,"1",true]}]},"select":["id"],"orderBy":[{"field":"rank","direction":"desc","nulls":"first"}],"after":"opaque","limit":2.0,"includeStale":true});
    let parsed: ProjectionQuery = serde_json::from_value(query.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), query);
    for bad in [
        json!({"sql":"SELECT * FROM files"}),
        json!({"where":{"op":"compare","field":"rank","comparison":"regex","value":1}}),
        json!({"where":{"op":"in","field":"rank","values":[{"sql":"x"}]}}),
        json!({"where":{"op":"not","operand":{"op":"exists","field":"rank","sql":"x"}}}),
        json!({"orderBy":[{"field":"rank","direction":"sideways"}]}),
    ] {
        assert!(serde_json::from_value::<ProjectionQuery>(bad).is_err());
    }
    let related = json!({"projectionId":"books/book","rowId":"a","relation":"child","direction":"in","targetWhere":{"op":"compare","field":"rank","comparison":"gt","value":1},"limit":1});
    let _: ProjectionRelatedQuery = serde_json::from_value(related.clone()).unwrap();
    let mut bad = related;
    bad["root"] = json!("/tmp/other");
    assert!(serde_json::from_value::<ProjectionRelatedQuery>(bad).is_err());
    let result = ProjectionQueryResult {
        rows: vec![json!({"id":"a"})],
        next_cursor: Some("opaque".into()),
        revision: 1,
        complete: true,
        index_status: ProjectionIndexStatus::Ready,
    };
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({"rows":[{"id":"a"}],"nextCursor":"opaque","revision":1,"complete":true,"indexStatus":"ready"})
    );
}
