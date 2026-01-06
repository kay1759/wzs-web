use anyhow::Result;
use async_trait::async_trait;

use super::email::Email;

/// Port trait for sending email messages.
///
/// This trait represents an **abstraction over email delivery mechanisms**.
/// Implementations may send emails via:
///
/// - SMTP
/// - File output (for development / testing)
/// - External services (SES, SendGrid, etc.)
///
/// ## Design notes
///
/// - This trait is intentionally **minimal**:
///   - It only accepts an [`Email`] value object
///   - It returns a generic `Result<()>`
///
/// - The trait does **not**:
///   - Perform validation of recipients
///   - Know about configuration sources (env, files, etc.)
///   - Decide whether an email *should* be sent
///
/// Those concerns belong to the application layer.
///
/// ## Thread safety
///
/// Implementations must be:
/// - `Send`: usable across thread boundaries
/// - `Sync`: safely shared via `Arc`
///
/// This allows `EmailSender` to be injected into async runtimes,
/// GraphQL resolvers, background tasks, etc.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Sends a single email message.
    ///
    /// ## Arguments
    ///
    /// - `email`: A fully constructed email value object.
    ///
    /// ## Returns
    ///
    /// - `Ok(())` if the email was successfully handed off to the transport
    /// - `Err(_)` if delivery failed for any reason
    ///
    /// ## Error handling
    ///
    /// Implementations should return meaningful errors, but callers
    /// should treat failures as **delivery errors**, not validation errors.
    async fn send(&self, email: Email) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use lettre::message::Mailbox;

    use crate::notification::email::EmailBody;

    /// A test double for `EmailSender`.
    ///
    /// This implementation records all emails passed to it,
    /// allowing tests to verify that:
    ///
    /// - `send` is called
    /// - The correct `Email` is passed
    ///
    /// No I/O or external systems are involved.
    #[derive(Default)]
    struct TestEmailSender {
        sent: Mutex<Vec<Email>>,
    }

    #[async_trait]
    impl EmailSender for TestEmailSender {
        async fn send(&self, email: Email) -> Result<()> {
            self.sent.lock().unwrap().push(email);
            Ok(())
        }
    }

    fn mb(addr: &str) -> Mailbox {
        addr.parse::<Mailbox>().expect("valid mailbox")
    }

    #[tokio::test]
    async fn email_sender_contract_allows_sending_email() {
        let sender = Arc::new(TestEmailSender::default());

        let email = Email {
            subject: "Test".to_string(),
            body: EmailBody::Text("Hello".to_string()),
            to: vec![mb("to@example.com")],
            cc: vec![],
            bcc: vec![],
        };

        sender
            .send(email.clone())
            .await
            .expect("send should succeed");

        let sent = sender.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].subject, "Test");
        assert_eq!(sent[0].to.len(), 1);
    }

    #[tokio::test]
    async fn email_sender_can_be_shared_across_threads() {
        let sender: Arc<dyn EmailSender> = Arc::new(TestEmailSender::default());

        let email = Email {
            subject: "Shared".to_string(),
            body: EmailBody::Text("Body".to_string()),
            to: vec![mb("to@example.com")],
            cc: vec![],
            bcc: vec![],
        };

        // Clone the Arc to simulate multi-owner usage
        let sender_clone = sender.clone();

        sender.send(email.clone()).await.unwrap();
        sender_clone.send(email).await.unwrap();
    }

    #[tokio::test]
    async fn email_sender_does_not_enforce_validation_rules() {
        // This test documents that the trait itself does not enforce
        // validation rules such as "must have at least one To recipient".
        // Such rules belong to the application layer.

        let sender = TestEmailSender::default();

        let email = Email {
            subject: "No recipients".to_string(),
            body: EmailBody::Text("Body".to_string()),
            to: vec![],
            cc: vec![],
            bcc: vec![],
        };

        sender.send(email).await.expect("send should succeed");
    }
}
