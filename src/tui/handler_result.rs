pub enum KeyHandlerResult {
    NotHandled,
    Handled,
    ShouldQuit,
    ShouldCancelTask,
    StartCommand(String),
    StartConversation {
        input: String,
        image_attachments: Vec<crate::agent::Attachment>,
    },
}
