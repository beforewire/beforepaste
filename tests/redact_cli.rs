use beforepaste::config::{Config, RedactStyle};
use beforepaste::detector::Detector;
use beforepaste::redact_cli::redact_with;

fn fresh_cfg() -> Config {
    let mut c = Config {
        onboarding_done: true,
        ..Config::default()
    };
    beforepaste::detector::presets::Preset::Balanced.apply(&mut c);
    c
}

#[test]
fn pipe_redacts_openai_key() {
    let cfg = fresh_cfg();
    let det = Detector::from_config(&cfg);
    let input = "API key for testing: sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghij\nend\n";
    let (out, names) = redact_with(&det, &cfg, input);
    assert_ne!(out, input, "expected the secret to be redacted");
    assert!(
        !out.contains("sk-proj-abcdefghijklmnopqrstuvwxyz"),
        "raw key leaked: {}",
        out
    );
    assert!(!names.is_empty(), "expected at least one matched pattern");
}

#[test]
fn pipe_passes_clean_text_through_unchanged() {
    let cfg = fresh_cfg();
    let det = Detector::from_config(&cfg);
    let input = "this is just a comment about the weather today\n";
    let (out, names) = redact_with(&det, &cfg, input);
    assert_eq!(out, input);
    assert!(names.is_empty());
}

#[test]
fn export_assignments_keep_names_and_redact_values() {
    let cfg = fresh_cfg();
    let det = Detector::from_config(&cfg);
    let input = concat!(
        "export E2B_API_KEY=\"e2b_xxxxxxxxxx\"\n",
        "export ALIYUN_ACCESS_KEY_ID=LTxxxxxxx\n",
        "export ALIYUN_ACCESS_KEY_SECRET=Kxxxxxxx      \n",
    );
    let (out, names) = redact_with(&det, &cfg, input);

    assert!(!names.is_empty(), "expected assignment patterns to fire");
    assert_eq!(
        out,
        concat!(
            "export E2B_API_KEY=\"[REDACTED]\"\n",
            "export ALIYUN_ACCESS_KEY_ID=LTxxxxxxx\n",
            "export ALIYUN_ACCESS_KEY_SECRET=[REDACTED]      \n",
        )
    );
}

#[test]
fn short_labeled_secret_values_are_redacted() {
    let cfg = fresh_cfg();
    let det = Detector::from_config(&cfg);
    let input = concat!(
        "model_name: deepseek-demo\n",
        "base_url: https://example.invalid/v1\n",
        "api_key: sk-demo123\n",
        "**api_key**: sk-demo456\n",
        "export ALIYUN_ACCESS_KEY_SECRET=abcd\n",
    );
    let (out, names) = redact_with(&det, &cfg, input);

    assert!(!names.is_empty(), "expected short labeled secrets to fire");
    assert_eq!(
        out,
        concat!(
            "model_name: deepseek-demo\n",
            "base_url: https://example.invalid/v1\n",
            "api_key: [REDACTED]\n",
            "**api_key**: [REDACTED]\n",
            "export ALIYUN_ACCESS_KEY_SECRET=[REDACTED]\n",
        )
    );
}

#[test]
fn typed_short_labeled_secret_values_keep_key_names() {
    let mut cfg = fresh_cfg();
    cfg.redact_style = RedactStyle::Typed;
    let det = Detector::from_config(&cfg);
    let input = concat!(
        "api_key: sk-demo123\n",
        "**api_key**: sk-demo456\n",
        "export ALIYUN_ACCESS_KEY_SECRET=abcd\n",
    );
    let (once, names) = redact_with(&det, &cfg, input);

    assert!(!names.is_empty(), "expected short labeled secrets to fire");
    assert_eq!(
        once,
        concat!(
            "api_key: [API_KEY]\n",
            "**api_key**: [API_KEY]\n",
            "export ALIYUN_ACCESS_KEY_SECRET=[ALIYUN_ACCESS_KEY_SECRET]\n",
        )
    );

    let (twice, twice_names) = redact_with(&det, &cfg, &once);
    assert_eq!(twice, once);
    assert!(
        twice_names.is_empty(),
        "already-redacted short labels should not be detected again"
    );
}

#[test]
fn typed_dotenv_assignment_redaction_is_idempotent_with_unbalanced_quote() {
    let mut cfg = fresh_cfg();
    cfg.redact_style = RedactStyle::Typed;
    let det = Detector::from_config(&cfg);
    let input = "export ANTHROPIC_API_KEY=\"sk-ant-api03-";

    let (once, once_names) = redact_with(&det, &cfg, input);
    assert_eq!(once, "export ANTHROPIC_API_KEY=\"[ANTHROPIC_API_KEY]");
    assert!(
        !once_names.is_empty(),
        "expected dotenv secret to be redacted"
    );

    let (twice, twice_names) = redact_with(&det, &cfg, &once);
    assert_eq!(twice, once);
    assert!(
        twice_names.is_empty(),
        "already-redacted typed marker should not be detected again"
    );
}
