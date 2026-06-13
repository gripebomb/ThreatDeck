use crate::config::TlsTrustStore;
use anyhow::{Context, Result};
use std::sync::Arc;

pub fn agent(tls_trust_store: TlsTrustStore) -> Result<ureq::Agent> {
    Ok(agent_builder(tls_trust_store)?.build())
}

pub fn agent_builder(tls_trust_store: TlsTrustStore) -> Result<ureq::AgentBuilder> {
    let mut builder = ureq::AgentBuilder::new();
    if tls_trust_store == TlsTrustStore::Os {
        let connector = native_tls::TlsConnector::new().context("creating native TLS connector")?;
        builder = builder.tls_connector(Arc::new(connector));
    }
    Ok(builder)
}
