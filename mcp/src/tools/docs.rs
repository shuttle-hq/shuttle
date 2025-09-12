use crate::{constants::SHUTTLE_DOCS_SEARCH_BASE_URL, utils::build_client};

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchDocsArgs {
    #[schemars(description = "Search query for documentation")]
    query: String,
    #[schemars(description = "Maximum number of tokens to retrieve (default: 4000)")]
    max_tokens: Option<u32>,
}

pub async fn search_docs(params: SearchDocsArgs) -> Result<String, String> {
    let url = format!(
        "{SHUTTLE_DOCS_SEARCH_BASE_URL}/search?q={}&maxTokens={}",
        urlencoding::encode(&params.query),
        params.max_tokens.unwrap_or(4000)
    );

    let client = build_client()?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if response.status().is_success() {
        response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {e}"))
    } else {
        Err(format!("Request failed with status: {}", response.status()))
    }
}
