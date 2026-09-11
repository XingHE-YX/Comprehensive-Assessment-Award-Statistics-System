use crate::{db::YearDeletionRepo, state::AppState};

/// Durable cleanup follows the metadata transaction and is safe to retry.
pub async fn cleanup(state: &AppState) -> Result<(), crate::error::AppError> {
    for name in YearDeletionRepo::pending_files(&state.db).await? {
        match state.storage.remove_by_name(&name).await {
            Ok(()) => YearDeletionRepo::finish_file(&state.db, &name).await?,
            Err(_) => tracing::warn!(event = "year_attachment_cleanup_pending"),
        }
    }
    Ok(())
}
