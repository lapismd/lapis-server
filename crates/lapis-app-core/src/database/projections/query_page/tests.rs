use super::*;

#[test]
fn row_bounds_do_not_cap_the_input_scan_of_a_limited_query() {
    let mut all = Page::new(ProjectionQuery::default()).unwrap();
    let mut limited = Page::new(ProjectionQuery {
        limit: Some(1.0),
        ..Default::default()
    })
    .unwrap();
    let mut excessive = Page::new(ProjectionQuery {
        limit: Some(100_001.0),
        ..Default::default()
    })
    .unwrap();
    for n in 0..=100_000 {
        let id = format!("{n:06}");
        let result = all.push(id.clone(), Map::new());
        if n < 100_000 {
            result.unwrap();
        } else {
            assert!(result.is_err());
        }
        limited.push(id.clone(), Map::new()).unwrap();
        excessive.push(id, Map::new()).unwrap();
    }
    let result = limited.finish(7, ProjectionIndexStatus::Ready).unwrap();
    assert_eq!(result.rows.len(), 1);
    assert!(result.next_cursor.is_some());
    assert_eq!(result.revision, 7);
    assert!(excessive.finish(7, ProjectionIndexStatus::Ready).is_err());
}
