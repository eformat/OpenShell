// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::ProviderDiscoverySpec;

pub const SPEC: ProviderDiscoverySpec = ProviderDiscoverySpec {
    id: "anthropic",
    credential_env_vars: &["ANTHROPIC_API_KEY"],
};

/// Anthropic-compatible API with bearer-token auth (e.g. MaaS endpoints).
/// Uses `ANTHROPIC_AUTH_TOKEN` as the credential key (→ `Authorization: Bearer`).
pub const OPENAI_SPEC: ProviderDiscoverySpec = ProviderDiscoverySpec {
    id: "anthropic-openai",
    credential_env_vars: &["ANTHROPIC_AUTH_TOKEN"],
};

test_discovers_env_credential!(
    discovers_anthropic_env_credentials,
    "ANTHROPIC_API_KEY",
    "sk-ant-test"
);
