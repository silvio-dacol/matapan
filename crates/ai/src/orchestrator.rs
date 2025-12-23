pub async fn run(prompt: &str, tools: &HashMap<String, Box<dyn Tool>>) {
    let reply = ask(prompt).await?;
    let cmd: serde_json::Value = serde_json::from_str(&reply)?;

    let tool = tools.get(cmd["tool"].as_str().unwrap()).unwrap();
    let result = tool.call(cmd["input"].clone())?;

    // feed result back to model if needed
}
