use crate::{
    db::{AttachmentRepo, SubmissionRepo},
    domain::{AcademicYear, Submission},
    error::AppError,
    state::AppState,
    storage::UploadInput,
    validation::{ValidatedSubmission, validate_uploads_with_existing},
};

pub async fn update_student(
    state: &AppState,
    submission: &Submission,
    year: &AcademicYear,
    input: &ValidatedSubmission,
    uploads: Vec<UploadInput>,
) -> Result<(), AppError> {
    let mut transaction = state.db.begin().await?;
    // The conditional write takes SQLite's write lock before counting or saving attachments.
    if !SubmissionRepo::update_student(&mut transaction, submission.id, input).await? {
        return Err(AppError::Forbidden);
    }
    let existing = AttachmentRepo::count_in_transaction(&mut transaction, submission.id).await?;
    validate_uploads_with_existing(&uploads, existing as usize).map_err(AppError::Validation)?;
    let stored = state
        .storage
        .save_many(
            &year.name,
            &submission.submission_no,
            existing as usize,
            uploads,
        )
        .await?;
    let result = async {
        for item in &stored {
            AttachmentRepo::insert_stored(&mut transaction, submission.id, item).await?;
        }
        transaction.commit().await
    }
    .await;
    if let Err(error) = result {
        for item in &stored {
            if state
                .storage
                .remove_for_submission(&year.name, &submission.submission_no, &item.stored_name)
                .await
                .is_err()
            {
                tracing::error!(
                    submission_id = submission.id,
                    "failed to clean up student update attachment"
                );
            }
        }
        return Err(error.into());
    }
    Ok(())
}
