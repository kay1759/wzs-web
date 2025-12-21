use thiserror::Error;

/// A common error representing that a requested entity was not found.
///
/// This error is intended to be used across applications and layers
/// (repository, application, presentation) without depending on
/// domain-specific business rules.
///
/// # Design
/// - Infrastructure-agnostic (no DB / HTTP dependency)
/// - Reusable across multiple applications
/// - Suitable for repository or use case boundaries
///
/// # Example
/// ```
/// use wzs_web::error::entity::NotFoundError;
///
/// let err = NotFoundError::new("User");
/// assert_eq!(err.to_string(), "User not found");
/// ```
#[derive(Debug, Error)]
#[error("{entity} not found")]
pub struct NotFoundError {
    /// Name of the entity that was not found (e.g. `"User"`, `"Member"`)
    pub entity: &'static str,
}

impl NotFoundError {
    /// Create a new `NotFoundError` for the specified entity.
    ///
    /// # Example
    /// ```
    /// use wzs_web::error::entity::NotFoundError;
    ///
    /// let err = NotFoundError::new("Location");
    /// assert_eq!(err.entity, "Location");
    /// ```
    pub fn new(entity: &'static str) -> Self {
        Self { entity }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_entity_correctly() {
        let err = NotFoundError::new("User");
        assert_eq!(err.entity, "User");
    }

    #[test]
    fn display_format_is_correct() {
        let err = NotFoundError::new("Location");
        assert_eq!(err.to_string(), "Location not found");
    }

    #[test]
    fn debug_output_contains_struct_name_and_entity() {
        let err = NotFoundError::new("Order");
        let debug = format!("{:?}", err);

        assert!(debug.contains("NotFoundError"));
        assert!(debug.contains("Order"));
    }
}
