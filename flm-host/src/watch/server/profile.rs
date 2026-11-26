//! The request to change the build type: `POST` of [`PROFILE`](super::PROFILE).

use std::io::Read;

use serde_json::Value;
use tiny_http::Request;

use super::Shared;
use super::http::reply;

/// The largest body, in bytes, of a request to [`PROFILE`](super::PROFILE); a longer body
/// is answered with `400`.
const PROFILE_BODY_LIMIT: u64 = 4 * 1024;

/// Answers a request to change the build type: passes the profile the body names to the
/// watch loop ([`Handle::set_profile`](crate::watch::Handle::set_profile)) and answers
/// `204`, or answers `400` for a body that names none. See the [module
/// documentation](super), "Changing the build type".
pub(super) fn set(shared: &Shared, mut request: Request) {
    let mut body = Vec::new();
    let read = request.as_reader().take(PROFILE_BODY_LIMIT + 1).read_to_end(&mut body);
    match read.ok().filter(|_| body.len() as u64 <= PROFILE_BODY_LIMIT).and_then(|_| profile_of(&body)) {
        Some(profile) => {
            shared.handle.set_profile(profile);
            reply(request, 204, None, None, b"");
        }
        None => {
            let reason = "The body must be a JSON object of at most 4 KiB with the name of a profile (‘profile’)";
            reply(request, 400, Some("text/plain"), None, reason.as_bytes());
        }
    }
}

/// The profile a request's body names, `{"profile": NAME}`, the name a text that is not
/// empty; `None` when the body is not that.
fn profile_of(body: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let profile = value.get("profile")?.as_str()?;
    (!profile.is_empty()).then(|| profile.to_owned())
}
