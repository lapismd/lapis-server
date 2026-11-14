use super::{ContentError, OperationDescriptor, Value, invalid};
use std::io::Read;

pub(super) fn read(
    descriptor: &OperationDescriptor,
    matches: &clap::ArgMatches,
) -> Result<Value, ContentError> {
    let mut input = if let Some(path) = matches.get_one::<String>("input-file") {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| invalid())?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.len() > 65536 {
            return Err(invalid());
        }
        serde_json::from_slice::<Value>(&bytes)
            .map_err(|_| invalid())?
            .as_object()
            .cloned()
            .ok_or_else(invalid)?
    } else {
        serde_json::Map::new()
    };
    for argument in &descriptor.help.arguments {
        if let Some(value) = matches.get_one::<String>(&argument.field) {
            input.insert(
                argument.field.clone(),
                if argument.json {
                    serde_json::from_str(value).map_err(|_| invalid())?
                } else {
                    Value::String(value.clone())
                },
            );
        }
    }
    Ok(Value::Object(input))
}
