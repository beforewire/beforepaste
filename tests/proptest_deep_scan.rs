// Property-based invariant for the deep-scan key/value heuristic.

use beforepaste::detector::deep_scan::deep_scan;
use beforepaste::detector::entropy::shannon_entropy;
use proptest::prelude::*;

proptest! {
    // Integration-test targets have no crate source root, so the default
    // file-based failure persistence cannot resolve a path; disable it.
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    // A secret-looking key paired with a high-entropy value always yields at
    // least one deep finding.
    #[test]
    fn kv_with_high_entropy_emits_finding(
        keyword in prop::sample::select(vec![
            "password", "api_key", "secret_key", "token", "auth_key",
        ]),
        token in "[A-Za-z0-9]{20,40}",
    ) {
        prop_assume!(shannon_entropy(&token) >= 4.0);
        let text = format!("{keyword}: {token}");
        let f = deep_scan(&text);
        prop_assert!(!f.is_empty());
    }
}
