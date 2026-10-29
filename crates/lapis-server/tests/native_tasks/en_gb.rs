use super::route;
use crate::support::Fixture;
use serde_json::{Value, json};

#[tokio::test]
async fn native_tasks_en_gb_and_us_week_contexts_uses_the_ordered_journal_and_default_daily_writer()
{
    use lapis_app_protocol::{Outcome, VaultCommand, VaultValue};
    let corpus: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lapis-app-core/tests/fixtures/task-lifecycle.json"
    )))
    .unwrap();
    let case = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "complete / schedule / count")
        .unwrap();
    for (locale, first_day, minimal_days) in [("en-GB", 1, 4), ("en-US", 7, 1), ("en-US", 7, 4)] {
        let f = Fixture::with_database().await;
        std::fs::create_dir(f.roots[0].path().join("tasks")).unwrap();
        std::fs::write(
            f.roots[0].path().join("tasks/Read.md"),
            case["raw"].as_str().unwrap(),
        )
        .unwrap();
        let client = f.attach(f.ids[0]).await;
        let url = route(&f, client.client_id);
        let call = |command: Value| {
            let f = &f;
            let url = &url;
            async move {
                let response = f
                    .authorize(f.client.post(url))
                    .json(&command)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), 200);
                response.json::<Value>().await.unwrap()
            }
        };
        let selected=call(json!({"kind":"configure","configuration":{"day_parts":case["settings"],"daily":{"folder":"Journal/GB","date_format":"dd-LL-yyyy"},"format":{"locale":locale,"firstDay":first_day,"minimalDays":minimal_days},"provider":{"kind":"default"}}})).await;
        assert_eq!(selected["status"]["enabled"], true);
        let Outcome::Ok {
            value: VaultValue::Text { snapshot },
        } = f
            .execute(
                &client,
                VaultCommand::ReadText {
                    path: "tasks/Read.md".into(),
                },
            )
            .await
            .outcome
        else {
            panic!("source snapshot")
        };
        let request = json!({"request_id":uuid::Uuid::new_v4(),"path":"tasks/Read.md","expected_version":snapshot.version,"action":case["actions"][0]});
        let result = call(json!({"kind":"submit","request":request})).await;
        assert_eq!(result["operation"]["disposition"], "completed");
        assert_eq!(
            result["operation"]["task_replacement"],
            case["expected"]["attempts"][0]["taskRaw"]
        );
        assert_eq!(
            result["operation"]["daily"]["path"],
            "Journal/GB/20-08-2026.md"
        );
        assert_eq!(
            std::fs::read_to_string(f.roots[0].path().join("Journal/GB/20-08-2026.md")).unwrap(),
            result["operation"]["daily"]["text"]
        );
        assert_eq!(
            call(json!({"kind":"submit","request":request})).await,
            result
        );
        let host = f.host.clone();
        tokio::task::spawn_blocking(move || host.shutdown())
            .await
            .unwrap()
            .unwrap();
    }
}
