use octocrab::models::webhook_events::{WebhookEvent, WebhookEventPayload, WebhookEventType};
use octocrab::models::{CheckRunId, Repository};
use octocrab::params::checks::{
    CheckRunConclusion, CheckRunOutput, CheckRunOutputAnnotation, CheckRunOutputAnnotationLevel,
    CheckRunStatus,
};
use octocrab::Octocrab;
use snafu::Backtrace;
use snafu::{ResultExt, Snafu};
use std::fmt::Debug;
use std::future::Future;
use tracing::instrument;

pub async fn handle_event<A>(
    action: A,
    event: WebhookEvent,
) -> Result<Option<String>, HandleEventError>
where
    A: GitHubApi,
{
    match event.specific {
        WebhookEventPayload::Ping(ping) => Ok(ping.zen),
        WebhookEventPayload::PullRequest(pr) => {
            let Some(repository) = event.repository else {
                return MissingRepositorySnafu.fail();
            };
            let sha = &pr.pull_request.head.sha;
            action
                .create_commit_status(&repository, sha)
                .await
                .map_err(|err| Box::new(err) as _)
                .context(EventHandlingSnafu { event: event.kind })?;
            Ok(None)
        }
        WebhookEventPayload::Push(_) => Ok(None),
        WebhookEventPayload::CheckRun(_) => Ok(None),
        _ => {
            tracing::debug!(kind = ?event.kind, "unhandled event");
            Ok(None)
        }
    }
}

#[derive(Debug, Snafu)]
pub enum HandleEventError {
    #[snafu(display("Missing repository in the event"))]
    MissingRepository,
    #[snafu(display("Failed to handle event: {:?}", event))]
    EventHandling {
        event: WebhookEventType,
        source: Box<dyn std::error::Error>,
        backtrace: Backtrace,
    },
}

pub trait GitHubApi: Send {
    fn create_commit_status(
        &self,
        repository: &Repository,
        sha: &str,
    ) -> impl Future<Output = Result<impl Debug, impl std::error::Error + Send + Sync + 'static>> + Send;
}

impl GitHubApi for Octocrab {
    #[allow(refining_impl_trait)]
    #[instrument(skip(self, repository), fields(repo = %repository.name), ret)]
    async fn create_commit_status(
        &self,
        repository: &Repository,
        sha: &str,
    ) -> Result<CheckRunId, GitHubActionError> {
        let Some(owner) = repository.clone().owner else {
            return MissingOwnerSnafu.fail();
        };
        self.checks(owner.login.to_owned(), repository.name.to_owned())
            .create_check_run("my-check", sha)
            .details_url("https://54aa-91-118-110-130.ngrok-free.app/1234")
            .external_id("1234")
            .status(CheckRunStatus::Completed)
            .conclusion(CheckRunConclusion::Success)
            .output(CheckRunOutput {
                title: "my title".to_string(),
                summary: indoc::indoc! {"
                    this **worked** right?
                "}
                .to_string(),
                text: Some(
                    indoc::indoc! {"
                    # github yada

                    > [!CAUTION]
                    > Advises about risks or negative outcomes of certain actions.
                "}
                    .to_string(),
                ),
                annotations: vec![CheckRunOutputAnnotation {
                    path: "Cargo.toml".to_string(),
                    start_line: 5,
                    end_line: 5,
                    start_column: Some(8),
                    end_column: Some(12),
                    annotation_level: CheckRunOutputAnnotationLevel::Warning,
                    message: "Is this **markdown**? insert meme here".to_string(),
                    title: Some("invalid rule".into()),
                    raw_details: Some("`yada` yada?".into()),
                }],
                images: vec![],
            })
            .send()
            .await
            .context(OctocrabSnafu)
            .map(|s| s.id)
    }
}

#[derive(Debug, Snafu)]
pub enum GitHubActionError {
    #[snafu(display("Missing owner!"))]
    MissingOwner { backtrace: Backtrace },
    #[snafu(display("Something went horribly wrong: {}", source))]
    Octocrab {
        source: octocrab::Error,
        backtrace: Backtrace,
    },
}
