use lapis_task_services::{
    TaskOccurrenceCompletion, TaskOccurrenceMaterialization, TaskOccurrenceObservation,
    TaskTransformError, complete_task_occurrence, materialize_task_occurrence,
    scan_task_occurrence_observations,
};

fn failure(message: String) -> TaskTransformError {
    TaskTransformError {
        name: "Error".into(),
        code: None,
        message,
        parts: None,
    }
}

pub(super) fn process(
    raw: &str,
    path: &str,
    input: &TaskOccurrenceMaterialization,
    completion: Option<&TaskOccurrenceCompletion>,
) -> Result<(String, TaskOccurrenceObservation), TaskTransformError> {
    let materialized = materialize_task_occurrence(raw, path, input)?;
    let matches = |value: &TaskOccurrenceObservation| {
        value.record.task_id == input.task_id
            && value.record.occurrence_date == input.occurrence_date
    };
    let text = match completion {
        None => materialized,
        Some(completion) => {
            let observation = scan_task_occurrence_observations(&materialized, path)
                .into_iter()
                .find(matches)
                .ok_or_else(|| {
                    failure(format!(
                        "Failed to materialize daily observation for {}.",
                        input.task_id
                    ))
                })?;
            complete_task_occurrence(&materialized, path, &observation, completion)?
        }
    };
    let mut observations = scan_task_occurrence_observations(&text, path)
        .into_iter()
        .filter(matches);
    let first = observations.next();
    if observations.next().is_some() || first.is_none() {
        return Err(failure(format!(
            "Expected one daily observation for {} on {}.",
            input.task_id, input.occurrence_date
        )));
    }
    Ok((text, first.expect("checked one observation")))
}
