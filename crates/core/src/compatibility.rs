use std::collections::BTreeSet;

use viden_types::{
    CapabilityId, CoreHandshake, FRONTEND_SCHEMA_V1, FRONTEND_V1_CAPABILITIES, SchemaVersion,
};

pub const CORE_CLIENT_CAPABILITIES: &[&str] = FRONTEND_V1_CAPABILITIES;

pub const CORE_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn frontend_capabilities() -> BTreeSet<CapabilityId> {
    CORE_CLIENT_CAPABILITIES
        .iter()
        .map(|capability| CapabilityId((*capability).to_string()))
        .collect()
}

pub fn local_core_handshake() -> CoreHandshake {
    CoreHandshake {
        core_version: CORE_CLIENT_VERSION.to_string(),
        supported_schema_versions: vec![FRONTEND_SCHEMA_V1],
        active_schema_version: FRONTEND_SCHEMA_V1,
        capabilities: frontend_capabilities(),
    }
}

pub fn validate_schema_version(schema_version: SchemaVersion) -> Result<(), String> {
    if schema_version == FRONTEND_SCHEMA_V1 {
        Ok(())
    } else {
        Err(format!("unsupported frontend schema {}", schema_version.0))
    }
}

pub fn validate_handshake(handshake: &CoreHandshake) -> Result<(), String> {
    validate_schema_version(handshake.active_schema_version)?;
    if !handshake
        .supported_schema_versions
        .contains(&handshake.active_schema_version)
    {
        return Err(format!(
            "active frontend schema {} is not advertised as supported",
            handshake.active_schema_version.0
        ));
    }
    if !handshake
        .supported_schema_versions
        .contains(&FRONTEND_SCHEMA_V1)
    {
        return Err(format!(
            "required frontend schema {} is not supported",
            FRONTEND_SCHEMA_V1.0
        ));
    }
    validate_capabilities(&handshake.capabilities)
}

pub(crate) fn validate_capabilities(capabilities: &BTreeSet<CapabilityId>) -> Result<(), String> {
    for required in CORE_CLIENT_CAPABILITIES {
        if !capabilities.contains(&CapabilityId((*required).to_string())) {
            return Err(format!("missing core capability `{required}`"));
        }
    }
    Ok(())
}
