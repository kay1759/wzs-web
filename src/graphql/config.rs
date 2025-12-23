/// Configuration for GraphQL authentication handling.
///
/// This configuration is injected via `axum::Extension` and
/// controls how authentication information is extracted.
#[derive(Clone, Debug)]
pub struct GraphqlAuthConfig {
    /// Cookie name that stores the JWT payload.
    ///
    /// Example: `"foo_token"`
    pub jwt_cookie_name: String,
}

impl GraphqlAuthConfig {
    pub fn new(jwt_cookie_name: impl Into<String>) -> Self {
        Self {
            jwt_cookie_name: jwt_cookie_name.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_config_with_str_literal() {
        let cfg = GraphqlAuthConfig::new("foo_token");

        assert_eq!(cfg.jwt_cookie_name, "foo_token");
    }

    #[test]
    fn creates_config_with_string() {
        let name = String::from("auth_token");
        let cfg = GraphqlAuthConfig::new(name.clone());

        assert_eq!(cfg.jwt_cookie_name, name);
    }

    #[test]
    fn config_is_cloneable() {
        let cfg = GraphqlAuthConfig::new("foo_token");
        let cloned = cfg.clone();

        assert_eq!(cfg.jwt_cookie_name, cloned.jwt_cookie_name);
    }

    #[test]
    fn debug_output_contains_cookie_name() {
        let cfg = GraphqlAuthConfig::new("foo_token");
        let debug = format!("{:?}", cfg);

        // Debug 表現の厳密なフォーマットには依存しない
        assert!(debug.contains("GraphqlAuthConfig"));
        assert!(debug.contains("foo_token"));
    }
}
