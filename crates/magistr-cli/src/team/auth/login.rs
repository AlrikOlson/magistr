//! Drive a device-code login to completion.

use std::time::Duration;

use crate::error::MagistrError;

use super::device_authorization::DeviceAuthorization;
use super::device_flow::DeviceFlow;
use super::poll_schedule::PollSchedule;
use super::token_poll::TokenPoll;
use super::verified_identity::VerifiedIdentity;

/// Run the device authorization grant against `flow`.
///
/// `prompt` is called exactly once, with the code the human has to type
/// and where to type it; `sleep` is called before each poll. Both are
/// injected so the loop itself stays a pure function of the provider's
/// answers — the tests pass a recorder and a no-op and never wait.
///
/// The loop ends on a grant, on refusal, or when the device code's own
/// lifetime is spent. It cannot spin: an unrecognised provider error is
/// a hard failure inside [`TokenPoll::from_json`], not another retry.
pub fn login(
    flow: &dyn DeviceFlow,
    prompt: &mut dyn FnMut(&DeviceAuthorization),
    sleep: &mut dyn FnMut(Duration),
) -> Result<VerifiedIdentity, MagistrError> {
    let auth = flow.start()?;
    prompt(&auth);

    let mut schedule = PollSchedule::new(&auth);
    loop {
        if !schedule.charge() {
            return Err(expired(auth.expires_in));
        }
        sleep(schedule.interval());
        match flow.poll(&auth.device_code)? {
            TokenPoll::Pending => {}
            TokenPoll::SlowDown => schedule.slow_down(),
            TokenPoll::Denied => {
                return Err(MagistrError::Permission(
                    "the login was denied at the provider — nothing was cached".to_string(),
                ));
            }
            TokenPoll::Expired => return Err(expired(auth.expires_in)),
            TokenPoll::Granted(token) => return flow.identify(&token),
        }
    }
}

fn expired(lifetime: u64) -> MagistrError {
    MagistrError::Other(format!(
        "the device code expired after {lifetime}s without an answer — run `magistr login` again"
    ))
}

#[cfg(test)]
#[path = "login.tests.rs"]
mod tests;
