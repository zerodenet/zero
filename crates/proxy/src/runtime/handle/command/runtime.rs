pub(super) fn with_current_runtime<T>(
    missing_message: &'static str,
    run: impl FnOnce(&tokio::runtime::Handle) -> zero_api::ApiResult<T>,
) -> zero_api::ApiResult<T> {
    match tokio::runtime::Handle::try_current() {
        Ok(rt) => run(&rt),
        Err(_) => Err(zero_api::ApiError::new(
            zero_api::ApiErrorCode::Internal,
            missing_message,
        )),
    }
}
