use anyhow::{Context, Result};
use async_trait::async_trait;
use lettre::message::{Attachment as LettreAttachment, Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use tracing::info;

use crate::notification::{
    email::{Email, EmailBody},
    email_sender::EmailSender,
};

/// SMTP-based implementation of [`EmailSender`].
///
/// ## Responsibilities
///
/// - Builds a MIME-compliant email message from [`Email`]
/// - Sends the message via SMTP using STARTTLS
///
/// ## Assumptions
///
/// - STARTTLS is used (typically port 587)
/// - Recipient validation is handled by the application layer
///
/// ## What this type does *not* do
///
/// - Read files from disk
/// - Validate business rules (e.g. required recipients)
/// - Load configuration from environment variables
///
/// Those concerns belong to higher layers.
#[derive(Clone, Debug)]
pub struct SmtpEmailSender {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    default_to: Vec<Mailbox>,
}

impl SmtpEmailSender {
    /// Constructs a new `SmtpEmailSender`.
    ///
    /// ## Arguments
    ///
    /// - `smtp_host`: SMTP server hostname
    /// - `smtp_port`: SMTP server port (usually 587)
    /// - `username`: SMTP username
    /// - `password`: SMTP password
    /// - `from_email`: Sender email address
    /// - `from_name`: Sender display name
    /// - `default_to`: Fallback recipients when `Email.to` is empty
    pub fn new(
        smtp_host: &str,
        smtp_port: u16,
        username: &str,
        password: &str,
        from_email: &str,
        from_name: &str,
        default_to: Vec<Mailbox>,
    ) -> Result<Self> {
        info!(
            "SMTP init: host={} port={} user={} from={} default_to_count={}",
            smtp_host,
            smtp_port,
            username,
            from_email,
            default_to.len()
        );

        let creds = Credentials::new(username.to_string(), password.to_string());

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
            .with_context(|| format!("invalid relay host: {smtp_host}"))?
            .port(smtp_port)
            .credentials(creds)
            .build();

        let from = Mailbox::new(Some(from_name.to_string()), from_email.parse()?);

        Ok(Self {
            mailer,
            from,
            default_to,
        })
    }

    /// Builds a `lettre::Message` from an [`Email`].
    ///
    /// This method contains all MIME construction logic and is kept
    /// separate to allow unit testing without performing SMTP I/O.
    fn build_message(&self, email: Email) -> Result<Message> {
        // Sanitize subject to prevent header injection
        let mut subject = email.subject;
        subject.retain(|c| c != '\r' && c != '\n');

        let mut builder = Message::builder().from(self.from.clone()).subject(subject);

        // To: use default recipients if none are provided
        if email.to.is_empty() {
            for to in &self.default_to {
                builder = builder.to(to.clone());
            }
        } else {
            for to in email.to {
                builder = builder.to(to);
            }
        }

        // Cc / Bcc
        for cc in email.cc {
            builder = builder.cc(cc);
        }
        for bcc in email.bcc {
            builder = builder.bcc(bcc);
        }

        let message = match email.body {
            EmailBody::Text(text) => builder.singlepart(SinglePart::plain(text))?,

            EmailBody::TextWithAttachments { text, attachments } => {
                let mut mixed = MultiPart::mixed().singlepart(SinglePart::plain(text));
                for a in attachments {
                    let part = LettreAttachment::new(a.filename).body(a.bytes, a.content_type);
                    mixed = mixed.singlepart(part);
                }
                builder.multipart(mixed)?
            }

            EmailBody::TextAndHtml { text, html } => {
                let alternative = MultiPart::alternative()
                    .singlepart(SinglePart::plain(text))
                    .singlepart(SinglePart::html(html));
                builder.multipart(alternative)?
            }

            EmailBody::TextAndHtmlWithAttachments {
                text,
                html,
                attachments,
            } => {
                let alternative = MultiPart::alternative()
                    .singlepart(SinglePart::plain(text))
                    .singlepart(SinglePart::html(html));

                let mut mixed = MultiPart::mixed().multipart(alternative);
                for a in attachments {
                    let part = LettreAttachment::new(a.filename).body(a.bytes, a.content_type);
                    mixed = mixed.singlepart(part);
                }
                builder.multipart(mixed)?
            }
        };

        Ok(message)
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(&self, email: Email) -> Result<()> {
        let message = self.build_message(email)?;
        self.mailer
            .send(message)
            .await
            .context("SMTP send failed")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettre::message::header::ContentType;

    fn mb(addr: &str) -> Mailbox {
        addr.parse::<Mailbox>().expect("valid mailbox")
    }

    fn test_sender() -> SmtpEmailSender {
        SmtpEmailSender::new(
            "smtp.example.com",
            587,
            "user",
            "pass",
            "from@example.com",
            "Sender",
            vec![mb("default@example.com")],
        )
        .expect("sender should be created")
    }

    #[test]
    fn builds_message_with_default_to_when_to_is_empty() {
        let sender = test_sender();

        let email = Email {
            subject: "Test".into(),
            body: EmailBody::Text("Body".into()),
            to: vec![],
            cc: vec![],
            bcc: vec![],
        };

        let msg = sender.build_message(email).expect("message build");

        let formatted = msg.formatted();
        let raw = String::from_utf8_lossy(&formatted);

        assert!(raw.contains("default@example.com"));
        assert!(raw.contains("Subject: Test"));
    }

    #[test]
    fn builds_message_with_explicit_to_over_default() {
        let sender = test_sender();

        let email = Email {
            subject: "Explicit".into(),
            body: EmailBody::Text("Body".into()),
            to: vec![mb("to@example.com")],
            cc: vec![],
            bcc: vec![],
        };

        let msg = sender.build_message(email).expect("message build");
        let formatted = msg.formatted();
        let raw = String::from_utf8_lossy(&formatted);

        assert!(raw.contains("to@example.com"));
        assert!(!raw.contains("default@example.com"));
    }

    #[test]
    fn builds_text_and_html_multipart() {
        let sender = test_sender();

        let email = Email {
            subject: "HTML".into(),
            body: EmailBody::TextAndHtml {
                text: "plain".into(),
                html: "<p>html</p>".into(),
            },
            to: vec![mb("to@example.com")],
            cc: vec![],
            bcc: vec![],
        };

        let msg = sender.build_message(email).unwrap();
        let formatted = msg.formatted();
        let raw = String::from_utf8_lossy(&formatted);

        assert!(raw.contains("Content-Type: multipart/alternative"));
        assert!(raw.contains("plain"));
        assert!(raw.contains("<p>html</p>"));
    }

    #[test]
    fn builds_message_with_attachment() {
        let sender = test_sender();

        let attachment = crate::notification::email::Attachment {
            filename: "file.txt".into(),
            content_type: "text/plain".parse::<ContentType>().unwrap(),
            bytes: b"hello".to_vec(),
        };

        let email = Email {
            subject: "Attach".into(),
            body: EmailBody::TextWithAttachments {
                text: "Body".into(),
                attachments: vec![attachment],
            },
            to: vec![mb("to@example.com")],
            cc: vec![],
            bcc: vec![],
        };

        let msg = sender.build_message(email).unwrap();
        let formatted = msg.formatted();
        let raw = String::from_utf8_lossy(&formatted);

        assert!(raw.contains("Content-Type: multipart/mixed"));
        assert!(raw.contains("file.txt"));
        assert!(raw.contains("hello"));
    }
}
