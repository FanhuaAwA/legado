use reader_core::CommandError;

type CommandResult<T> = Result<T, CommandError>;

fn unsupported() -> CommandError {
    CommandError {
        code: "UNSUPPORTED".into(),
        message: "书源在线更新功能尚未实现".into(),
        detail: None,
        retryable: false,
    }
}

#[tauri::command]
pub async fn booksource_apply_update() -> CommandResult<()> {
    Err(unsupported())
}
#[tauri::command]
pub async fn booksource_check_update() -> CommandResult<()> {
    Err(unsupported())
}
