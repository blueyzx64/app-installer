//! Wire types shared with the frontend. Every type derives `specta::Type`
//! so `bindings.ts` is generated from here (ADR-0009).
//!
//! W0a: only the types needed by the spike commands. S1/S4 extend this once
//! the domain types are frozen.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum InstallPhaseDto {
    Preparing,
    WaitingForPermission,
    Downloading,
    Installing,
    Finishing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InstallEventDto {
    #[serde(rename_all = "camelCase")]
    Plan {
        extra_packages: u32,
        // Byte counts stay below 2^53 (packages are capped at 8 GiB), so `number` is lossless.
        #[specta(type = specta_typescript::Number)]
        download_bytes: u64,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        phase: InstallPhaseDto,
        percent: Option<f32>,
        eta_secs: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InstallOutcomeDto {
    Cancelled,
    #[serde(rename_all = "camelCase")]
    Failed {
        code: ErrorCode,
        detail: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_event_is_tagged_camel_case() {
        let json = serde_json::to_string(&InstallEventDto::Progress {
            phase: InstallPhaseDto::WaitingForPermission,
            percent: None,
            eta_secs: Some(3),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"kind":"progress","phase":"waitingForPermission","percent":null,"etaSecs":3}"#
        );
    }
}
