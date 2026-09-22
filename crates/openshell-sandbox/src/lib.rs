// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Capability-free in-workload sandbox boundary.

pub mod activation;
pub mod health;

use miette::Result;
use std::sync::Arc;
#[cfg(target_os = "linux")]
mod accept_interrupt;
pub mod boundary_exec;
pub mod boundary_io;
mod boundary_server;
pub mod child_env;
#[cfg(target_os = "linux")]
pub(crate) mod delegated;
#[cfg(unix)]
pub mod identity;
#[cfg(target_os = "linux")]
pub mod main_session;
pub mod managed_children;
#[cfg(target_os = "linux")]
mod network_broker;
#[cfg(all(target_os = "linux", feature = "perf-harness"))]
pub mod perf;
#[cfg(unix)]
pub mod process;
mod pty;
pub mod sandbox;

/// Results of actively qualifying the admitted workload runtime before the
/// sandbox consumes protected bootstrap material.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "qualification preserves independently exercised security results"
)]
pub struct RuntimeQualification {
    pub seccomp: openshell_sandbox_backend::boundary_protocol::SeccompEvidence,
    pub landlock_abi: u32,
    pub landlock_allow_deny: bool,
    pub udp_dns_round_trip: bool,
    pub tcp_dns_round_trip: bool,
    pub tcp_allow_round_trip: bool,
    pub tcp_deny_round_trip: bool,
}

/// Placeholder used when compiling the package on a non-Linux host.
///
/// The sandbox binary rejects execution on those hosts before constructing a
/// qualification, but retaining the type keeps the library API portable for
/// workspace-wide checks.
#[cfg(not(target_os = "linux"))]
#[derive(Clone, Copy, Debug)]
pub struct RuntimeQualification;

/// Run the authenticated boundary-local sandbox.
///
/// # Errors
///
/// Returns an error when the protected bootstrap is invalid or the boundary
/// listener cannot be established.
pub fn run(
    config_path: &std::path::Path,
    qualification: RuntimeQualification,
) -> miette::Result<()> {
    boundary_server::run_boundary(config_path, qualification)
        .map_err(|error| miette::miette!(error))
}

pub async fn run_unidentified(
    health_check: bool,
    health_port: u16,
    ocsf_enabled: Arc<std::sync::atomic::AtomicBool>,
) -> Result<i32> {
    use activation::SupervisorService;
    use health::HealthServer;
    use openshell_core::proto::supervisor::v1::supervisor_server::SupervisorServer;

    let hostname = std::fs::read_to_string("/etc/hostname").map_or_else(
        |_| "openshell-sandbox".to_string(),
        |h| h.trim().to_string(),
    );
    openshell_ocsf::ctx::set_ctx(openshell_ocsf::SandboxContext {
        sandbox_id: String::new(),
        sandbox_name: String::new(),
        container_image: std::env::var("OPENSHELL_CONTAINER_IMAGE").unwrap_or_default(),
        hostname,
        product_version: openshell_core::VERSION.to_string(),
        proxy_ip: std::net::IpAddr::from([127, 0, 0, 1]),
        proxy_port: 0,
    });

    let _ = ocsf_enabled;

    let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));

    if health_check {
        let health_server = HealthServer::new(ready.clone());
        tokio::spawn(async move {
            if let Err(e) = health_server.serve(health_port).await {
                tracing::warn!(error = %e, "Health server failed");
            }
        });
    }

    let (activation_tx, activation_rx) = tokio::sync::oneshot::channel();
    let service = SupervisorService::new(activation_tx);

    let grpc_port: u16 = std::env::var("OPENSHELL_ACTIVATION_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9090);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], grpc_port));
    tracing::info!(port = grpc_port, "Starting gRPC activation server in unidentified mode");

    let server = tonic::transport::Server::builder()
        .add_service(SupervisorServer::new(service))
        .serve(addr);

    if health_check {
        ready.store(true, std::sync::atomic::Ordering::Release);
        tracing::info!(port = health_port, "Health endpoint /readyz is ready");
    }

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                tracing::warn!(error = %e, "gRPC server error");
            }
            Ok(0)
        }
        Ok(result) = activation_rx => {
            tracing::info!(
                sandbox_id = %result.sandbox_id,
                sandbox_name = %result.sandbox_name,
                "Activation received, starting run_sandbox via normal path"
            );
            Err(miette::miette!("activation_received:{}", result.sandbox_id))
        }
    }
}
