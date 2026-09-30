//! concierge over gRPC: the three calls a relying party makes (spec §4). Redeeming a code
//! and rotating a refresh token authenticate with the panel's client secret; `GetMe`
//! with the user's access token, the only RPC that token opens.

use std::time::Duration;

use jiff::Timestamp;
use panel::session::{RefreshError, Refresher, Tokens};
use panel_contracts::concierge::v1::{
	ClientTokenResponse, ExchangeCodeRequest, GetMeRequest, RefreshClientTokenRequest, UserProfile, auth_service_client::AuthServiceClient, user_directory_client::UserDirectoryClient,
};
use tonic::{
	Code, Request, Status,
	metadata::MetadataValue,
	transport::{Channel, Endpoint},
};
use uuid::Uuid;
use zeroize::Zeroizing;

/// The panel's client id at concierge.
pub const CLIENT_ID: &str = "sa";

/// How long a call to concierge may take, connecting included.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// What a failed call to concierge means to the panel.
#[derive(Debug, thiserror::Error)]
pub enum ConciergeError {
	/// concierge answered no: UNAUTHENTICATED or PERMISSION_DENIED.
	#[error("concierge refused: {0:?}")]
	Refused(Code),
	/// concierge could not be reached, or did not answer in time.
	#[error("concierge is unavailable: {0}")]
	Unavailable(String),
	/// Anything else: a bug on one side or the other.
	#[error("concierge failed: {0}")]
	Failed(String),
}

impl From<Status> for ConciergeError {
	fn from(s: Status) -> Self {
		match s.code() {
			Code::Unauthenticated | Code::PermissionDenied => Self::Refused(s.code()),
			Code::Unavailable | Code::DeadlineExceeded | Code::Cancelled => Self::Unavailable(s.message().to_owned()),
			_ => Self::Failed(format!("{:?}: {}", s.code(), s.message())),
		}
	}
}

/// The signed-in user as concierge tells it (`GetMe`), what the panel needs of it.
#[derive(Clone, Debug)]
pub struct Me {
	pub user_id: Uuid,
	pub email: String,
	pub preferred_name: String,
	/// The platform role: investor, operator, admin, owner.
	pub role: String,
	/// Active scoped grants, `(scope, role)`.
	pub scopes: Vec<(String, String)>,
}

impl TryFrom<UserProfile> for Me {
	type Error = ConciergeError;

	fn try_from(p: UserProfile) -> Result<Self, ConciergeError> {
		Ok(Self {
			user_id: Uuid::parse_str(&p.user_id).map_err(|_| ConciergeError::Failed("GetMe answered a user id that is not a UUID".into()))?,
			email: p.email,
			preferred_name: p.preferred_name,
			role: p.role,
			scopes: p.scopes.into_iter().map(|g| (g.scope, g.role)).collect(),
		})
	}
}

/// A token pair and whose it is.
pub struct Issued {
	pub user_id: Uuid,
	pub tokens: Tokens,
}

#[derive(Clone)]
pub struct Concierge {
	auth: AuthServiceClient<Channel>,
	directory: UserDirectoryClient<Channel>,
	secret: Zeroizing<String>,
}

impl std::fmt::Debug for Concierge {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Concierge").finish_non_exhaustive()
	}
}

impl Concierge {
	/// Connects on first use, so the panel starts (and serves ingest) while concierge is down.
	pub fn new(grpc_addr: &str, client_secret: &str) -> eyre::Result<Self> {
		let channel = Endpoint::from_shared(grpc_addr.to_owned())
			.map_err(|e| eyre::eyre!("CONCIERGE_GRPC_ADDR {grpc_addr:?}: {e}"))?
			.connect_timeout(CALL_TIMEOUT)
			.timeout(CALL_TIMEOUT)
			.connect_lazy();
		Ok(Self {
			auth: AuthServiceClient::new(channel.clone()),
			directory: UserDirectoryClient::new(channel),
			secret: Zeroizing::new(client_secret.to_owned()),
		})
	}

	/// Redeems the code the browser brought back, with the verifier of the challenge sent to
	/// authorize.
	pub async fn exchange_code(&self, code: &str, redirect_uri: &str, verifier: &str) -> Result<Issued, ConciergeError> {
		let answer = self
			.auth
			.clone()
			.exchange_code(ExchangeCodeRequest {
				client_id: CLIENT_ID.to_owned(),
				client_secret: self.secret.to_string(),
				code: code.to_owned(),
				redirect_uri: redirect_uri.to_owned(),
				code_verifier: verifier.to_owned(),
			})
			.await?
			.into_inner();
		issued(answer)
	}

	/// The user behind an access token.
	pub async fn me(&self, access: &str) -> Result<Me, ConciergeError> {
		let mut req = Request::new(GetMeRequest {});
		let bearer: MetadataValue<_> = format!("Bearer {access}")
			.parse()
			.map_err(|_| ConciergeError::Failed("an access token that is not a header value".into()))?;
		req.metadata_mut().insert("authorization", bearer);
		self.directory.clone().get_me(req).await?.into_inner().try_into()
	}
}

impl Refresher for Concierge {
	async fn refresh(&self, refresh_token: &str) -> Result<Tokens, RefreshError> {
		let answer = self
			.auth
			.clone()
			.refresh_client_token(RefreshClientTokenRequest {
				client_id: CLIENT_ID.to_owned(),
				client_secret: self.secret.to_string(),
				refresh_token: refresh_token.to_owned(),
			})
			.await
			.map_err(ConciergeError::from)
			.and_then(|r| issued(r.into_inner()));
		match answer {
			Ok(issued) => Ok(issued.tokens),
			Err(ConciergeError::Refused(_)) => Err(RefreshError::Rejected),
			Err(ConciergeError::Unavailable(why)) => Err(RefreshError::Unavailable(why)),
			Err(e @ ConciergeError::Failed(_)) => Err(RefreshError::Failed(eyre::eyre!(e))),
		}
	}
}

fn issued(r: ClientTokenResponse) -> Result<Issued, ConciergeError> {
	let at = |secs: i64, what: &str| Timestamp::from_second(secs).map_err(|_| ConciergeError::Failed(format!("{what} out of range")));
	Ok(Issued {
		user_id: Uuid::parse_str(&r.user_id).map_err(|_| ConciergeError::Failed("a token pair for a user id that is not a UUID".into()))?,
		tokens: Tokens {
			access_expires_at: at(r.access_expires_at, "access_expires_at")?,
			refresh_expires_at: at(r.refresh_expires_at, "refresh_expires_at")?,
			access: Zeroizing::new(r.access_token),
			refresh: Zeroizing::new(r.refresh_token),
		},
	})
}
