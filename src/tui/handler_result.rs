pub enum KeyHandlerResult {
    NotHandled,
    Handled,
    ShouldQuit,
    ShouldCancelTask,
    StartCommand(String),
    StartConversation(String),
}
