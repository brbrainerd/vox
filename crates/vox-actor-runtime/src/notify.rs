//! Multi-channel notification primitive (GA-14).
//!
//! Implements the `Notify { channel, recipient, template }` value type from
//! CC-03. The structural type-safety lever is **the recipient type
//! constrains the channel** — sending an SMS to an `Email`-typed recipient
//! is a compile error, not a runtime exception.
//!
//! This module is the runtime trait surface; concrete adapter implementations
//! (Resend, SES, Twilio, web-push) plug in via the `NotifyDispatcher`
//! trait. Adapters live in feature-gated submodules so users don't pay the
//! transitive dependency cost for channels they don't use.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

/// A typed notification recipient — channel is structurally fixed by the
/// recipient variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    Email { address: String },
    Sms { phone: String },
    WebPush { subscription_json: String },
}

impl Recipient {
    pub fn channel(&self) -> Channel {
        match self {
            Recipient::Email { .. } => Channel::Email,
            Recipient::Sms { .. } => Channel::Sms,
            Recipient::WebPush { .. } => Channel::WebPush,
        }
    }
}

/// Channel discriminant — what wire mechanism delivers the notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Email,
    Sms,
    WebPush,
}

/// A notification ready for dispatch.
#[derive(Debug, Clone)]
pub struct Notification {
    pub recipient: Recipient,
    /// Subject line (Email) or short title (WebPush). Ignored for SMS.
    pub subject: Option<String>,
    /// Rendered body text. For email, this is plain-text fallback; HTML body
    /// rides in `metadata["html_body"]`.
    pub body: String,
    /// Free-form per-adapter metadata (deep-link URL, message-id, etc.).
    pub metadata: HashMap<String, String>,
}

/// Outcome of a delivery attempt.
#[derive(Debug, Clone)]
pub enum DeliveryOutcome {
    Delivered {
        provider_message_id: String,
        at: SystemTime,
    },
    Bounced {
        reason: String,
        at: SystemTime,
    },
    Deferred {
        retry_after: SystemTime,
    },
    Failed {
        error: String,
        at: SystemTime,
    },
}

/// Dispatch trait. Adapters (`vox_notify_resend`, `vox_notify_twilio`, etc.)
/// implement this once and register via [`NotifyRegistry`].
pub trait NotifyDispatcher: Send + Sync {
    /// Channels this dispatcher handles. Used by the registry to route.
    fn channels(&self) -> &[Channel];
    /// Dispatch a single notification. Errors land in `DeliveryOutcome::Failed`
    /// so callers do not need to handle both `Result::Err` and bounce paths.
    fn dispatch(&self, notification: &Notification) -> DeliveryOutcome;
}

/// Channel → dispatcher routing registry.
///
/// Dispatchers are stored as `Arc<dyn NotifyDispatcher>` so a single adapter
/// can register itself for multiple channels (e.g., a unified adapter that
/// handles Email + WebPush) without double-boxing.
pub struct NotifyRegistry {
    by_channel: HashMap<Channel, Arc<dyn NotifyDispatcher>>,
}

impl NotifyRegistry {
    pub fn new() -> Self {
        Self {
            by_channel: HashMap::new(),
        }
    }

    /// Register a dispatcher for every channel it advertises.
    ///
    /// Duplicate-channel registration follows last-writer-wins; the test
    /// suite covers the typical one-adapter-per-channel case.
    pub fn register(&mut self, dispatcher: Arc<dyn NotifyDispatcher>) {
        for &c in dispatcher.channels() {
            self.by_channel.insert(c, Arc::clone(&dispatcher));
        }
    }

    pub fn dispatch(&self, n: &Notification) -> Option<DeliveryOutcome> {
        let channel = n.recipient.channel();
        self.by_channel.get(&channel).map(|d| d.dispatch(n))
    }
}

impl Default for NotifyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDispatcher {
        channel: Channel,
    }

    impl NotifyDispatcher for MockDispatcher {
        fn channels(&self) -> &[Channel] {
            std::slice::from_ref(&self.channel)
        }
        fn dispatch(&self, _n: &Notification) -> DeliveryOutcome {
            DeliveryOutcome::Delivered {
                provider_message_id: "mock-id".into(),
                at: SystemTime::now(),
            }
        }
    }

    #[test]
    fn email_recipient_routes_to_email_channel() {
        let r = Recipient::Email {
            address: "x@example.com".into(),
        };
        assert_eq!(r.channel(), Channel::Email);
    }

    #[test]
    fn sms_recipient_routes_to_sms_channel() {
        let r = Recipient::Sms {
            phone: "+15551234567".into(),
        };
        assert_eq!(r.channel(), Channel::Sms);
    }

    #[test]
    fn webpush_recipient_routes_to_webpush_channel() {
        let r = Recipient::WebPush {
            subscription_json: "{}".into(),
        };
        assert_eq!(r.channel(), Channel::WebPush);
    }

    #[test]
    fn unregistered_channel_returns_none() {
        let reg = NotifyRegistry::new();
        let n = Notification {
            recipient: Recipient::Email {
                address: "x@example.com".into(),
            },
            subject: None,
            body: "hi".into(),
            metadata: HashMap::new(),
        };
        assert!(reg.dispatch(&n).is_none());
    }

    #[test]
    fn registered_dispatcher_handles_matching_channel() {
        let mut reg = NotifyRegistry::new();
        reg.register(Arc::new(MockDispatcher {
            channel: Channel::Email,
        }));
        let n = Notification {
            recipient: Recipient::Email {
                address: "x@example.com".into(),
            },
            subject: None,
            body: "hello".into(),
            metadata: HashMap::new(),
        };
        let outcome = reg.dispatch(&n).expect("dispatcher resolved");
        match outcome {
            DeliveryOutcome::Delivered {
                provider_message_id,
                ..
            } => assert_eq!(provider_message_id, "mock-id"),
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn registered_dispatcher_does_not_handle_other_channels() {
        let mut reg = NotifyRegistry::new();
        reg.register(Arc::new(MockDispatcher {
            channel: Channel::Email,
        }));
        let n = Notification {
            recipient: Recipient::Sms {
                phone: "+15551234567".into(),
            },
            subject: None,
            body: "hi".into(),
            metadata: HashMap::new(),
        };
        assert!(reg.dispatch(&n).is_none());
    }
}
