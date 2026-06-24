use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Mock,
    Anthropic,
    OpenAi,
    Groq,
    TogetherAi,
    Ollama,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendKind::Mock => "mock",
            BackendKind::Anthropic => "anthropic",
            BackendKind::OpenAi => "openai",
            BackendKind::Groq => "groq",
            BackendKind::TogetherAi => "together_ai",
            BackendKind::Ollama => "ollama",
        }
    }

    pub fn default_model(&self) -> Option<&'static str> {
        match self {
            BackendKind::Anthropic => Some("claude-sonnet-4.5"),
            BackendKind::OpenAi => Some("gpt-4"),
            BackendKind::TogetherAi => Some("meta-llama/Llama-2-7b-chat-hf"),
            BackendKind::Ollama => Some("llama3.1"),
            BackendKind::Groq | BackendKind::Mock => None,
        }
    }

    pub fn default_base_url(&self) -> Option<&'static str> {
        match self {
            BackendKind::Anthropic => Some("https://api.anthropic.com/v1"),
            BackendKind::OpenAi => Some("https://api.openai.com/v1"),
            BackendKind::TogetherAi => Some("https://api.together.xyz/v1"),
            BackendKind::Ollama => Some("http://localhost:11434"),
            BackendKind::Groq | BackendKind::Mock => None,
        }
    }

    pub fn default_chat_api(&self) -> Option<&'static str> {
        match self {
            BackendKind::OpenAi => Some("/chat/completions"),
            _ => None,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            BackendKind::Anthropic => "Claude AI models (Sonnet, Opus, Haiku)",
            BackendKind::OpenAi => "GPT models (GPT-4, GPT-3.5)",
            BackendKind::Groq => "Fast inference for open source models",
            BackendKind::TogetherAi => "Various open source models",
            BackendKind::Ollama => "Local LLM inference (no API key needed)",
            BackendKind::Mock => "Deterministic backend for testing",
        }
    }

    pub fn needs_api_key(&self) -> bool {
        !matches!(self, BackendKind::Ollama | BackendKind::Mock)
    }

    pub fn is_available(&self) -> bool {
        match self {
            BackendKind::Mock => true,
            BackendKind::TogetherAi => cfg!(feature = "together-ai"),
            BackendKind::Anthropic => cfg!(feature = "anthropic"),
            BackendKind::Ollama => cfg!(feature = "ollama"),
            BackendKind::OpenAi | BackendKind::Groq => cfg!(feature = "openai-compatible"),
        }
    }

    pub fn all() -> &'static [BackendKind] {
        &[
            BackendKind::Mock,
            BackendKind::Anthropic,
            BackendKind::OpenAi,
            BackendKind::Groq,
            BackendKind::TogetherAi,
            BackendKind::Ollama,
        ]
    }

    pub fn available() -> Vec<BackendKind> {
        BackendKind::all()
            .iter()
            .copied()
            .filter(BackendKind::is_available)
            .collect()
    }

    pub fn user_selectable() -> Vec<BackendKind> {
        [
            BackendKind::Anthropic,
            BackendKind::OpenAi,
            BackendKind::TogetherAi,
            BackendKind::Ollama,
        ]
        .into_iter()
        .filter(|k| k.is_available())
        .collect()
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BackendKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mock" => Ok(BackendKind::Mock),
            "anthropic" => Ok(BackendKind::Anthropic),
            "openai" => Ok(BackendKind::OpenAi),
            "groq" => Ok(BackendKind::Groq),
            "together_ai" => Ok(BackendKind::TogetherAi),
            "ollama" => Ok(BackendKind::Ollama),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_string() {
        for kind in BackendKind::all() {
            assert_eq!(BackendKind::from_str(kind.as_str()), Ok(*kind));
        }
    }

    #[test]
    fn available_reflects_compiled_features() {
        let available = BackendKind::available();
        assert!(available.contains(&BackendKind::Mock));
        for kind in &available {
            assert!(kind.is_available());
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert_eq!(BackendKind::from_str("nope"), Err(()));
    }
}
