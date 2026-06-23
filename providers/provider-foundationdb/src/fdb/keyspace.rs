use sorla_provider_core::SorNamespace;

use crate::encode_key_segment;

/// Builds ordered FDB byte keys under `/sorx/{tenant}/{sor}`.
/// Sequences are zero-padded to 20 digits so byte order == numeric order.
#[allow(dead_code)]
pub struct FdbKeyspace {
    root: String,
}

#[allow(dead_code)]
impl FdbKeyspace {
    pub fn new(namespace: &SorNamespace) -> Self {
        let root = format!(
            "/sorx/{}/{}",
            encode_key_segment(&namespace.tenant_id),
            encode_key_segment(&namespace.sor_id),
        );
        Self { root }
    }

    fn at(&self, suffix: &str) -> Vec<u8> {
        format!("{}{suffix}", self.root).into_bytes()
    }

    pub fn canonical_event_key(&self, stream_id: &str, sequence: u64) -> Vec<u8> {
        self.at(&format!(
            "/events/{}/{sequence:020}",
            encode_key_segment(stream_id)
        ))
    }

    pub fn canonical_event_head_key(&self, stream_id: &str) -> Vec<u8> {
        self.at(&format!("/events/{}/_head", encode_key_segment(stream_id)))
    }

    pub fn canonical_event_stream_range(&self, stream_id: &str) -> (Vec<u8>, Vec<u8>) {
        // lo = first possible padded key; hi = exclusive upper bound that
        // EXCLUDES the `_head` key. The padded sequence keys live at
        // `/events/{enc}/{000..0}`..`/events/{enc}/{999..9}`. Choose bounds so
        // every padded key sorts within [lo, hi) and `_head` (which sorts as
        // `_` = 0x5f, AFTER digits 0x30-0x39) is OUTSIDE the range.
        //
        // ASCII order: digits `0`(0x30)..`9`(0x39) < `:`(0x3a) < `_`(0x5f)
        // So `hi = .../events/{enc}/:` works: padded keys (start with a digit)
        // are `< hi`, and `_head` (starts with `_` = 0x5f > 0x3a) is `>= hi`.
        let enc = encode_key_segment(stream_id);
        let lo = self.at(&format!("/events/{enc}/0"));
        let hi = self.at(&format!("/events/{enc}/:")); // ':' = 0x3a, just after '9' (0x39), before '_'
        (lo, hi)
    }

    pub fn canonical_entity_key(&self, entity_type: &str, entity_id: &str) -> Vec<u8> {
        self.at(&format!("/entities/{entity_type}\u{1f}{entity_id}"))
    }

    pub fn edge_out_key(&self, relationship_type: &str, from: &str, to: &str) -> Vec<u8> {
        self.at(&format!(
            "/edges/out/{relationship_type}\u{1f}{from}\u{1f}{to}"
        ))
    }

    pub fn edge_in_key(&self, relationship_type: &str, to: &str, from: &str) -> Vec<u8> {
        self.at(&format!(
            "/edges/in/{relationship_type}\u{1f}{to}\u{1f}{from}"
        ))
    }

    pub fn idempotency_key(&self, idem: &str) -> Vec<u8> {
        self.at(&format!("/idempotency/{idem}"))
    }

    /// Key for an entity link, scoped by the linked entity token, the source
    /// reference, and the match kind so distinct links to the same entity do
    /// not collide while re-linking the same (entity, source, match) is
    /// idempotent.
    pub fn entity_link_key(
        &self,
        entity_token: &str,
        source_ref: &str,
        match_kind: &str,
    ) -> Vec<u8> {
        self.at(&format!(
            "/links/{entity_token}\u{1f}{source_ref}\u{1f}{match_kind}"
        ))
    }

    pub fn projection_key(&self, name: &str, key: &str) -> Vec<u8> {
        self.at(&format!("/projections/{name}\u{1f}{key}"))
    }

    pub fn checkpoint_key(&self, name: &str) -> Vec<u8> {
        self.at(&format!("/checkpoints/{name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::FdbKeyspace;
    use sorla_provider_core::SorNamespace;

    fn ns() -> SorNamespace {
        SorNamespace {
            tenant_id: "tenant/acme".into(),
            sor_id: "contracts".into(),
            environment_id: Some("dev".into()),
        }
    }

    #[test]
    fn root_ignores_environment_and_encodes_segments() {
        let ks = FdbKeyspace::new(&ns());
        let key = ks.canonical_entity_key("Contract", "c-1");
        let text = String::from_utf8_lossy(&key);
        // tenant/acme -> tenant%2Facme ; environment_id must NOT appear
        assert!(text.starts_with("/sorx/tenant%2Facme/contracts/entities/"));
        assert!(!text.contains("dev"));
        assert!(text.ends_with("Contract\u{1f}c-1"));
    }

    #[test]
    fn event_keys_are_lexicographically_ordered_by_sequence() {
        let ks = FdbKeyspace::new(&ns());
        let k9 = ks.canonical_event_key("Contract/c-1", 9);
        let k10 = ks.canonical_event_key("Contract/c-1", 10);
        assert!(k9 < k10, "zero-padded sequence must sort numerically");
    }

    #[test]
    fn stream_range_brackets_all_events() {
        let ks = FdbKeyspace::new(&ns());
        let (lo, hi) = ks.canonical_event_stream_range("Contract/c-1");
        let k = ks.canonical_event_key("Contract/c-1", 5);
        assert!(lo <= k && k < hi);
    }

    #[test]
    fn idempotency_and_edge_keys_are_distinct_subspaces() {
        let ks = FdbKeyspace::new(&ns());
        let idem = ks.idempotency_key("idem-1");
        let edge = ks.edge_out_key("has_contract", "Customer\u{1f}cust-1", "Contract\u{1f}c-1");
        assert_ne!(idem, edge);
        assert!(String::from_utf8_lossy(&edge).contains("/edges/out/"));
    }

    #[test]
    fn head_key_is_outside_stream_event_range() {
        let ks = FdbKeyspace::new(&ns());
        let (_, hi) = ks.canonical_event_stream_range("Contract/c-1");
        let head = ks.canonical_event_head_key("Contract/c-1");
        // `_head` starts with `_` (0x5f) which is > `:` (0x3a, the high bound prefix),
        // so the head key must sort >= hi (i.e. outside the half-open event range).
        assert!(
            head >= hi,
            "head key ({}) must be >= hi bound ({}) to be excluded from the event range",
            String::from_utf8_lossy(&head),
            String::from_utf8_lossy(&hi)
        );
    }
}
