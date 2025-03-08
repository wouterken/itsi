use std::os::fd::RawFd;

use itsi_error::{ItsiError, Result};
use mio::Interest;
use nix::libc::{fcntl, poll, pollfd, F_GETFL, F_SETFL, O_NONBLOCK};

use super::Readiness;

pub fn set_nonblocking(fd: RawFd) -> itsi_error::Result<()> {
    unsafe {
        let flags = fcntl(fd, F_GETFL);
        if flags < 0 {
            return Err(ItsiError::ArgumentError(format!(
                "fcntl(F_GETFL) error for fd {}: {}",
                fd,
                std::io::Error::last_os_error()
            )));
        }
        let new_flags = flags | O_NONBLOCK;
        if fcntl(fd, F_SETFL, new_flags) < 0 {
            return Err(ItsiError::ArgumentError(format!(
                "fcntl(F_SETFL) error for fd {}: {}",
                fd,
                std::io::Error::last_os_error()
            )));
        }
    }
    Ok(())
}

pub fn poll_readiness(fd: RawFd, events: i16) -> Option<Readiness> {
    let mut pfd = pollfd {
        fd,
        events,
        revents: 0,
    };
    let ret = unsafe { poll(&mut pfd as *mut pollfd, 1, 0) };
    if ret > 0 {
        return Some(Readiness(pfd.revents));
    }
    None
}

pub fn build_interest(events: i16) -> Result<Interest> {
    let mut interest_opt = None;
    if events & 1 != 0 {
        interest_opt = Some(Interest::READABLE);
    }
    if events & 4 != 0 {
        interest_opt = Some(match interest_opt {
            Some(i) => i | Interest::WRITABLE,
            None => Interest::WRITABLE,
        });
    }
    interest_opt.ok_or_else(|| ItsiError::ArgumentError("No valid event specified".to_owned()))
}
