/// An authenticated principal extracted from an authentication mechanism
/// (e.g. JWT).
///
/// # Overview
///
/// `CurrentUser` represents the *result of authentication*, not a domain user.
/// It deliberately does **not** contain any application-specific concepts such
/// as:
///
/// - user / member / admin
/// - roles or permissions
/// - domain status or profile information
///
/// Instead, it only carries the **JWT subject**, leaving all interpretation
/// and authorization decisions to the application layer.
///
/// # Design Intent
///
/// - Keep `wzs-web` independent from application domain models
/// - Allow multiple projects to share the same authentication abstraction
/// - Preserve clear boundaries between:
///   - authentication (library responsibility)
///   - authorization (application responsibility)
///
/// # Typical Usage
///
/// ```rust
/// use wzs_web::auth::CurrentUser;
///
/// let user = CurrentUser::new("123");
/// assert_eq!(user.subject, "123");
/// ```
///
/// In a GraphQL resolver or handler:
///
/// ```ignore
/// let user = ctx.data::<Option<CurrentUser>>()?
///     .ok_or(Error::Unauthorized)?;
///
/// let member_id = user.subject.parse::<i64>()?;
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentUser {
    /// The JWT `sub` (subject) claim.
    ///
    /// Its semantic meaning is intentionally undefined at this layer.
    /// The application decides whether it represents a user ID, member ID,
    /// admin ID, or something else.
    pub subject: String,
}

impl CurrentUser {
    /// Creates a new `CurrentUser` from a JWT subject.
    ///
    /// This constructor performs no validation and does not interpret the
    /// subject in any way.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wzs_web::auth::CurrentUser;
    ///
    /// let user = CurrentUser::new("user-123");
    /// assert_eq!(user.subject, "user-123");
    /// ```
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_current_user_with_string_subject() {
        let user = CurrentUser::new("123");

        assert_eq!(user.subject, "123");
    }

    #[test]
    fn creates_current_user_with_owned_string() {
        let subject = String::from("admin-42");
        let user = CurrentUser::new(subject.clone());

        assert_eq!(user.subject, subject);
    }

    #[test]
    fn current_user_is_cloneable() {
        let user = CurrentUser::new("abc");
        let cloned = user.clone();

        assert_eq!(user, cloned);
    }

    #[test]
    fn current_user_does_not_interpret_subject() {
        let user = CurrentUser::new("member:999");

        // The library must not make assumptions about the subject format
        assert_eq!(user.subject, "member:999");
    }
}
