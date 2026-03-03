use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

pub async fn embed(text: &str) -> Result<Vec<f32>, String> {
    let client = reqwest::Client::new();

    let res = client
        .post("http://localhost:11434/api/embeddings")
        .json(&EmbedRequest {
            model: "nomic-embed-text",
            prompt: text,
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: EmbedResponse = res.json().await.map_err(|e| e.to_string())?;

    Ok(body.embedding)
}