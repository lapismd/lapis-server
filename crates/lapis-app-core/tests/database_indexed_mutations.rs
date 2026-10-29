use lapis_app_core::database::Database;
use serde_json::json;
use uuid::Uuid;
mod indexed_mutation_support;
mod projection_support;
mod search_support;
use indexed_mutation_support::{fixture, run, seed, snapshot};
use search_support::assert_json;

#[tokio::test]
async fn native_indexed_mutations_retain_the_complete_direct_turso_reference() {
    for case in fixture()["cases"].as_array().unwrap() {
        let profile = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        let db = Database::open(profile.path(), id).await.unwrap();
        seed(&db, &case["seed"]).await;
        let before = db.revision().await.unwrap();
        eprintln!("retained case: {}", case["name"]);
        assert_json(
            run(&db, case["steps"].as_array().unwrap()).await,
            case["expected"].clone(),
        );
        db.close().await.unwrap();
        let journal = projection_support::inspect(profile.path(),id,&format!("SELECT revision,domains_json,paths_json,renamed_json FROM app_changes WHERE revision>{before} ORDER BY revision")).await;
        let expected = case["expected"]["events"].as_array().unwrap();
        assert_eq!(journal.len(), expected.len());
        for (row, event) in journal.iter().zip(expected) {
            assert_json(
                row[0].clone(),
                json!(event["revision"].as_u64().unwrap() + before),
            );
            for (column, key) in [(1, "domains"), (2, "paths"), (3, "renamed")] {
                assert_json(
                    serde_json::from_str(row[column].as_str().unwrap()).unwrap(),
                    event[key].clone(),
                );
            }
        }
        let db = Database::open(profile.path(), id).await.unwrap();
        assert_json(
            snapshot(&db).await,
            case["expected"]["states"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["state"]
                .clone(),
        );
        db.close().await.unwrap();
    }
}
