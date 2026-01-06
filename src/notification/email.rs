use lettre::message::{header::ContentType, Mailbox};

/// A Value Object representing a complete email message.
///
/// This type is intentionally **transport-agnostic**:
/// - It does not know about SMTP, SES, SendGrid, etc.
/// - It only describes *what* should be sent (subject, recipients, body).
///
/// ### Recipients
/// - `to`, `cc`, `bcc` are lists (0..n).
/// - Whether an empty `to` is allowed is an application decision.
///   (For example, an adapter may fall back to a default recipient.)
#[derive(Debug, Clone)]
pub struct Email {
    /// Email subject line.
    ///
    /// Note: sanitization (e.g., header injection prevention) should be done
    /// in the transport adapter layer, because it depends on the actual protocol.
    pub subject: String,

    /// Email body representation (text-only, HTML, attachments, etc.).
    pub body: EmailBody,

    /// Primary recipients.
    pub to: Vec<Mailbox>,

    /// Carbon copy recipients.
    pub cc: Vec<Mailbox>,

    /// Blind carbon copy recipients.
    ///
    /// Avoid logging this list in application logs.
    pub bcc: Vec<Mailbox>,
}

/// The body representation of an email.
///
/// The variants are designed to map cleanly onto common MIME structures:
/// - `Text` -> `text/plain`
/// - `TextWithAttachments` -> `multipart/mixed` (text/plain + attachments)
/// - `TextAndHtml` -> `multipart/alternative` (text/plain + text/html)
/// - `TextAndHtmlWithAttachments` -> `multipart/mixed`
///    - child: `multipart/alternative` (text/plain + text/html)
///    - plus attachments
#[derive(Debug, Clone)]
pub enum EmailBody {
    /// Plain text only (`text/plain`).
    Text(String),

    /// Plain text with attachments.
    ///
    /// Typically encoded as `multipart/mixed`.
    TextWithAttachments {
        /// Plain text body (`text/plain`).
        text: String,
        /// Attachments to include.
        attachments: Vec<Attachment>,
    },

    /// Plain text + HTML (no attachments).
    ///
    /// Typically encoded as `multipart/alternative`.
    TextAndHtml {
        /// Plain text body (`text/plain`).
        text: String,
        /// HTML body (`text/html`).
        html: String,
    },

    /// Plain text + HTML + attachments.
    ///
    /// Typically encoded as `multipart/mixed` containing
    /// a `multipart/alternative` plus attachments.
    TextAndHtmlWithAttachments {
        /// Plain text body (`text/plain`).
        text: String,
        /// HTML body (`text/html`).
        html: String,
        /// Attachments to include.
        attachments: Vec<Attachment>,
    },
}

/// An in-memory email attachment.
///
/// This is kept purely in memory to keep infrastructure concerns (filesystem I/O)
/// out of transport adapters. The application layer can decide how to load bytes.
///
/// Notes:
/// - `filename` should be a safe display name (not necessarily a filesystem path).
/// - `content_type` should be the MIME type (e.g., `application/pdf`, `text/plain`).
#[derive(Debug, Clone)]
pub struct Attachment {
    /// Filename presented to the recipient (e.g., `document.pdf`).
    pub filename: String,

    /// MIME content type of this attachment.
    pub content_type: ContentType,

    /// Raw bytes of the attachment.
    pub bytes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mb(addr: &str) -> Mailbox {
        // For tests, parsing into Mailbox is sufficient.
        addr.parse::<Mailbox>().expect("valid mailbox")
    }

    #[test]
    fn email_is_cloneable_and_debuggable() {
        let email = Email {
            subject: "Subject".to_string(),
            body: EmailBody::Text("Hello".to_string()),
            to: vec![mb("to@example.com")],
            cc: vec![mb("cc@example.com")],
            bcc: vec![mb("bcc@example.com")],
        };

        // Clone
        let cloned = email.clone();
        assert_eq!(cloned.subject, "Subject");
        assert_eq!(cloned.to.len(), 1);
        assert_eq!(cloned.cc.len(), 1);
        assert_eq!(cloned.bcc.len(), 1);

        // Debug (just ensure it formats)
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn text_body_can_be_matched() {
        let email = Email {
            subject: "S".into(),
            body: EmailBody::Text("Plain".into()),
            to: vec![],
            cc: vec![],
            bcc: vec![],
        };

        match email.body {
            EmailBody::Text(t) => assert_eq!(t, "Plain"),
            _ => panic!("expected EmailBody::Text"),
        }
    }

    #[test]
    fn text_with_attachments_holds_bytes_and_metadata() {
        let attachment = Attachment {
            filename: "file.txt".into(),
            content_type: "text/plain"
                .parse::<ContentType>()
                .expect("valid content type"),
            bytes: b"hello".to_vec(),
        };

        let email = Email {
            subject: "S".into(),
            body: EmailBody::TextWithAttachments {
                text: "Body".into(),
                attachments: vec![attachment.clone()],
            },
            to: vec![mb("to@example.com")],
            cc: vec![],
            bcc: vec![],
        };

        match email.body {
            EmailBody::TextWithAttachments { text, attachments } => {
                assert_eq!(text, "Body");
                assert_eq!(attachments.len(), 1);
                assert_eq!(attachments[0].filename, "file.txt");
                assert_eq!(attachments[0].bytes, b"hello".to_vec());
            }
            _ => panic!("expected EmailBody::TextWithAttachments"),
        }
    }

    #[test]
    fn text_and_html_variant_holds_both_representations() {
        let email = Email {
            subject: "S".into(),
            body: EmailBody::TextAndHtml {
                text: "Text".into(),
                html: "<p>HTML</p>".into(),
            },
            to: vec![mb("to@example.com")],
            cc: vec![],
            bcc: vec![],
        };

        match email.body {
            EmailBody::TextAndHtml { text, html } => {
                assert_eq!(text, "Text");
                assert_eq!(html, "<p>HTML</p>");
            }
            _ => panic!("expected EmailBody::TextAndHtml"),
        }
    }

    #[test]
    fn text_and_html_with_attachments_variant_holds_all_parts() {
        let attachment = Attachment {
            filename: "doc.pdf".into(),
            content_type: "application/pdf"
                .parse::<ContentType>()
                .expect("valid content type"),
            bytes: vec![1, 2, 3],
        };

        let email = Email {
            subject: "S".into(),
            body: EmailBody::TextAndHtmlWithAttachments {
                text: "Text".into(),
                html: "<p>HTML</p>".into(),
                attachments: vec![attachment.clone()],
            },
            to: vec![mb("to@example.com")],
            cc: vec![mb("cc@example.com")],
            bcc: vec![],
        };

        match email.body {
            EmailBody::TextAndHtmlWithAttachments {
                text,
                html,
                attachments,
            } => {
                assert_eq!(text, "Text");
                assert_eq!(html, "<p>HTML</p>");
                assert_eq!(attachments.len(), 1);
                assert_eq!(attachments[0].filename, "doc.pdf");
                assert_eq!(attachments[0].bytes, vec![1, 2, 3]);
            }
            _ => panic!("expected EmailBody::TextAndHtmlWithAttachments"),
        }
    }

    #[test]
    fn recipients_can_be_empty_lists() {
        // This test documents that the VO itself does not enforce recipient presence.
        // Validation rules (e.g., "must have at least one To") are application decisions.
        let email = Email {
            subject: "S".into(),
            body: EmailBody::Text("Body".into()),
            to: vec![],
            cc: vec![],
            bcc: vec![],
        };

        assert!(email.to.is_empty());
        assert!(email.cc.is_empty());
        assert!(email.bcc.is_empty());
    }
}
