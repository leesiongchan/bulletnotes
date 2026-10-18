//! Own a supervised fixture process until it has been terminated and reaped.
use std::{
    io,
    process::{Child, ExitStatus},
};

pub(super) struct ChildGuard {
    child: Child,
    reaped: bool,
}
impl ChildGuard {
    pub(super) fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
        }
    }
    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        self.reaped |= status.is_some();
        Ok(status)
    }
    pub(super) fn terminate_and_reap(&mut self) -> io::Result<()> {
        if !self.reaped {
            // A kill may race normal exit; successful wait is the terminal authority.
            let _ = self.child.kill();
            loop {
                match self.child.wait() {
                    Ok(_) => {
                        self.reaped = true;
                        break;
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.terminate_and_reap();
    }
}
