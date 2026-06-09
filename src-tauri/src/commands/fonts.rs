use reader_core::CommandError;

type CommandResult<T> = Result<T, CommandError>;

fn unsupported(feature: &str) -> CommandError {
    CommandError {
        code: "UNSUPPORTED".to_string(),
        message: format!("{feature} 功能尚未实现"),
        detail: None,
        retryable: false,
    }
}

#[tauri::command] pub async fn list_system_fonts() -> CommandResult<()> { Err(unsupported("系统字体列表")) }
#[tauri::command] pub async fn list_user_fonts() -> CommandResult<()> { Err(unsupported("用户字体列表")) }
#[tauri::command] pub async fn upload_user_font() -> CommandResult<()> { Err(unsupported("上传字体")) }
#[tauri::command] pub async fn delete_user_font() -> CommandResult<()> { Err(unsupported("删除字体")) }
#[tauri::command] pub async fn rename_user_font() -> CommandResult<()> { Err(unsupported("重命名字体")) }
