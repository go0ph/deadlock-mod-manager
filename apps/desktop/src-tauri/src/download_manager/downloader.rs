use crate::errors::Error;
use futures::StreamExt;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

#[allow(dead_code)]
const BUFFER_SIZE: usize = 64 * 1024; // 64KB buffer for future use
const PROGRESS_THROTTLE_MS: u128 = 500; // Emit progress every 500ms

/// A shared reqwest client reused across all downloads.  Connection pooling and keep-alive
/// are enabled by default in reqwest, so reusing the client avoids the per-download TLS
/// handshake and TCP connection overhead.
static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
  SHARED_CLIENT.get_or_init(|| {
    reqwest::Client::builder()
      .pool_max_idle_per_host(8)
      .tcp_keepalive(std::time::Duration::from_secs(30))
      .build()
      .expect("Failed to build shared reqwest client")
  })
}

#[derive(Clone, Debug)]
pub struct DownloadProgress {
  pub downloaded: u64,
  pub speed: f64,
}

pub async fn download_file<F>(
  url: &str,
  target_path: &Path,
  on_progress: F,
  cancel_token: CancellationToken,
) -> Result<(), Error>
where
  F: Fn(DownloadProgress) + Send + 'static,
{
  let host = url.split('/').nth(2).unwrap_or("unknown").to_string();

  log::info!("Starting download from {url} (host: {host}) to {target_path:?}");

  let client = get_client();
  let request_start = Instant::now();

  let response = client
    .get(url)
    .send()
    .await
    .map_err(|e| Error::Network(format!("Failed to send request: {e}")))?;

  let ttfb = request_start.elapsed();
  log::debug!("Time to first byte from {host}: {:.0}ms", ttfb.as_millis());

  if !response.status().is_success() {
    return Err(Error::Network(format!(
      "Server returned error status: {}",
      response.status()
    )));
  }

  let total_size = response.content_length().unwrap_or(0);
  log::info!("Download size from {host}: {total_size} bytes");

  if let Some(parent) = target_path.parent() {
    tokio::fs::create_dir_all(parent).await.map_err(Error::Io)?;
  }

  let mut file = File::create(target_path)
    .await
    .map_err(|e| Error::FileWriteFailed(format!("Failed to create file: {e}")))?;

  let mut stream = response.bytes_stream();
  let mut downloaded: u64 = 0;
  let start_time = Instant::now();
  let mut last_progress_time = Instant::now();

  while let Some(chunk) = stream.next().await {
    if cancel_token.is_cancelled() {
      log::info!("Download cancelled");
      return Err(Error::DownloadCancelled);
    }

    let chunk = chunk.map_err(|e| Error::Network(format!("Failed to read chunk: {e}")))?;

    file
      .write_all(&chunk)
      .await
      .map_err(|e| Error::FileWriteFailed(format!("Failed to write to file: {e}")))?;

    downloaded += chunk.len() as u64;

    let now = Instant::now();
    let elapsed_since_last = now.duration_since(last_progress_time).as_millis();

    let is_complete = total_size > 0 && downloaded >= total_size;

    if is_complete || elapsed_since_last >= PROGRESS_THROTTLE_MS {
      let elapsed_total = start_time.elapsed().as_secs_f64();
      let speed = if elapsed_total > 0.0 {
        downloaded as f64 / elapsed_total
      } else {
        0.0
      };

      on_progress(DownloadProgress { downloaded, speed });

      last_progress_time = now;
    }
  }

  file
    .flush()
    .await
    .map_err(|e| Error::FileWriteFailed(format!("Failed to flush file: {e}")))?;

  let elapsed = start_time.elapsed();
  let throughput_kbps = if elapsed.as_secs_f64() > 0.0 {
    (downloaded as f64 / 1024.0) / elapsed.as_secs_f64()
  } else {
    0.0
  };
  log::info!(
    "Download completed: {target_path:?} — host={host} bytes={downloaded} \
     ttfb={ttfb_ms}ms time={elapsed_ms}ms throughput={throughput_kbps:.1}KB/s",
    ttfb_ms = ttfb.as_millis(),
    elapsed_ms = elapsed.as_millis(),
  );

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

  #[tokio::test]
  async fn test_download_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let cancel_token = CancellationToken::new();

    let result = download_file(
      "https://httpbin.org/bytes/1024",
      &file_path,
      |progress| {
        println!("Downloaded: {} bytes", progress.downloaded);
      },
      cancel_token,
    )
    .await;

    assert!(result.is_ok());
    assert!(file_path.exists());
  }
}
