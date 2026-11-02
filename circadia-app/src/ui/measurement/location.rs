use super::GraphMeasurement;
use super::Shared;
use crate::Coordinates;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::ui::settings::location::Completed;
use crate::ui::settings::location::Locate;
use std::any::Any;
use std::io;
use std::sync::Arc;

/// Count simulated location calls during a desktop check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocationMeasurement {
    /// Number of silent permission checks.
    pub permission_checks: usize,
    /// Number of foreground consent requests.
    pub consent_requests: usize,
    /// Number of position acquisitions.
    pub position_requests: usize,
}

pub(super) struct Simulation {
    permission: Result<(), DiagnosticFailure>,
    calls: LocationMeasurement,
    access: Option<Completed<()>>,
    position: Option<Completed<Coordinates>>,
}

pub(super) struct Source(pub(super) Arc<Shared>);

impl GraphMeasurement {
    /// Replace Windows location with controlled permission and completion.
    ///
    /// - Call before starting the application.
    ///
    /// # Errors
    /// Report poisoned measurement synchronization.
    pub fn simulate_location(&self, permission: Result<(), DiagnosticFailure>) -> io::Result<()> {
        let mut state = self
            .0
            .location
            .lock()
            .map_err(|_error| io::Error::other("location simulation lock failed"))?;
        *state = Some(Simulation {
            permission,
            calls: LocationMeasurement::default(),
            access: None,
            position: None,
        });
        Ok(())
    }

    /// Read simulated location call counts.
    ///
    /// # Errors
    /// Report unavailable simulation or poisoned synchronization.
    pub fn location_calls(&self) -> io::Result<LocationMeasurement> {
        let state = self
            .0
            .location
            .lock()
            .map_err(|_error| io::Error::other("location simulation lock failed"))?;
        state
            .as_ref()
            .map(|state| state.calls)
            .ok_or_else(|| io::Error::other("location simulation is not enabled"))
    }

    /// Complete the pending foreground permission request.
    ///
    /// # Errors
    /// Report a missing request or poisoned synchronization.
    pub fn complete_location_access(
        &self,
        result: Result<(), DiagnosticFailure>,
    ) -> io::Result<()> {
        let completed = {
            let mut state = self
                .0
                .location
                .lock()
                .map_err(|_error| io::Error::other("location simulation lock failed"))?;
            let state = state
                .as_mut()
                .ok_or_else(|| io::Error::other("location simulation is not enabled"))?;
            state.permission = result;
            state
                .access
                .take()
                .ok_or_else(|| io::Error::other("location consent is not pending"))?
        };
        completed(result);
        Ok(())
    }

    /// Complete the pending position acquisition.
    ///
    /// # Errors
    /// Report a missing request or poisoned synchronization.
    pub fn complete_location(
        &self,
        result: Result<Coordinates, DiagnosticFailure>,
    ) -> io::Result<()> {
        let completed = {
            let mut state = self
                .0
                .location
                .lock()
                .map_err(|_error| io::Error::other("location simulation lock failed"))?;
            state
                .as_mut()
                .and_then(|state| state.position.take())
                .ok_or_else(|| io::Error::other("location acquisition is not pending"))?
        };
        completed(result);
        Ok(())
    }
}

impl Locate for Source {
    fn permission(&self) -> Result<(), DiagnosticFailure> {
        let mut state = self.0.location.lock().map_err(|_error| unavailable())?;
        let state = state.as_mut().ok_or_else(unavailable)?;
        state.calls.permission_checks += 1;
        state.permission
    }

    fn access(&self, completed: Completed<()>) -> Result<Option<Box<dyn Any>>, DiagnosticFailure> {
        let mut state = self.0.location.lock().map_err(|_error| unavailable())?;
        let state = state.as_mut().ok_or_else(unavailable)?;
        state.calls.consent_requests += 1;
        state.access = Some(completed);
        Ok(Some(Box::new(())))
    }

    fn position(
        &self,
        completed: Completed<Coordinates>,
    ) -> Result<Box<dyn Any>, DiagnosticFailure> {
        let mut state = self.0.location.lock().map_err(|_error| unavailable())?;
        let state = state.as_mut().ok_or_else(unavailable)?;
        state.calls.position_requests += 1;
        state.position = Some(completed);
        Ok(Box::new(()))
    }
}

fn unavailable() -> DiagnosticFailure {
    DiagnosticFailure {
        category: DiagnosticCategory::Unavailable,
        code: None,
    }
}
