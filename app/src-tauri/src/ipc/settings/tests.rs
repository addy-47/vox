//! ============================================================================
//! src/ipc/settings/tests.rs — Unit tests for settings mutation and bounds validation
//! ============================================================================

#[cfg(test)]
mod tests {
    use super::super::mutation::apply_setting_mutation;
    use crate::core::settings::VoxSettings;
    use serde_json::json;

    #[test]
    fn test_apply_setting_mutation_type_safety() {
        let mut settings = VoxSettings::default();

        // 1. Valid key and correct type
        let res = apply_setting_mutation(&mut settings, "ui", "theme", &json!("light"));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.ui.theme, "light");

        // 2. Invalid domain ("invalid_domain")
        let res = apply_setting_mutation(&mut settings, "invalid_domain", "theme", &json!("dark"));
        assert_eq!(res, Ok(false));

        // 3. Unknown key within valid domain
        let res = apply_setting_mutation(&mut settings, "ui", "unknown_key", &json!("val"));
        assert_eq!(res, Ok(false));

        // 4. Type mismatch: string passed to boolean field (tray_enabled)
        let res = apply_setting_mutation(&mut settings, "ui", "tray_enabled", &json!("not_a_bool"));
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("tray_enabled must be a boolean"));

        // 5. Type mismatch: string passed to numeric field (tray_blur_density)
        let res = apply_setting_mutation(&mut settings, "ui", "tray_blur_density", &json!("dense"));
        assert!(res.is_err());

        // 6. Type mismatch: boolean passed to string field (theme)
        let res = apply_setting_mutation(&mut settings, "ui", "theme", &json!(true));
        assert!(res.is_err());
    }

    #[test]
    fn test_setting_numeric_bounds() {
        let mut settings = VoxSettings::default();

        // --- VAD threshold bounds ---
        // Valid threshold (0.75)
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(0.75));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.vad.threshold, 0.75);

        // Lower bound (0.0)
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(0.0));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.vad.threshold, 0.0);

        // Upper bound (1.0)
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(1.0));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.vad.threshold, 1.0);

        // Below 0.0 -> Err
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(-0.1));
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .contains("threshold must be between 0.0 and 1.0"));

        // Above 1.0 -> Err
        let res = apply_setting_mutation(&mut settings, "vad", "threshold", &json!(1.5));
        assert!(res.is_err());

        // --- Memory top_k_facts bounds ---
        // Valid top_k_facts
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(10));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.memory.top_k_facts, 10);

        // Lower boundary (1)
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(1));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.memory.top_k_facts, 1);

        // Upper boundary (100)
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(100));
        assert_eq!(res, Ok(true));
        assert_eq!(settings.memory.top_k_facts, 100);

        // Zero (0) -> Out of bounds
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(0));
        assert!(res.is_err());

        // Over 100 (101) -> Out of bounds
        let res = apply_setting_mutation(&mut settings, "memory", "top_k_facts", &json!(101));
        assert!(res.is_err());
    }
}
