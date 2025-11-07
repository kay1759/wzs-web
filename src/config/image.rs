//! # Image Configuration
//!
//! Provides basic configuration parameters for image processing,
//! such as maximum allowed width and height.
//!
//! Typically used to constrain uploaded image sizes or
//! to define resize limits in image processing pipelines.
//!
//! # Example
//! ```rust
//! use wzs_web::config::image::ImageConfig;
//!
//! let cfg = ImageConfig {
//!     max_width: 1920,
//!     max_height: 1080,
//! };
//! assert_eq!(cfg.max_width, 1920);
//! assert_eq!(cfg.max_height, 1080);
//! ```

/// Configuration for image processing or upload validation.
///
/// Defines upper limits for image dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageConfig {
    pub max_width: u32,
    pub max_height: u32,
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
    fn image_config_clone_and_debug() {
        let cfg = ImageConfig {
            max_width: 800,
            max_height: 600,
        };

        let clone = cfg.clone();
        assert_eq!(cfg, clone);

        let dbg_str = format!("{:?}", cfg);
        assert!(dbg_str.contains("800"));
        assert!(dbg_str.contains("600"));
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
