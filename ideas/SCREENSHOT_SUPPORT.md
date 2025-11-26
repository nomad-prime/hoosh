# Screenshot Support for Hoosh

## Overview

This document outlines the design and implementation plan for adding screenshot/image posting capability to hoosh, similar to Claude Code's ability to accept images in user input.

## Research Summary

### Cross-Backend Compatibility

All four LLM backends that hoosh supports have vision capabilities:

#### ✅ OpenAI-Compatible (OpenAI, GPT-4o, etc.)
- Uses `content` as array with `{"type": "text"}` and `{"type": "image_url"}` blocks
- Base64 format: `"data:image/jpeg;base64,BASE64_DATA"`
- Max 10 images per request
- Supported models: GPT-4o, GPT-4-turbo, GPT-5 series (2025)

#### ✅ Anthropic (Claude 3/4)
- Uses content blocks: `{"type": "text"}` and `{"type": "image"}`
- Base64 format: `{"type": "base64", "media_type": "image/jpeg", "data": "BASE64"}`
- Max 20 images per turn, 10MB each
- Supported models: Claude 3 family, Claude 4 family (Sonnet, Opus)

#### ✅ Together AI
- OpenAI-compatible format: array with `image_url` type
- Base64 format: `"data:image/jpeg;base64,BASE64_DATA"`
- Supported models: Llama Vision (11B, 90B), Qwen2.5-VL (72B)
- Context: 128K tokens, 1120x1120 images

#### ✅ Ollama (Local models like LLaVA)
- Supports `images` parameter with base64 arrays
- Works with LLaVA 7B/13B/34B models
- Runs locally with 16GB RAM minimum
- Supports up to 4x higher resolution in LLaVA 1.6

### Clipboard Library

Hoosh already uses `arboard` (v3.4) which has full image support:
- `get_image()` → `Result<ImageData<'static>, Error>`
- `ImageData` contains: `width`, `height`, `bytes` (RGBA format)
- Cross-platform: macOS (NSImage), Linux (PNG), Windows (CF_DIB/CF_BITMAP)

---

## Unified Design

### 1. Message Content Structure Changes

**Note:** Hoosh does not require backward compatibility with old conversations, so we can simplify the design.

#### Current Structure
```rust
pub struct ConversationMessage {
    pub role: String,
    pub content: Option<String>,  // ❌ Only text
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}
```

#### New Structure (Simplified - No Backward Compatibility)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String
    },
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,  // "low" | "high" | "auto" for OpenAI
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    pub media_type: String,  // "image/png", "image/jpeg", "image/webp"
    pub data: String,        // base64 encoded data
}

pub struct ConversationMessage {
    pub role: String,
    pub content: Vec<ContentBlock>,  // ✅ Always an array of content blocks
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}
```

#### Helper Methods
```rust
impl ConversationMessage {
    pub fn text(role: String, text: String) -> Self {
        Self {
            role,
            content: vec![ContentBlock::Text { text }],
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn with_content(role: String, content: Vec<ContentBlock>) -> Self {
        Self {
            role,
            content,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn get_text(&self) -> Option<&str> {
        self.content.iter()
            .find_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
    }

    pub fn has_images(&self) -> bool {
        self.content.iter().any(|b| matches!(b, ContentBlock::Image { .. }))
    }

    pub fn image_count(&self) -> usize {
        self.content.iter()
            .filter(|b| matches!(b, ContentBlock::Image { .. }))
            .count()
    }
}
```

### 2. Backend-Specific Conversion

Each backend needs to convert the unified format to its API-specific format:

#### Anthropic Backend (src/backends/anthropic.rs)
Anthropic already uses content blocks internally, so minimal changes needed:

```rust
// Extend existing ContentBlock enum
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    // NEW: Add image support
    Image {
        source: ImageSourceBlock,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ImageSourceBlock {
    r#type: String,  // "base64"
    media_type: String,  // "image/png", "image/jpeg"
    data: String,  // base64 data
}

// In convert_messages(), handle MessageContent::Blocks with images
fn convert_message_content(content: &MessageContent) -> AnthropicContent {
    match content {
        MessageContent::Text(text) => AnthropicContent::Text(text.clone()),
        MessageContent::Blocks(blocks) => {
            let anthropic_blocks: Vec<ContentBlock> = blocks.iter().map(|block| {
                match block {
                    ContentBlock::Text { text } => ContentBlock::Text { text: text.clone() },
                    ContentBlock::Image { source, .. } => ContentBlock::Image {
                        source: ImageSourceBlock {
                            r#type: "base64".to_string(),
                            media_type: source.media_type.clone(),
                            data: source.data.clone(),
                        }
                    },
                }
            }).collect();
            AnthropicContent::Blocks(anthropic_blocks)
        }
    }
}
```

#### OpenAI/Together AI Backend (src/backends/openai_compatible.rs)
Convert to OpenAI's array format with `image_url`:

```rust
// Update ConversationMessage serialization for OpenAI
#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: OpenAIContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenAIContent {
    Text(String),
    Array(Vec<OpenAIContentPart>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAIContentPart {
    Text { text: String },
    ImageUrl { image_url: OpenAIImageUrl },
}

#[derive(Debug, Serialize)]
struct OpenAIImageUrl {
    url: String,  // "data:image/png;base64,..."
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,  // "low" | "high" | "auto"
}

fn convert_to_openai_message(msg: &ConversationMessage) -> OpenAIMessage {
    let content = match &msg.content {
        MessageContent::Text(text) => OpenAIContent::Text(text.clone()),
        MessageContent::Blocks(blocks) => {
            let parts: Vec<OpenAIContentPart> = blocks.iter().map(|block| {
                match block {
                    ContentBlock::Text { text } => OpenAIContentPart::Text {
                        text: text.clone()
                    },
                    ContentBlock::Image { source, detail } => OpenAIContentPart::ImageUrl {
                        image_url: OpenAIImageUrl {
                            url: format!("data:{};base64,{}", source.media_type, source.data),
                            detail: detail.clone(),
                        }
                    },
                }
            }).collect();
            OpenAIContent::Array(parts)
        }
    };

    OpenAIMessage {
        role: msg.role.clone(),
        content,
        tool_calls: msg.tool_calls.clone(),
        tool_call_id: msg.tool_call_id.clone(),
        name: msg.name.clone(),
    }
}
```

#### Ollama Backend (src/backends/ollama.rs)
Ollama uses a separate `images` parameter:

```rust
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ModelOptions>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,  // NEW: base64 image strings
}

fn convert_messages_and_extract_images(
    messages: &[ConversationMessage]
) -> (Vec<OllamaMessage>, Option<Vec<String>>) {
    let mut ollama_messages = Vec::new();
    let mut all_images = Vec::new();

    for msg in messages {
        let (text_content, images) = match &msg.content {
            MessageContent::Text(text) => (Some(text.clone()), vec![]),
            MessageContent::Blocks(blocks) => {
                let mut texts = Vec::new();
                let mut imgs = Vec::new();

                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => texts.push(text.clone()),
                        ContentBlock::Image { source, .. } => imgs.push(source.data.clone()),
                    }
                }

                let combined_text = if texts.is_empty() {
                    None
                } else {
                    Some(texts.join("\n"))
                };

                (combined_text, imgs)
            }
        };

        all_images.extend(images);

        ollama_messages.push(OllamaMessage {
            role: msg.role.clone(),
            content: text_content,
            tool_calls: convert_tool_calls(&msg.tool_calls),
            tool_call_id: msg.tool_call_id.clone(),
            name: msg.name.clone(),
        });
    }

    let images_param = if all_images.is_empty() {
        None
    } else {
        Some(all_images)
    };

    (ollama_messages, images_param)
}
```

### 3. Clipboard Integration

Extend the ClipboardManager to support images:

```rust
// src/tui/clipboard.rs
use anyhow::Result;
use arboard::{Clipboard, ImageData};
use image::{ImageEncoder, ColorType};

pub struct ClipboardManager {
    clipboard: Option<Clipboard>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        let clipboard = Clipboard::new().ok();
        Self { clipboard }
    }

    pub fn get_text(&mut self) -> Result<String> {
        if let Some(clipboard) = &mut self.clipboard {
            clipboard
                .get_text()
                .map_err(|e| anyhow::anyhow!("Failed to get clipboard text: {}", e))
        } else {
            Err(anyhow::anyhow!("Clipboard not available"))
        }
    }

    pub fn set_text(&mut self, text: String) -> Result<()> {
        if let Some(clipboard) = &mut self.clipboard {
            clipboard
                .set_text(text)
                .map_err(|e| anyhow::anyhow!("Failed to set clipboard text: {}", e))
        } else {
            Err(anyhow::anyhow!("Clipboard not available"))
        }
    }

    // NEW: Get image from clipboard
    pub fn get_image(&mut self) -> Result<Option<ImageData<'static>>> {
        if let Some(clipboard) = &mut self.clipboard {
            match clipboard.get_image() {
                Ok(img_data) => Ok(Some(img_data)),
                Err(_) => Ok(None),  // No image in clipboard
            }
        } else {
            Ok(None)
        }
    }

    // NEW: Convert ImageData to base64 PNG
    pub fn image_to_base64(img: &ImageData) -> Result<(String, String)> {
        // Convert RGBA to PNG
        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);

        encoder.write_image(
            &img.bytes,
            img.width as u32,
            img.height as u32,
            ColorType::Rgba8,
        ).map_err(|e| anyhow::anyhow!("Failed to encode image: {}", e))?;

        let base64_data = base64::engine::general_purpose::STANDARD.encode(&png_data);

        Ok(("image/png".to_string(), base64_data))
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
```

### 4. User Interaction Design

#### Option A: Keyboard Shortcut (Recommended)
- `Cmd/Ctrl + Shift + V` - Paste image from clipboard
- Shows visual indicator: `[📷 Image attached: 1024x768]`
- Can attach multiple images before sending

#### Option B: Command
- `/attach-image` - Attach clipboard image to next message
- `/attach-image path/to/file.png` - Attach file (future enhancement)

#### Option C: Auto-detect (Power user mode)
- If clipboard has image when submitting, prompt: "Attach image? [y/n]"

**Recommendation:** Start with Option A (keyboard shortcut) as it's most intuitive and similar to Claude Code.

### 5. UI/UX Implementation

#### AppState Changes
```rust
// src/tui/app_state.rs

#[derive(Clone, Debug)]
pub struct AttachedImage {
    pub width: usize,
    pub height: usize,
    pub media_type: String,
    pub size_kb: usize,
    pub data: String,  // base64
}

impl AttachedImage {
    pub fn from_clipboard_image(img: ImageData<'static>) -> Result<Self> {
        let (media_type, data) = ClipboardManager::image_to_base64(&img)?;
        let size_kb = data.len() / 1024;

        Ok(Self {
            width: img.width,
            height: img.height,
            media_type,
            size_kb,
            data,
        })
    }

    pub fn display_info(&self) -> String {
        format!("📷 Image ({}x{}, {} KB)", self.width, self.height, self.size_kb)
    }
}

pub struct AppState {
    pub input: TextArea<'static>,
    pub messages: VecDeque<MessageLine>,
    pub pending_messages: VecDeque<MessageLine>,
    pub agent_state: AgentState,
    pub should_quit: bool,
    pub should_cancel_task: bool,
    pub max_messages: usize,
    pub completion_state: Option<CompletionState>,
    pub completers: Vec<Box<dyn Completer>>,
    pub tool_permission_dialog_state: Option<ToolPermissionDialogState>,
    pub approval_dialog_state: Option<ApprovalDialogState>,
    pub autopilot_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub animation_frame: usize,
    pub prompt_history: PromptHistory,
    pub current_thinking_spinner: usize,
    pub current_executing_spinner: usize,
    pub clipboard: ClipboardManager,
    pub current_retry_status: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_cost: f64,
    pub active_tool_calls: Vec<ActiveToolCall>,
    pub todos: Vec<TodoItem>,
    // NEW: Attached images for next message
    pub attached_images: Vec<AttachedImage>,
}

impl AppState {
    pub fn attach_image(&mut self, image: AttachedImage) {
        self.attached_images.push(image);
    }

    pub fn clear_attached_images(&mut self) {
        self.attached_images.clear();
    }

    pub fn has_attached_images(&self) -> bool {
        !self.attached_images.is_empty()
    }
}
```

#### Image Paste Handler
```rust
// src/tui/handlers/image_paste_handler.rs

use crate::tui::app_state::{AppState, AttachedImage};
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyModifiers};

pub struct ImagePasteHandler;

impl ImagePasteHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ImagePasteHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for ImagePasteHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Key(key) = event else {
            return KeyHandlerResult::NotHandled;
        };

        // Check for Cmd/Ctrl + Shift + V
        if key.code == KeyCode::Char('v')
            && key.modifiers.contains(KeyModifiers::SHIFT)
            && (key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::SUPER))
        {
            // Try to get image from clipboard
            match app.clipboard.get_image() {
                Ok(Some(img_data)) => {
                    match AttachedImage::from_clipboard_image(img_data) {
                        Ok(image) => {
                            // Validate image size (warn if > 5MB)
                            if image.size_kb > 5 * 1024 {
                                app.add_system_message(format!(
                                    "⚠️  Warning: Image is large ({} KB). Some backends may reject it.",
                                    image.size_kb
                                ));
                            }

                            app.add_system_message(format!(
                                "✅ Image attached: {}x{} ({} KB)",
                                image.width, image.height, image.size_kb
                            ));
                            app.attach_image(image);
                            KeyHandlerResult::Handled
                        }
                        Err(e) => {
                            app.add_system_message(format!(
                                "❌ Failed to process image: {}",
                                e
                            ));
                            KeyHandlerResult::Handled
                        }
                    }
                }
                Ok(None) => {
                    app.add_system_message("ℹ️  No image in clipboard".to_string());
                    KeyHandlerResult::Handled
                }
                Err(e) => {
                    app.add_system_message(format!(
                        "❌ Failed to access clipboard: {}",
                        e
                    ));
                    KeyHandlerResult::Handled
                }
            }
        } else {
            KeyHandlerResult::NotHandled
        }
    }
}
```

#### Update Submit Handler
```rust
// src/tui/handlers/submit_handler.rs

// In handle_event, when submitting:
if key.code == KeyCode::Enter && !input_text.trim().is_empty() && !agent_task_active {
    // Build message content with text and images
    let content = if app.attached_images.is_empty() {
        MessageContent::Text(input_text.clone())
    } else {
        let mut blocks = vec![ContentBlock::Text { text: input_text.clone() }];

        for img in &app.attached_images {
            blocks.push(ContentBlock::Image {
                source: ImageSource {
                    media_type: img.media_type.clone(),
                    data: img.data.clone(),
                },
                detail: Some("auto".to_string()),
            });
        }

        MessageContent::Blocks(blocks)
    };

    // Add to conversation
    app.add_user_message_with_content(content);

    // Clear attached images
    app.clear_attached_images();

    app.prompt_history.add(input_text.clone());
    app.clear_input();

    if input_text.trim().starts_with('/') {
        KeyHandlerResult::StartCommand(input_text)
    } else {
        KeyHandlerResult::StartConversation(input_text)
    }
} else {
    KeyHandlerResult::Handled
}
```

#### Visual Indicators in Layout
```rust
// In layout rendering, show attached images above input box:

if !app.attached_images.is_empty() {
    let attachments: Vec<Line> = app.attached_images
        .iter()
        .map(|img| {
            Line::from(vec![
                Span::styled("📷 ", Style::default().fg(Color::Blue)),
                Span::raw(format!("{}x{} ", img.width, img.height)),
                Span::styled(format!("{} KB", img.size_kb), Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    // Render above input area
}
```

### 6. Dependencies to Add

```toml
# Cargo.toml
[dependencies]
image = "0.25"       # For PNG encoding from RGBA
base64 = "0.22"      # For base64 encoding

# Note: arboard = "3.4" already exists
```

---

## Implementation Plan

### Phase 1: Core Data Structures (Breaking Change - Acceptable)
**Goal:** Replace text-only content with content blocks

**Files to modify:**
- `src/agent/conversation.rs`
  - Add `ContentBlock` enum
  - Add `ImageSource` struct
  - Update `ConversationMessage.content` from `Option<String>` to `Vec<ContentBlock>`
  - Update all methods that create messages:
    - `add_user_message()` → create single text block
    - `add_assistant_message()` → handle text content
    - `add_system_message()` → create single text block
  - Add new method: `add_user_message_with_blocks(blocks: Vec<ContentBlock>)`
  - Update all code that reads `content` field

**Migration strategy:**
- Direct replacement - no backward compatibility needed
- All conversation storage will use new format
- Users will need to start fresh conversations (acceptable per README)

**Testing:**
- Test text-only message creation
- Test serialization/deserialization
- Test conversation save/load

### Phase 2: Anthropic Backend (Easiest)
**Goal:** Get vision working with one backend first

**Files to modify:**
- `src/backends/anthropic.rs`
  - Extend `ContentBlock` enum to include `Image` variant
  - Update `convert_messages()` to handle image blocks
  - Add `ImageSourceBlock` struct

**Testing:**
- Manual test with Claude 4 sending a screenshot
- Verify API request format matches Anthropic docs

### Phase 3: OpenAI/Together AI Backends
**Goal:** Support OpenAI-compatible backends

**Files to modify:**
- `src/backends/openai_compatible.rs`
  - Add `OpenAIContent` enum
  - Add `OpenAIContentPart` enum
  - Add `OpenAIImageUrl` struct
  - Update message conversion

- `src/backends/together_ai.rs`
  - Similar changes as OpenAI (they share the same format)

**Testing:**
- Test with GPT-4o
- Test with Llama Vision on Together AI

### Phase 4: Ollama Backend
**Goal:** Support local vision models

**Files to modify:**
- `src/backends/ollama.rs`
  - Add `images` parameter to `ChatRequest`
  - Implement `convert_messages_and_extract_images()`
  - Update request building

**Testing:**
- Test with LLaVA models locally
- Verify image extraction and separate parameter passing

### Phase 5: Clipboard & UI Integration
**Goal:** Allow users to attach screenshots

**Files to modify:**
- `src/tui/clipboard.rs`
  - Add `get_image()` method
  - Add `image_to_base64()` helper

- `src/tui/app_state.rs`
  - Add `AttachedImage` struct
  - Add `attached_images` field
  - Add attachment methods

- `src/tui/handlers/image_paste_handler.rs` (NEW)
  - Implement keyboard shortcut handler

- `src/tui/handlers/submit_handler.rs`
  - Update to include attached images in messages
  - Clear attachments after submit

- `src/tui/handlers/mod.rs`
  - Register `ImagePasteHandler`

**Testing:**
- Test keyboard shortcut detection
- Test clipboard image reading on macOS/Linux/Windows
- Test image attachment and clearing

### Phase 6: Visual Feedback
**Goal:** Show users what images are attached

**Files to modify:**
- `src/tui/layout.rs` or `src/tui/layout_builder.rs`
  - Add attachment indicator area
  - Show list of attached images with dimensions/size

- `src/tui/message_renderer.rs`
  - Optionally show image placeholders in message history

**Testing:**
- Verify visual indicators appear correctly
- Test with multiple images

### Phase 7: Error Handling & Polish
**Goal:** Handle edge cases gracefully

**Enhancements:**
- Validate image sizes (warn if >5MB)
- Handle non-vision models gracefully (strip images or error)
- Add `/attach-image` command as alternative
- Compress large images automatically
- Add configuration option to disable vision

**Testing:**
- Test with very large images
- Test with non-vision models
- Test error scenarios (clipboard unavailable, encoding failure)

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_only_message() {
        let msg = ConversationMessage::text("user".to_string(), "Hello!".to_string());
        assert_eq!(msg.content.len(), 1);
        assert_eq!(msg.get_text(), Some("Hello!"));
        assert!(!msg.has_images());
    }

    #[test]
    fn test_message_with_image() {
        let blocks = vec![
            ContentBlock::Text { text: "Look at this:".to_string() },
            ContentBlock::Image {
                source: ImageSource {
                    media_type: "image/png".to_string(),
                    data: "base64data".to_string(),
                },
                detail: None,
            },
        ];
        let msg = ConversationMessage::with_content("user".to_string(), blocks);
        assert!(msg.has_images());
        assert_eq!(msg.image_count(), 1);
        assert_eq!(msg.get_text(), Some("Look at this:"));
    }

    #[test]
    fn test_image_to_base64_conversion() {
        // Create a simple 2x2 red image
        let bytes = vec![
            255, 0, 0, 255,  // Red pixel
            255, 0, 0, 255,  // Red pixel
            255, 0, 0, 255,  // Red pixel
            255, 0, 0, 255,  // Red pixel
        ];

        let img = ImageData {
            width: 2,
            height: 2,
            bytes: std::borrow::Cow::from(bytes),
        };

        let (media_type, base64) = ClipboardManager::image_to_base64(&img).unwrap();
        assert_eq!(media_type, "image/png");
        assert!(!base64.is_empty());
    }
}
```

### Integration Tests
1. Test full flow: clipboard → attach → send → backend conversion
2. Test with each backend (Anthropic, OpenAI, Together, Ollama)
3. Test conversation save/load with images
4. Test multiple images in one message

### Manual Testing Checklist
- [ ] Copy screenshot to clipboard (Cmd+Shift+4 on macOS)
- [ ] Paste with Cmd+Shift+V
- [ ] Verify visual indicator appears
- [ ] Send message with image
- [ ] Verify API request contains base64 image
- [ ] Verify LLM responds to image content
- [ ] Test with multiple images
- [ ] Test with large images (>5MB)
- [ ] Test on different platforms (macOS, Linux, Windows)

---

## Configuration Options

```toml
# ~/.config/hoosh/config.toml

[vision]
# Enable/disable vision support globally
enabled = true

# Maximum image size in KB (default: 5120 = 5MB)
max_image_size_kb = 5120

# Automatically compress large images
auto_compress = true

# Target width for compression (maintains aspect ratio)
compress_max_width = 1920

# Image quality for JPEG compression (1-100)
jpeg_quality = 85

# Warn when attaching large images
warn_large_images = true
```

---

## Future Enhancements

### Nice to Have
1. **Terminal Image Preview**
   - Use sixel protocol for terminals that support it
   - Use kitty graphics protocol
   - Fallback to ASCII art

2. **Image File Support**
   - `/attach-image path/to/file.png`
   - Drag-and-drop files (if terminal supports)

3. **Automatic Compression**
   - Resize images >1920px width
   - Convert to JPEG with quality slider
   - Strip EXIF data for privacy

4. **Multiple Image Management**
   - `/list-images` - Show all attached
   - `/remove-image 2` - Remove by index
   - `/clear-images` - Remove all

5. **Image History**
   - Show thumbnails in message history (if terminal supports)
   - Cache displayed images
   - Click to view full size

6. **OCR Integration**
   - Automatically extract text from screenshots
   - Add as additional context

---

## Known Limitations

1. **Storage Size:** Images stored as base64 in conversation files will increase storage significantly
2. **Terminal Support:** Not all terminals can display images natively
3. **Model Support:** Need to detect if selected model supports vision
4. **Context Limits:** Images consume significant token budget
5. **Ollama:** Image support varies by model (LLaVA works, but not all models)

---

## Sources & References

- [Vision - Anthropic - Claude API](https://docs.claude.com/en/docs/build-with-claude/vision)
- [Vision - Claude Docs](https://platform.claude.com/docs/en/build-with-claude/vision)
- [Vision - Together.ai Docs](https://docs.together.ai/docs/vision-overview)
- [OpenAI's Vision API Guide](https://platform.openai.com/docs/guides/vision)
- [Images and vision - OpenAI API](https://platform.openai.com/docs/guides/images-vision)
- [llava - Ollama](https://ollama.com/library/llava)
- [Vision models - Ollama Blog](https://ollama.com/blog/vision-models)
- [Clipboard in arboard - Rust](https://docs.rs/arboard/latest/arboard/struct.Clipboard.html)
- [GitHub - 1Password/arboard](https://github.com/1Password/arboard)

---

## Questions for Discussion

1. **Image Compression:** Should we automatically compress images >2MB or let users handle it?
2. **Model Detection:** Should we warn users when they select a non-vision model with images attached?
3. **Storage:** Should we store full base64 in conversation files or use external file references?
4. **UI Preference:** Keyboard shortcut vs command vs auto-detect?
5. **Default Detail Level:** Should OpenAI `detail` default to "auto", "low", or "high"?
