// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{debug, info};

pub struct HealthServer {
    ready: Arc<AtomicBool>,
}

impl HealthServer {
    pub fn new(ready: Arc<AtomicBool>) -> Self {
        Self { ready }
    }

    pub async fn serve(self, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr).await?;
        info!(port, "Health check endpoint listening");

        let ready = self.ready;
        loop {
            let (mut stream, _) = listener.accept().await?;
            let is_ready = ready.load(Ordering::Acquire);
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
                let (status, body) = if is_ready {
                    ("200 OK", "ok")
                } else {
                    ("503 Service Unavailable", "not ready")
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
                debug!("Health check responded: {status}");
            });
        }
    }
}
