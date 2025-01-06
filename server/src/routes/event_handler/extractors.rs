use std::sync::Arc;

use axum::{
    extract::{FromRequest, FromRequestParts},
    http::{request::Parts, HeaderName},
};
use axum_core::{
    extract::{FromRef, Request},
    response::{IntoResponse, Response},
};
use hex::FromHexError;
use http_body_util::BodyExt;
use hyper::{header::ToStrError, StatusCode};
use octocrab::models::webhook_events::WebhookEvent;
use orion::hazardous::mac::hmac::sha256::{SecretKey, Tag};
use thiserror::Error;

pub struct ExtractSignatureHeader(pub(crate) Sha256VerificationSignature);

#[derive(Clone)]
pub(crate) struct Sha256VerificationSignature(Vec<u8>);

impl<'a> TryFrom<(&'a str, &'a str)> for Sha256VerificationSignature {
    type Error = SignatureHeaderError;

    fn try_from((kind, hmac): (&'a str, &'a str)) -> Result<Self, Self::Error> {
        match kind {
            "sha256" => Ok(Sha256VerificationSignature(hex::decode(hmac)?)),
            _ => Err(SignatureHeaderError::MissingHeader),
        }
    }
}

impl PartialEq<Tag> for &Sha256VerificationSignature {
    fn eq(&self, other: &Tag) -> bool {
        *other == &*self.0
    }
}

impl<S> FromRequestParts<S> for ExtractSignatureHeader
where
    S: Send + Sync,
{
    type Rejection = SignatureHeaderError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        static HEADER: HeaderName = HeaderName::from_static("x-hub-signature-256");
        let Some(signature) = parts.headers.get(&HEADER) else {
            return Err(SignatureHeaderError::MissingHeader);
        };
        let (kind, hmac) = signature
            .to_str()?
            .split_once('=')
            .ok_or(SignatureHeaderError::NotAPair)?;
        Ok(Self((kind, hmac).try_into()?))
    }
}

#[derive(Debug, Error)]
pub enum SignatureHeaderError {
    #[error("The header value does not consist of a valid string")]
    InvalidValue(#[from] ToStrError),
    #[error("The header value is missing the correct delimiter")]
    NotAPair,
    #[error("The header value is not a valid hex value")]
    NotHex(#[from] FromHexError),
    #[error("Missing header pair (either left or right side)")]
    MissingHeader,
}

impl IntoResponse for SignatureHeaderError {
    fn into_response(self) -> Response {
        match self {
            e @ SignatureHeaderError::InvalidValue(_) => (StatusCode::BAD_REQUEST, e.to_string()),
            e @ SignatureHeaderError::NotAPair => (StatusCode::BAD_REQUEST, e.to_string()),
            e @ SignatureHeaderError::NotHex(_) => (StatusCode::BAD_REQUEST, e.to_string()),
            e @ SignatureHeaderError::MissingHeader => (StatusCode::BAD_REQUEST, e.to_string()),
        }
        .into_response()
    }
}

pub(crate) struct ExtractGitHubEventHeader(String);

impl<S> FromRequestParts<S> for ExtractGitHubEventHeader
where
    S: Send + Sync,
{
    type Rejection = GitHubEventHeaderError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        static HEADER: HeaderName = HeaderName::from_static("x-github-event");
        let Some(signature) = parts.headers.get(&HEADER) else {
            return Err(GitHubEventHeaderError::MissingHeader);
        };
        Ok(Self(signature.to_str()?.to_owned()))
    }
}

#[derive(Debug, Error)]
pub enum GitHubEventHeaderError {
    #[error("Missing header")]
    MissingHeader,
    #[error("The header value does not consist of a valid string")]
    InvalidValue(#[from] ToStrError),
}

impl IntoResponse for GitHubEventHeaderError {
    fn into_response(self) -> Response {
        match self {
            e @ GitHubEventHeaderError::MissingHeader => (StatusCode::BAD_REQUEST, e.to_string()),
            e @ GitHubEventHeaderError::InvalidValue(_) => (StatusCode::BAD_REQUEST, e.to_string()),
        }
        .into_response()
    }
}

pub(crate) struct GitHubEvent(pub(crate) WebhookEvent);

impl<S> FromRequest<S> for GitHubEvent
where
    S: Send + Sync,
    Arc<SecretKey>: FromRef<S>,
{
    type Rejection = GitHubEventExtractionError;

    async fn from_request(request: Request, webhook_secret: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = request.into_parts();
        let webhook_secret = Arc::<SecretKey>::from_ref(webhook_secret);

        let ExtractGitHubEventHeader(event) =
            ExtractGitHubEventHeader::from_request_parts(&mut parts, &()).await?;
        let ExtractSignatureHeader(signature) =
            ExtractSignatureHeader::from_request_parts(&mut parts, &()).await?;

        let body = body.collect().await?.to_bytes();

        verify_signature(&signature, &webhook_secret, &body)?;
        Ok(Self(
            WebhookEvent::try_from_header_and_body(&event, &body)
                .map_err(GitHubEventExtractionError::EventUnparsable)?,
        ))
    }
}

fn verify_signature(
    signature: &Sha256VerificationSignature,
    webhook_secret: &SecretKey,
    body: &[u8],
) -> Result<(), GitHubEventExtractionError> {
    use orion::hazardous::mac::hmac::sha256::HmacSha256;
    let tag = HmacSha256::hmac(webhook_secret, body)
        .map_err(|_| GitHubEventExtractionError::InvalidSignature)?;
    if signature == tag {
        Ok(())
    } else {
        Err(GitHubEventExtractionError::SignatureMismatch)
    }
}

#[derive(Debug, Error)]
pub enum GitHubEventExtractionError {
    #[error("The header value does not consist of a valid string")]
    InvalidValue(#[from] ToStrError),
    #[error("Unable to calculate the signature of the body")]
    InvalidSignature,
    #[error("Signature of body does not match the header")]
    SignatureMismatch,
    #[error("Unable to verify the signature: {0}")]
    SignatureHeader(#[from] SignatureHeaderError),
    #[error("Unable to fetch the event name: {0}")]
    GitHubHeader(#[from] GitHubEventHeaderError),
    #[error("Unable to parse and process the request")]
    EventUnparsable(serde_json::Error),
    #[error("Something went wrong whilst processing the body")]
    AxumError(#[from] axum::Error),
}

impl IntoResponse for GitHubEventExtractionError {
    fn into_response(self) -> Response {
        match self {
            e @ GitHubEventExtractionError::InvalidValue(_) => {
                (StatusCode::BAD_REQUEST, e.to_string())
            }
            e @ GitHubEventExtractionError::InvalidSignature => {
                (StatusCode::BAD_REQUEST, e.to_string())
            }
            e @ GitHubEventExtractionError::SignatureMismatch => {
                (StatusCode::BAD_REQUEST, e.to_string())
            }
            e @ GitHubEventExtractionError::SignatureHeader(_) => {
                (StatusCode::BAD_REQUEST, e.to_string())
            }
            e @ GitHubEventExtractionError::EventUnparsable(_) => {
                (StatusCode::BAD_REQUEST, e.to_string())
            }
            e @ GitHubEventExtractionError::AxumError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
            e @ GitHubEventExtractionError::GitHubHeader(_) => {
                (StatusCode::BAD_REQUEST, e.to_string())
            }
        }
        .into_response()
    }
}
