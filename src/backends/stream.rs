use crate::agent::{AgentEvent, ToolCall};
use crate::backends::LlmResponse;
use crate::backends::llm_error::LlmError;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

pub struct LineReader<S> {
    stream: S,
    buffer: Vec<u8>,
}

impl<S, B, E> LineReader<S>
where
    S: Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
        }
    }

    pub async fn next_line(&mut self) -> Result<Option<String>, LlmError> {
        loop {
            if let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.buffer.drain(..=pos).collect();
                return Ok(Some(decode_line(&line_bytes)));
            }

            match self.stream.next().await {
                Some(Ok(chunk)) => self.buffer.extend_from_slice(chunk.as_ref()),
                Some(Err(e)) => {
                    return Err(LlmError::NetworkError {
                        message: e.to_string(),
                    });
                }
                None => {
                    if self.buffer.is_empty() {
                        return Ok(None);
                    }
                    let line_bytes = std::mem::take(&mut self.buffer);
                    return Ok(Some(decode_line(&line_bytes)));
                }
            }
        }
    }
}

fn decode_line(bytes: &[u8]) -> String {
    let mut end = bytes.len();
    if end > 0 && bytes[end - 1] == b'\n' {
        end -= 1;
    }
    if end > 0 && bytes[end - 1] == b'\r' {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Extract the payload of an SSE `data:` line, stripping the field name and one
/// optional leading space. Returns `None` for `event:`, comments, and blanks.
pub fn sse_data(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("data:")?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

pub fn emit_stream_started(tx: &UnboundedSender<AgentEvent>) {
    let _ = tx.send(AgentEvent::StreamStarted);
}

pub fn emit_text_delta(tx: &UnboundedSender<AgentEvent>, text: &str) {
    if !text.is_empty() {
        let _ = tx.send(AgentEvent::TextDelta(text.to_string()));
    }
}

pub fn emit_thinking_delta(tx: &UnboundedSender<AgentEvent>, text: &str) {
    if !text.is_empty() {
        let _ = tx.send(AgentEvent::ThinkingDelta(text.to_string()));
    }
}

/// OpenAI chat-completions streaming, shared by the OpenAI-compatible and
/// Together AI backends (both speak the same `choices[].delta` SSE dialect).
#[derive(Debug, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAiStreamUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: Option<OpenAiStreamDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiStreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamToolCall {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<OpenAiStreamFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiStreamUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Default)]
struct OpenAiToolCallAcc {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
pub struct OpenAiStreamAccumulator {
    text: String,
    reasoning: String,
    tool_calls: std::collections::BTreeMap<usize, OpenAiToolCallAcc>,
    input_tokens: usize,
    output_tokens: usize,
}

impl OpenAiStreamAccumulator {
    /// Apply one streamed chunk, emitting deltas. Returns `true` if the model
    /// signalled it was cut off due to the token limit (`finish_reason == "length"`).
    pub fn apply(
        &mut self,
        chunk: OpenAiStreamChunk,
        event_tx: &UnboundedSender<AgentEvent>,
    ) -> bool {
        if let Some(usage) = chunk.usage {
            let input = if usage.input_tokens > 0 {
                usage.input_tokens as usize
            } else {
                usage.prompt_tokens as usize
            };
            let output = if usage.output_tokens > 0 {
                usage.output_tokens as usize
            } else {
                usage.completion_tokens as usize
            };
            if input > 0 {
                self.input_tokens = input;
            }
            if output > 0 {
                self.output_tokens = output;
            }
        }

        let mut hit_length_limit = false;
        for choice in chunk.choices {
            if choice.finish_reason.as_deref() == Some("length") {
                hit_length_limit = true;
            }
            let Some(delta) = choice.delta else {
                continue;
            };
            if let Some(content) = &delta.content {
                emit_text_delta(event_tx, content);
                self.text.push_str(content);
            }
            if let Some(reasoning) = &delta.reasoning {
                emit_thinking_delta(event_tx, reasoning);
                self.reasoning.push_str(reasoning);
            }
            if let Some(tool_calls) = delta.tool_calls {
                for tc in tool_calls {
                    let entry = self.tool_calls.entry(tc.index).or_default();
                    if let Some(id) = tc.id
                        && !id.is_empty()
                    {
                        entry.id = id;
                    }
                    if let Some(func) = tc.function {
                        if let Some(name) = func.name
                            && !name.is_empty()
                        {
                            entry.name = name;
                        }
                        if let Some(args) = func.arguments {
                            entry.arguments.push_str(&args);
                        }
                    }
                }
            }
        }
        hit_length_limit
    }

    pub fn into_response(self) -> LlmResponse {
        let thinking = if self.reasoning.is_empty() {
            None
        } else {
            Some(self.reasoning)
        };

        let tool_calls: Vec<ToolCall> = self
            .tool_calls
            .into_values()
            .filter(|tc| !tc.name.is_empty())
            .map(|tc| ToolCall {
                id: tc.id,
                r#type: "function".to_string(),
                function: crate::agent::ToolFunction {
                    name: tc.name,
                    arguments: if tc.arguments.trim().is_empty() {
                        "{}".to_string()
                    } else {
                        tc.arguments
                    },
                },
            })
            .collect();

        if !tool_calls.is_empty() {
            let content = if self.text.is_empty() {
                None
            } else {
                Some(self.text)
            };
            LlmResponse::with_tool_calls(content, tool_calls)
                .with_tokens(self.input_tokens, self.output_tokens)
                .with_thinking(thinking)
        } else {
            LlmResponse::content_only(self.text)
                .with_tokens(self.input_tokens, self.output_tokens)
                .with_thinking(thinking)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    fn reader(
        chunks: Vec<&'static [u8]>,
    ) -> LineReader<impl Stream<Item = Result<&'static [u8], Infallible>> + Unpin> {
        LineReader::new(futures::stream::iter(chunks.into_iter().map(Ok)))
    }

    async fn collect(
        mut r: LineReader<impl Stream<Item = Result<&'static [u8], Infallible>> + Unpin>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(line) = r.next_line().await.expect("line") {
            out.push(line);
        }
        out
    }

    #[tokio::test]
    async fn splits_lines_across_chunk_boundaries() {
        let lines = collect(reader(vec![b"hel", b"lo\nwor", b"ld\n"])).await;
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[tokio::test]
    async fn handles_crlf_and_trailing_line_without_newline() {
        let lines = collect(reader(vec![b"a\r\n", b"b"])).await;
        assert_eq!(lines, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn reassembles_utf8_split_across_chunks() {
        // "é" is 0xC3 0xA9 — split between two chunks mid-codepoint.
        let lines = collect(reader(vec![b"caf\xc3", b"\xa9\n"])).await;
        assert_eq!(lines, vec!["café"]);
    }

    #[tokio::test]
    async fn preserves_blank_lines_between_events() {
        let lines = collect(reader(vec![b"data: x\n\ndata: y\n"])).await;
        assert_eq!(lines, vec!["data: x", "", "data: y"]);
    }

    #[test]
    fn sse_data_strips_field_and_one_space() {
        assert_eq!(sse_data("data: {\"a\":1}"), Some("{\"a\":1}"));
        assert_eq!(sse_data("data:{\"a\":1}"), Some("{\"a\":1}"));
        assert_eq!(sse_data("event: message"), None);
        assert_eq!(sse_data(""), None);
    }

    fn apply_chunks(chunks: &[&str]) -> LlmResponse {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut acc = OpenAiStreamAccumulator::default();
        for c in chunks {
            let chunk: OpenAiStreamChunk = serde_json::from_str(c).expect("chunk");
            acc.apply(chunk, &tx);
        }
        acc.into_response()
    }

    #[test]
    fn openai_accumulator_assembles_text_and_usage() {
        let resp = apply_chunks(&[
            r#"{"choices":[{"delta":{"content":"Hel"}}]}"#,
            r#"{"choices":[{"delta":{"content":"lo"}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":11,"completion_tokens":4}}"#,
        ]);
        assert_eq!(resp.content.as_deref(), Some("Hello"));
        assert!(resp.tool_calls.is_none());
        assert_eq!(resp.input_tokens, Some(11));
        assert_eq!(resp.output_tokens, Some(4));
    }

    #[test]
    fn openai_accumulator_assembles_tool_call_across_chunks() {
        let resp = apply_chunks(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{\"path\""}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":":\"a.txt\"}"}}]}}]}"#,
        ]);
        let calls = resp.tool_calls.expect("tool calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].function.name, "read");
        assert_eq!(calls[0].function.arguments, "{\"path\":\"a.txt\"}");
    }

    #[test]
    fn openai_accumulator_collects_reasoning() {
        let resp = apply_chunks(&[
            r#"{"choices":[{"delta":{"reasoning":"think "}}]}"#,
            r#"{"choices":[{"delta":{"reasoning":"more"}}]}"#,
            r#"{"choices":[{"delta":{"content":"done"}}]}"#,
        ]);
        assert_eq!(resp.content.as_deref(), Some("done"));
        assert_eq!(resp.thinking.as_deref(), Some("think more"));
    }
}
