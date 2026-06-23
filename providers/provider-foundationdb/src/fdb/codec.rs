use serde::Serialize;
use serde::de::DeserializeOwned;
use sorla_provider_core::ProviderError;

/// Serialize a value to canonical CBOR (matching the IR canonical codec).
pub fn encode_value<T: Serialize>(value: &T) -> Result<Vec<u8>, ProviderError> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes)
        .map_err(|err| ProviderError::Validation(format!("cbor encode failed: {err}")))?;
    Ok(bytes)
}

/// Deserialize a value from CBOR bytes.
pub fn decode_value<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProviderError> {
    ciborium::de::from_reader(bytes)
        .map_err(|err| ProviderError::Validation(format!("cbor decode failed: {err}")))
}

#[cfg(test)]
mod tests {
    use super::{decode_value, encode_value};
    use sorla_provider_core::{CanonicalEntityRecord, SorNamespace};

    #[test]
    fn canonical_entity_record_round_trips_through_cbor() {
        let record = CanonicalEntityRecord {
            namespace: SorNamespace {
                tenant_id: "tenant-a".into(),
                sor_id: "contracts".into(),
                environment_id: None,
            },
            entity_type: "Contract".into(),
            entity_id: "contract-001".into(),
            canonical_version: "2026-05-22".into(),
            revision: 7,
            data_json: serde_json::json!({ "status": "active", "amount": 1250 }),
            created_at: "2026-05-22T10:00:00Z".into(),
            updated_at: "2026-05-22T11:00:00Z".into(),
        };
        let bytes = encode_value(&record).expect("encode");
        let parsed: CanonicalEntityRecord = decode_value(&bytes).expect("decode");
        assert_eq!(parsed, record);
    }

    #[test]
    fn decode_rejects_corrupt_bytes() {
        let err = decode_value::<CanonicalEntityRecord>(&[0xff, 0x00, 0x13]).unwrap_err();
        assert!(matches!(err, sorla_provider_core::ProviderError::Validation(_)));
    }
}
