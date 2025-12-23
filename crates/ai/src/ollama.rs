pub async fn ask(prompt: &str) -> anyhow::Result<String> {
    let body = serde_json::json!({
        "model": "qwen2.5:7b",
        "prompt": prompt,
        "stream": false
    });

    let res = reqwest::Client::new()
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    Ok(res["response"].as_str().unwrap().to_string())
}
