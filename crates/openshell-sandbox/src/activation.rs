// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicBool, Ordering};

use openshell_core::proto::supervisor::v1::supervisor_server::Supervisor;
use openshell_core::proto::supervisor::v1::{
    ActivateSandboxRequest, ActivateSandboxResponse, ErrorCode,
};
use tokio::sync::oneshot;
use tonic::{Request, Response, Status};
use tracing::info;

pub struct SupervisorService {
    activated: AtomicBool,
    activation_tx: std::sync::Mutex<Option<oneshot::Sender<ActivationResult>>>,
}

pub struct ActivationResult {
    pub sandbox_id: String,
    pub sandbox_name: String,
    pub gateway_endpoint: String,
}

impl SupervisorService {
    pub fn new(activation_tx: oneshot::Sender<ActivationResult>) -> Self {
        Self {
            activated: AtomicBool::new(false),
            activation_tx: std::sync::Mutex::new(Some(activation_tx)),
        }
    }
}

#[tonic::async_trait]
impl Supervisor for SupervisorService {
    async fn activate_sandbox(
        &self,
        request: Request<ActivateSandboxRequest>,
    ) -> Result<Response<ActivateSandboxResponse>, Status> {
        let req = request.into_inner();
        info!(sandbox_id = %req.sandbox_id, sandbox_name = %req.sandbox_name, "ActivateSandbox request received");

        if self.activated.swap(true, Ordering::AcqRel) {
            return Ok(Response::new(ActivateSandboxResponse {
                success: false,
                error_message: "supervisor already activated".into(),
                error_code: ErrorCode::AlreadyActivated.into(),
            }));
        }

        if req.sandbox_id.is_empty() || req.gateway_endpoint.is_empty() {
            self.activated.store(false, Ordering::Release);
            return Ok(Response::new(ActivateSandboxResponse {
                success: false,
                error_message: "sandbox_id and gateway_endpoint are required".into(),
                error_code: ErrorCode::InvalidRequest.into(),
            }));
        }

        // SAFETY: called before run_sandbox spawns threads; single-threaded at this point.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(openshell_core::sandbox_env::SANDBOX_ID, &req.sandbox_id);
            std::env::set_var(openshell_core::sandbox_env::SANDBOX, &req.sandbox_name);
            std::env::set_var(openshell_core::sandbox_env::ENDPOINT, &req.gateway_endpoint);
        }

        info!(
            sandbox_id = %req.sandbox_id,
            sandbox_name = %req.sandbox_name,
            gateway = %req.gateway_endpoint,
            "Identity set via env vars, signalling run_sandbox"
        );

        let sender = self.activation_tx.lock().unwrap().take();
        if let Some(tx) = sender {
            let _ = tx.send(ActivationResult {
                sandbox_id: req.sandbox_id,
                sandbox_name: req.sandbox_name,
                gateway_endpoint: req.gateway_endpoint,
            });
        }

        Ok(Response::new(ActivateSandboxResponse {
            success: true,
            error_message: String::new(),
            error_code: ErrorCode::Unspecified.into(),
        }))
    }
}
