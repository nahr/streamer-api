//! Upgrade endpoints: apt update and apt install table-tv.
//! Streams command output to the client.

use axum::{
    body::Body,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::post,
};
use bytes::Bytes;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::api::AppState;
use crate::error::ApiError;

const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");

/// POST /api/upgrade/check - Runs `apt update`. Logs output server-side, returns empty body.
pub async fn check_for_upgrades(
    State(_app): State<AppState>,
) -> Result<Response, ApiError> {
    tracing::info!("check for upgrades: apt update");

    let output = Command::new("apt")
        .arg("update")
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .await
        .map_err(ApiError::from)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        for line in stdout.lines() {
            tracing::info!(phase = "check", "{}", line);
        }
    }
    if !stderr.is_empty() {
        for line in stderr.lines() {
            tracing::warn!(phase = "check", "{}", line);
        }
    }

    match output.status.success() {
        true => tracing::info!(phase = "check", "apt update completed successfully"),
        false => tracing::warn!(phase = "check", code = ?output.status.code(), "apt update exited with error"),
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        Body::from(""),
    )
        .into_response())
}

/// POST /api/upgrade/install - Runs `apt install -y table-tv`. Streams output.
pub async fn upgrade_now(
    State(_app): State<AppState>,
) -> Result<Response, ApiError> {
    run_apt_stream("upgrade", &["install", "-y", PACKAGE_NAME]).await
}

async fn run_apt_stream(label: &str, args: &[&str]) -> Result<Response, ApiError> {
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);

    let label = label.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    tokio::spawn(async move {
        let mut child = match Command::new("apt")
            .args(&args)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(phase = %label, error = %e, "apt spawn failed");
                let _ = tx.send(format!("Error spawning apt: {}\n", e)).await;
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return,
        };
        let stderr = match child.stderr.take() {
            Some(s) => s,
            None => return,
        };

        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stdout.send(line + "\n").await;
            }
        });

        let tx_stderr = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stderr.send(line + "\n").await;
            }
        });

        let status = child.wait().await;
        match status {
            Ok(s) if s.success() => tracing::info!(phase = %label, "apt completed successfully"),
            Ok(s) => tracing::warn!(phase = %label, code = ?s.code(), "apt exited with error"),
            Err(e) => tracing::error!(phase = %label, error = %e, "apt wait failed"),
        }
        drop(tx);
    });

    let stream = ReceiverStream::new(rx).map(|line| Ok::<_, std::io::Error>(Bytes::from(line)));
    let body = Body::from_stream(stream);

    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        body,
    )
        .into_response())
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/upgrade/check", post(check_for_upgrades))
        .route("/api/upgrade/install", post(upgrade_now))
}
