use crate::{constants::SHUTTLE_DOCS_SEARCH_BASE_URL, utils::build_client};

pub async fn search_docs(query: String) -> Result<String, String> {
    let url = format!(
        "{SHUTTLE_DOCS_SEARCH_BASE_URL}/search?q={}",
        urlencoding::encode(&query)
    );

    let client = build_client()?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if response.status().is_success() {
        response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))
    } else {
        Err(format!("Request failed with status: {}", response.status()))
    }
}
