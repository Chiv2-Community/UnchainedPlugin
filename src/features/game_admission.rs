//! Registration-bound offline admission state. No network work occurs here.
use server_browser_client::models::GameAdmissionClaims;
use server_browser_client_sdk::admission::{AdmissionError, AdmissionVerifier};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Debug)]
struct Lease {
    server_id: String,
    verifier: Arc<AdmissionVerifier>,
    deadline: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct HostAdmissions {
    lease: Mutex<Option<Lease>>,
}

impl HostAdmissions {
    /// Publish confirmed SDK lease state, preserving replay protection when a
    /// response is lost or a paused/expired registration later renews its lease.
    pub fn publish(
        &self,
        issuer: &str,
        server_id: Option<&str>,
        deadline: Option<Instant>,
    ) -> Result<(), AdmissionError> {
        let mut lease = self
            .lease
            .lock()
            .map_err(|_| AdmissionError::KeysUnavailable)?;
        let Some(server_id) = server_id else {
            *lease = None;
            return Ok(());
        };
        if let Some(current) = lease
            .as_mut()
            .filter(|current| current.server_id == server_id)
        {
            current.deadline = deadline;
        } else {
            *lease = Some(Lease {
                server_id: server_id.to_owned(),
                verifier: Arc::new(AdmissionVerifier::new(issuer, server_id)?),
                deadline,
            });
        }
        Ok(())
    }

    /// Stop admitting immediately without forgetting consumed tokens.
    pub fn pause(&self) {
        if let Ok(mut lease) = self.lease.lock() {
            if let Some(lease) = lease.as_mut() {
                lease.deadline = None;
            }
        }
    }

    /// Background workers may refresh this snapshot; obsolete registrations'
    /// snapshots cannot replace the verifier for a newer registration.
    pub fn verifier_for_refresh(&self) -> Option<Arc<AdmissionVerifier>> {
        self.lease
            .lock()
            .ok()?
            .as_ref()
            .map(|lease| Arc::clone(&lease.verifier))
    }

    /// Verify and consume locally, only within the last confirmed lease.
    pub fn consume(&self, token: &str) -> Result<GameAdmissionClaims, AdmissionError> {
        let lease = self
            .lease
            .lock()
            .map_err(|_| AdmissionError::KeysUnavailable)?;
        let lease = lease
            .as_ref()
            .filter(|lease| {
                lease
                    .deadline
                    .is_some_and(|deadline| deadline > Instant::now())
            })
            .ok_or(AdmissionError::KeysUnavailable)?;
        lease.verifier.consume(token)
    }
}
