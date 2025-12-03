use viceroy::updater::{check_for_updates, UPDATE_METADATA_URL_ENV};

#[ignore = "Requires mock update server exposing metadata and binary endpoints"]
#[tokio::test]
async fn updater_can_follow_mock_server_flow() {
    // The mock server should respond with a valid metadata document and bundle for this to pass.
    std::env::set_var(UPDATE_METADATA_URL_ENV, "http://127.0.0.1:8999/latest.json");
    let result = check_for_updates(true).await;
    assert!(result.is_ok());
}
