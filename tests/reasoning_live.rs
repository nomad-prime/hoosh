//! Live reasoning check against the real configured backend.
//!
//! Ignored by default — it makes a network call. Run it in the environment
//! where the backend key is set (the same env the app uses); config loading
//! resolves `${env:...}` placeholders itself, so the key is never passed in:
//!
//!     cargo test --test reasoning_live -- --ignored --nocapture
//!
//! It drives the real streaming path and reports whether reasoning shows up as
//! ThinkingDelta events and in the assembled response. Bedrock adaptive-thinking
//! models (opus-4-7/4-8, fable-5) default thinking `display` to `omitted` and
//! need `reasoning_display = "summarized"` to surface reasoning text — see
//! `live_bedrock_adaptive_display_surfaces_reasoning`.

use hoosh::backends::backend_factory::create_backend;
use hoosh::config::{ReasoningDisplay, ReasoningEffort};
use hoosh::{
    AgentEvent, AppConfig, Conversation, LlmBackend, OpenAICompatibleBackend,
    OpenAICompatibleConfig, ToolRegistry,
};

#[tokio::test]
#[ignore = "hits the live backend; run with --ignored in an env with the key set"]
async fn live_backend_streams_reasoning() {
    let config = AppConfig::load().expect("load real config (resolves ${env:...})");
    let backend_name = config.default_backend.clone();
    let backend = create_backend(&backend_name, &config).expect("create backend");
    eprintln!(
        "backend: {backend_name}, streaming: {}",
        backend.supports_streaming()
    );

    let mut conversation = Conversation::new();
    conversation.add_user_message(
        "Reason step by step before answering. A bat and a ball cost $1.10 total. \
         The bat costs $1.00 more than the ball. How much does the ball cost?"
            .to_string(),
    );

    let tools = ToolRegistry::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
    let response = backend
        .send_message_with_tools_and_events(&conversation, &tools, Some(tx))
        .await
        .expect("backend call");

    let mut thinking_deltas = String::new();
    let mut delta_count = 0usize;
    while let Ok(event) = rx.try_recv() {
        if let AgentEvent::ThinkingDelta(t) = event {
            delta_count += 1;
            thinking_deltas.push_str(&t);
        }
    }

    eprintln!("ThinkingDelta events: {delta_count}");
    eprintln!("streamed reasoning:\n{thinking_deltas}");
    eprintln!(
        "response.thinking ({} chars):\n{}",
        response.thinking.as_deref().map(str::len).unwrap_or(0),
        response.thinking.as_deref().unwrap_or("<none>")
    );
    eprintln!(
        "response.content:\n{}",
        response.content.as_deref().unwrap_or("<none>")
    );
}

/// Bedrock adaptive-thinking models (opus-4-7/4-8, fable-5) default thinking
/// `display` to `omitted`, so reasoning text is withheld unless we send
/// `thinking: {type: adaptive, display: summarized}`. This drives that path
/// through hoosh's real backend and asserts reasoning is surfaced.
#[tokio::test]
#[ignore = "hits the live backend; run with --ignored in an env with the key set"]
async fn live_bedrock_adaptive_display_surfaces_reasoning() {
    let config = AppConfig::load().expect("load real config");
    let backend_name = config.default_backend.clone();
    let bc = config.backends.get(&backend_name).expect("backend config");

    let backend = OpenAICompatibleBackend::new(OpenAICompatibleConfig {
        name: backend_name.clone(),
        api_key: bc.api_key.clone().unwrap_or_default(),
        model: "claude-opus-4-8".to_string(),
        base_url: bc.base_url.clone().unwrap_or_default(),
        chat_api: bc
            .chat_api
            .clone()
            .unwrap_or_else(|| "/chat/completions".to_string()),
        temperature: bc.temperature,
        pricing_endpoint: bc.pricing_endpoint.clone(),
        thinking_budget: None,
        reasoning_effort: Some(ReasoningEffort::High),
        reasoning_display: Some(ReasoningDisplay::Summarized),
        streaming: true,
    })
    .expect("backend");

    let mut conversation = Conversation::new();
    conversation.add_user_message(
        "Find the remainder when 7^(7^7) is divided by 100. Show rigorous reasoning.".to_string(),
    );

    let tools = ToolRegistry::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
    let response = backend
        .send_message_with_tools_and_events(&conversation, &tools, Some(tx))
        .await
        .expect("backend call");

    let mut delta_count = 0usize;
    while let Ok(event) = rx.try_recv() {
        if let AgentEvent::ThinkingDelta(_) = event {
            delta_count += 1;
        }
    }

    eprintln!("opus-4-8 ThinkingDelta events: {delta_count}");
    eprintln!(
        "opus-4-8 response.thinking ({} chars):\n{}",
        response.thinking.as_deref().map(str::len).unwrap_or(0),
        response.thinking.as_deref().unwrap_or("<none>")
    );

    assert!(
        response.thinking.as_deref().is_some_and(|t| !t.is_empty()),
        "opus-4-8 with display:summarized must surface reasoning text"
    );
}
