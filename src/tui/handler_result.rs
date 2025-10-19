pub enum KeyHandlerResult {
    Handled,
    ShouldQuit,
    ShouldCancelTask,
    StartCommand(String),
    StartConversation(String),
}
