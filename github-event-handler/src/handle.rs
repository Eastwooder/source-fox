use crate::api::GitHubApi;
use crate::authentication::InstallationAuthenticator;
use octocrab::models::webhook_events::{
    EventInstallation, WebhookEvent, WebhookEventPayload, WebhookEventType,
};
use snafu::{Backtrace, ResultExt, Snafu};

pub async fn handle_event<C>(
    app_client: C,
    event: WebhookEvent,
) -> Result<Option<String>, HandleEventError>
where
    C: InstallationAuthenticator,
{
    let id = match event.installation {
        Some(EventInstallation::Full(ref installation)) => installation.id,
        Some(EventInstallation::Minimal(ref installation)) => installation.id,
        None if event.kind == WebhookEventType::Ping => return Ok(Some("pong".to_string())),
        None => return MissingInstallationSnafu.fail(),
    };
    let api_client = app_client
        .for_installation(id)
        .await
        .map_err(|err| Box::new(err) as _)
        .context(InstallationAuthenticationSnafu)?;
    match event.specific {
        WebhookEventPayload::Ping(ping) => Ok(ping.zen),
        WebhookEventPayload::PullRequest(pr) => {
            let Some(repository) = event.repository else {
                return MissingRepositorySnafu.fail();
            };
            let sha = &pr.pull_request.head.sha;
            api_client
                .create_commit_status(&repository, sha)
                .await
                .map_err(|err| Box::new(err) as _)
                .context(EventHandlingSnafu { event: event.kind })?;
            Ok(None)
        }
        WebhookEventPayload::Push(_) => Ok(None),
        WebhookEventPayload::CheckRun(_) => Ok(None),
        WebhookEventPayload::CheckSuite(check) => {
            let Some(_repository) = event.repository else {
                return MissingRepositorySnafu.fail();
            };
            // TODO: need to parse check.check_suite or check.enterprise as it's currently just a json object
            tracing::debug!(check_suite = ?check, "handling check suite");
            Ok(None)
        }
        _ => {
            tracing::debug!(kind = ?event.kind, "unhandled event");
            Ok(None)
        }
    }
}

#[derive(Debug, Snafu)]
pub enum HandleEventError {
    #[snafu(display("Missing installation in the event"))]
    MissingInstallation,
    #[snafu(display("Unable to authenticate installation"))]
    InstallationAuthentication {
        source: Box<dyn std::error::Error>,
        backtrace: Backtrace,
    },
    #[snafu(display("Missing repository in the event"))]
    MissingRepository,
    #[snafu(display("Failed to handle event: {:?}", event))]
    EventHandling {
        event: WebhookEventType,
        source: Box<dyn std::error::Error>,
        backtrace: Backtrace,
    },
}
