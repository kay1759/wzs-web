//! # Image Configuration
//!
//! Provides configuration parameters for image processing,
//! such as maximum allowed width and height.
//!
//! Configuration can be built from an [`EnvConfig`] environment snapshot.
//!
//! # Environment Variables
//!
//! - `IMAGE_MAX_WIDTH` — maximum image width in pixels (default: `1280`)
//! - `IMAGE_MAX_HEIGHT` — maximum image height in pixels (default: `1280`)
//!
//! Invalid numeric values fall back to their defaults.
//!
//! # Example
//!
//! ```rust
//! use wzs_web::config::env::EnvConfig;
//! use wzs_web::config::image::ImageConfig;
//!
//! let env = EnvConfig::from_pairs([
//!     ("IMAGE_MAX_WIDTH", "1920"),
//!     ("IMAGE_MAX_HEIGHT", "1080"),
//! ]);
//!
//! let cfg = ImageConfig::from_env_config(&env);
//!
//! assert_eq!(cfg.max_width, 1920);
//! assert_eq!(cfg.max_height, 1080);
//! ```

use crate::config::env::EnvConfig;

/// Default maximum image width in pixels.
const DEFAULT_MAX_WIDTH: u32 = 1280;

/// Default maximum image height in pixels.
const DEFAULT_MAX_HEIGHT: u32 = 1280;

/// Configuration for image processing or upload validation.
///
/// Defines upper limits for image dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageConfig {
    /// Maximum allowed image width in pixels.
    pub max_width: u32,

    /// Maximum allowed image height in pixels.
    pub max_height: u32,
}

impl ImageConfig {
    /// Builds an [`ImageConfig`] from an [`EnvConfig`] snapshot.
    ///
    /// Missing or invalid values use the following defaults:
    ///
    /// - `IMAGE_MAX_WIDTH`: `1280`
    /// - `IMAGE_MAX_HEIGHT`: `1280`
    ///
    /// # Example
    ///
    /// ```rust
    /// use wzs_web::config::env::EnvConfig;
    /// use wzs_web::config::image::ImageConfig;
    ///
    /// let env = EnvConfig::from_pairs([
    ///     ("IMAGE_MAX_WIDTH", "2048"),
    ///     ("IMAGE_MAX_HEIGHT", "1536"),
    /// ]);
    ///
    /// let cfg = ImageConfig::from_env_config(&env);
    ///
    /// assert_eq!(cfg.max_width, 2048);
    /// assert_eq!(cfg.max_height, 1536);
    /// ```
    pub fn from_env_config(env: &EnvConfig) -> Self {
        Self {
            max_width: env.get_u32("IMAGE_MAX_WIDTH").unwrap_or(DEFAULT_MAX_WIDTH),

            max_height: env
                .get_u32("IMAGE_MAX_HEIGHT")
                .unwrap_or(DEFAULT_MAX_HEIGHT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_config_holds_values() {
        let cfg = ImageConfig {
            max_width: 1920,
            max_height: 1080,
        };

        assert_eq!(cfg.max_width, 1920);
        assert_eq!(cfg.max_height, 1080);
    }

    #[test]
    fn from_env_config_uses_defaults() {
        let env = EnvConfig::default();

        let cfg = ImageConfig::from_env_config(&env);

        assert_eq!(cfg.max_width, 1280);
        assert_eq!(cfg.max_height, 1280);
    }

    #[test]
    fn from_env_config_reads_values() {
        let env =
            EnvConfig::from_pairs([("IMAGE_MAX_WIDTH", "2048"), ("IMAGE_MAX_HEIGHT", "1536")]);

        let cfg = ImageConfig::from_env_config(&env);

        assert_eq!(cfg.max_width, 2048);
        assert_eq!(cfg.max_height, 1536);
    }

    #[test]
    fn from_env_config_uses_defaults_for_invalid_values() {
        let env = EnvConfig::from_pairs([
            ("IMAGE_MAX_WIDTH", "invalid"),
            ("IMAGE_MAX_HEIGHT", "not-a-number"),
        ]);

        let cfg = ImageConfig::from_env_config(&env);

        assert_eq!(cfg.max_width, 1280);
        assert_eq!(cfg.max_height, 1280);
    }

    #[test]
    fn from_env_config_allows_independent_values() {
        let env =
            EnvConfig::from_pairs([("IMAGE_MAX_WIDTH", "1920"), ("IMAGE_MAX_HEIGHT", "invalid")]);

        let cfg = ImageConfig::from_env_config(&env);

        assert_eq!(cfg.max_width, 1920);
        assert_eq!(cfg.max_height, 1280);
    }

    #[test]
    fn image_config_clone_and_debug() {
        let cfg = ImageConfig {
            max_width: 800,
            max_height: 600,
        };

        let clone = cfg.clone();

        assert_eq!(cfg, clone);

        let debug = format!("{cfg:?}");

        assert!(debug.contains("800"));
        assert!(debug.contains("600"));
    }

    #[test]
    fn image_config_equality_check() {
        let cfg1 = ImageConfig {
            max_width: 100,
            max_height: 200,
        };

        let cfg2 = ImageConfig {
            max_width: 100,
            max_height: 200,
        };

        let cfg3 = ImageConfig {
            max_width: 300,
            max_height: 400,
        };

        assert_eq!(cfg1, cfg2);
        assert_ne!(cfg1, cfg3);
    }
}
