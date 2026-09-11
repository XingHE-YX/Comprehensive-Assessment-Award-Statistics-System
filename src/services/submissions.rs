use crate::{
    db::{AttachmentRepo, SubmissionRepo},
    domain::{AcademicYear, Submission},
    error::AppError,
    state::AppState,
    storage::UploadInput,
    validation::{ValidatedSubmission, validate_uploads_with_existing},
};

pub async fn reset_edit_code(
    state: &AppState,
    vault: &crate::auth::EditCodeVault,
    submission: &Submission,
    expected_version: i64,
) -> Result<(), AppError> {
    if expected_version < 0 || expected_version != submission.edit_code_version {
        return Err(AppError::Conflict);
    }
    let previous_hash = submission.edit_code_hash.clone();
    let (code, hash) = tokio::task::spawn_blocking(move || {
        // Even a coincidentally repeated random value must not keep the old code valid.
        let code = loop {
            let candidate = crate::auth::generate_edit_code();
            if !crate::auth::verify_secret(&previous_hash, &candidate) {
                break candidate;
            }
        };
        crate::auth::hash_secret(&code).map(|hash| (code, hash))
    })
    .await
    .map_err(|_| crate::auth::AuthError::Verification)?
    .map_err(|_| crate::auth::AuthError::Verification)?;
    let ciphertext = vault.encrypt(&submission.submission_no, &code);
    if !SubmissionRepo::reset_edit_code(
        &state.db,
        submission.id,
        expected_version,
        &hash,
        &ciphertext,
    )
    .await?
    {
        return Err(AppError::Conflict);
    }
    tracing::info!(submission_id = submission.id, event = "edit_code_reset");
    Ok(())
}

/// Authenticate against both the envelope and the current verifier. A valid but
/// stale envelope from an earlier credential must not present a misleading code.
pub fn recover_edit_code(
    vault: &crate::auth::EditCodeVault,
    submission: &Submission,
) -> Option<String> {
    let code = vault.decrypt(
        &submission.submission_no,
        submission.edit_code_ciphertext.as_deref()?,
    )?;
    crate::auth::verify_secret(&submission.edit_code_hash, &code).then_some(code)
}

pub async fn update_student(
    state: &AppState,
    submission: &Submission,
    year: &AcademicYear,
    input: &ValidatedSubmission,
    uploads: Vec<UploadInput>,
) -> Result<(), AppError> {
    let mut transaction = state.db.begin_with("BEGIN IMMEDIATE").await?;
    crate::services::roster::validate_identity(
        &mut transaction,
        year.id,
        &input.student_name,
        &input.student_no,
    )
    .await?;
    // The conditional write takes SQLite's write lock before counting or saving attachments.
    if !SubmissionRepo::update_student(
        &mut transaction,
        submission.id,
        submission.edit_code_version,
        input,
    )
    .await?
    {
        return Err(AppError::Forbidden);
    }
    let existing = AttachmentRepo::count_in_transaction(&mut transaction, submission.id).await?;
    validate_uploads_with_existing(&uploads, existing as usize).map_err(AppError::Validation)?;
    let year_directory = crate::storage::academic_year_directory(year.id);
    let stored = state
        .storage
        .save_many(
            &year_directory,
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
                .remove_for_submission(
                    &year_directory,
                    &submission.submission_no,
                    &item.stored_name,
                )
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
