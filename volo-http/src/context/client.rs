//! Context and its utilities of client

use std::time::Duration;

use chrono::{DateTime, Local};
use faststr::FastStr;
use http::uri::Scheme;
use volo::{
    context::{Reusable, Role, RpcCx, RpcInfo},
    newtype_impl_context,
};

use crate::utils::macros::{impl_deref_and_deref_mut, stat_impl};

/// RPC context of http client
#[derive(Debug)]
pub struct ClientContext(pub(crate) RpcCx<ClientCxInner, Config>);

impl ClientContext {
    /// Create a new [`ClientContext`]
    pub fn new() -> Self {
        Self(RpcCx::new(
            RpcInfo::<Config>::with_role(Role::Client),
            ClientCxInner {
                scheme: Scheme::HTTP,
                hostname: FastStr::empty(),
                stats: ClientStats::default(),
            },
        ))
    }
}

impl Default for ClientContext {
    fn default() -> Self {
        Self::new()
    }
}

newtype_impl_context!(ClientContext, Config, 0);

impl_deref_and_deref_mut!(ClientContext, RpcCx<ClientCxInner, Config>, 0);

/// Inner details of [`ClientContext`]
#[derive(Debug)]
pub struct ClientCxInner {
    // Scheme of the request.
    scheme: Scheme,
    // Hostname of the request.
    //
    // It will be set as the header `Host` and used as SNI for the HTTPS handshake.
    //
    // Since the target domain name and hostname may be different (e.g. accessing
    // `server.example.com` via `proxy.example.com`), we should add an extra field to save the
    // hostname.
    //
    // And since service discovery (DNS) can only access the `Endpoint` instead of `ClientContext`,
    // so we should save the domain name in callee and save hostname here.
    hostname: FastStr,

    /// Statistics of client
    ///
    /// This is unstable now and may be changed in the future.
    pub stats: ClientStats,
}

impl ClientCxInner {
    /// Get [`Scheme`] of current request context.
    pub fn scheme(&self) -> &Scheme {
        &self.scheme
    }

    pub(crate) fn set_scheme(&mut self, scheme: Scheme) {
        self.scheme = scheme;
    }

    /// Get host name of current request context.
    pub fn hostname(&self) -> &FastStr {
        &self.hostname
    }

    pub(crate) fn set_hostname(&mut self, hostname: FastStr) {
        self.hostname = hostname;
    }
}

/// Statistics of client
///
/// This is unstable now and may be changed in the future.
#[derive(Debug, Default, Clone)]
pub struct ClientStats {
    transport_start_at: Option<DateTime<Local>>,
    transport_end_at: Option<DateTime<Local>>,
}

impl ClientStats {
    stat_impl!(transport_start_at);
    stat_impl!(transport_end_at);
}

/// Configuration of the request
#[derive(Clone, Debug, Default)]
pub struct Config {
    /// Timeout of the current request
    pub timeout: Option<Duration>,
}

impl Config {
    /// Create a default [`Config`]
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Get current timeout of the request
    #[inline]
    pub fn timeout(&self) -> Option<&Duration> {
        self.timeout.as_ref()
    }

    /// Set timeout to the request
    #[inline]
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }
}

impl Reusable for Config {
    fn clear(&mut self) {
        self.timeout = None;
    }
}
