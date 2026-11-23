//! Validated geometry subset of the PC `zcinput` remote configuration.
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComposerConfig {
    pub rich_max_lines: f32,
    pub line_height: f32,
}

impl Default for ComposerConfig {
    fn default() -> Self {
        Self {
            rich_max_lines: 12.5,
            line_height: 22.0,
        }
    }
}

impl ComposerConfig {
    /// Accept only the documented version and finite, bounded geometry fields.
    /// This does not enable RTF or any remote feature flag.
    pub fn from_server_info(response: &Value) -> Option<Self> {
        if crate::api_response::integer(response.get("error_code")?) != Some(0) {
            return None;
        }
        let data = response.get("data")?;
        let config = data.pointer("/setttings/features/zcinput")?;
        Self::parse(config)
    }

    pub fn from_login_config(config: &Value) -> Option<Self> {
        Self::parse(config.pointer("/settings/features/zcinput")?)
    }

    pub(crate) fn from_value(config: &Value) -> Option<Self> {
        Self::parse(config)
    }

    fn parse(config: &Value) -> Option<Self> {
        if config.get("version").and_then(Value::as_u64) != Some(1) {
            return None;
        }
        let defaults = Self::default();
        // The UI owns the viewport clamp. These bounds only protect rendering
        // against malformed/unreasonable server values; they are fork limits.
        let bounded = |value: Option<&Value>, min: f64, max: f64, fallback: f32| {
            value
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value >= min && *value <= max)
                .map(|value| value as f32)
                .unwrap_or(fallback)
        };
        Some(Self {
            rich_max_lines: bounded(
                config.pointer("/behavior/inputMaxLines"),
                1.0,
                50.0,
                defaults.rich_max_lines,
            ),
            line_height: bounded(
                config.pointer("/style/lineHeight"),
                12.0,
                48.0,
                defaults.line_height,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn server_composer_geometry_accepts_versioned_fractional_lines() {
        let response = json!({"error_code":0,"data":{"setttings":{"features":{"zcinput":{"version":1,"behavior":{"inputMaxLines":7.5},"style":{"lineHeight":24}}}}}});
        assert_eq!(
            ComposerConfig::from_server_info(&response),
            Some(ComposerConfig {
                rich_max_lines: 7.5,
                line_height: 24.0
            })
        );
        let auth = json!({"settings":{"features":{"zcinput":{"version":1,"behavior":{"inputMaxLines":7.5},"style":{"lineHeight":24}}}}});
        let mut session = crate::session::Session::new(false, false, false).unwrap();
        crate::login::apply_login_config(&mut session, &auth);
        assert_eq!(
            session.composer_config,
            ComposerConfig::from_server_info(&response)
        );
    }

    #[test]
    fn server_composer_geometry_falls_back_on_bad_fields_and_rejects_wrong_version() {
        let mut response = json!({"error_code":0,"data":{"setttings":{"features":{"zcinput":{"version":1,"behavior":{"inputMaxLines":-1},"style":{"lineHeight":"24"}}}}}});
        assert_eq!(
            ComposerConfig::from_server_info(&response),
            Some(ComposerConfig::default())
        );
        response["data"]["setttings"]["features"]["zcinput"]["version"] = json!(2);
        assert_eq!(ComposerConfig::from_server_info(&response), None);
        response["error_code"] = json!(204);
        assert_eq!(ComposerConfig::from_server_info(&response), None);
    }
}
