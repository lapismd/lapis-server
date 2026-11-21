fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        serde_json::to_string_pretty(&lapis_server::http::openapi_document())?
    );
    Ok(())
}
