use std::ffi::c_int;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::ptr;
use std::thread::{self, JoinHandle};

pub(super) struct OutputPipes {
    stdout: Pipe,
    stderr: Pipe,
    actions: FileActions,
}

impl OutputPipes {
    pub(super) fn new() -> io::Result<Self> {
        let stdout = Pipe::new()?;
        let stderr = Pipe::new()?;
        let mut actions = FileActions::new()?;
        actions.redirect(&stdout, libc::STDOUT_FILENO)?;
        actions.redirect(&stderr, libc::STDERR_FILENO)?;
        Ok(Self { stdout, stderr, actions })
    }

    /// Starts both readers while this process is still the only holder of the write ends. A
    /// reader that cannot start therefore fails with no child to leave running, and with nothing
    /// to release but this process's own descriptors and the reader that did start.
    pub(super) fn readers(self) -> io::Result<Readers> {
        let Self { stdout, stderr, actions } = self;
        let writers = Writers { _stdout: stdout.write, _stderr: stderr.write, actions };
        let first = reader(stdout.read)?;
        match reader(stderr.read) {
            Ok(second) => Ok(Readers { writers, stdout: first, stderr: second }),
            Err(error) => {
                // With no child, this process holds the only write ends, so closing them is what
                // ends the reader that did start and joining it cannot block.
                drop(writers);
                let _ = first.join();
                Err(error)
            }
        }
    }
}

/// This process's copies of the write ends, with the file actions that give them to the child.
/// Both outlive the spawn that hands them over, and are closed immediately afterwards.
struct Writers {
    _stdout: OwnedFd,
    _stderr: OwnedFd,
    actions: FileActions,
}

/// Both child output readers, draining concurrently from before the child exists until its
/// streams close.
pub(super) struct Readers {
    writers: Writers,
    stdout: JoinHandle<io::Result<Vec<u8>>>,
    stderr: JoinHandle<io::Result<Vec<u8>>>,
}

impl Readers {
    pub(super) fn actions(&self) -> &libc::posix_spawn_file_actions_t {
        &self.writers.actions.0
    }

    /// The child owns the write ends now, so closing this process's copies is what lets each
    /// reader see end of file once the child's streams close.
    pub(super) fn drain(self) -> Draining {
        let Self { writers, stdout, stderr } = self;
        drop(writers);
        Draining { stdout, stderr }
    }

    /// Ends both readers when no child took the write ends. There is nothing left to collect.
    pub(super) fn release(self) {
        let Self { writers, stdout, stderr } = self;
        drop(writers);
        let _ = stdout.join();
        let _ = stderr.join();
    }
}

struct Pipe {
    read: OwnedFd,
    write: OwnedFd,
}

impl Pipe {
    fn new() -> io::Result<Self> {
        let mut descriptors = [-1; 2];
        // SAFETY: the live array receives two newly owned descriptors on success.
        if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a successful pipe call returned ownership of both descriptors.
        let (read, write) =
            unsafe { (OwnedFd::from_raw_fd(descriptors[0]), OwnedFd::from_raw_fd(descriptors[1])) };
        Ok(Self { read: close_on_exec(read)?, write: close_on_exec(write)? })
    }
}

fn close_on_exec(descriptor: OwnedFd) -> io::Result<OwnedFd> {
    let raw = descriptor.as_raw_fd();
    if raw <= libc::STDERR_FILENO {
        // Keep file-action close operations distinct from the standard descriptors they replace.
        // SAFETY: fcntl duplicates the live descriptor into the requested range.
        let duplicated = unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicated < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fcntl returned a newly owned descriptor.
        return Ok(unsafe { OwnedFd::from_raw_fd(duplicated) });
    }
    // SAFETY: a new pipe descriptor has no descriptor flags to preserve.
    if unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(descriptor)
}

struct FileActions(libc::posix_spawn_file_actions_t);

impl FileActions {
    fn new() -> io::Result<Self> {
        let mut actions = ptr::null_mut();
        // SAFETY: actions is a live out parameter, owned by the returned guard on success.
        checked(unsafe { libc::posix_spawn_file_actions_init(&mut actions) })?;
        Ok(Self(actions))
    }

    fn redirect(&mut self, pipe: &Pipe, standard: c_int) -> io::Result<()> {
        // SAFETY: the initialized actions and both live descriptors outlast the spawn.
        unsafe {
            checked(libc::posix_spawn_file_actions_adddup2(
                &mut self.0,
                pipe.write.as_raw_fd(),
                standard,
            ))?;
            checked(libc::posix_spawn_file_actions_addclose(&mut self.0, pipe.read.as_raw_fd()))?;
            checked(libc::posix_spawn_file_actions_addclose(&mut self.0, pipe.write.as_raw_fd()))
        }
    }
}

impl Drop for FileActions {
    fn drop(&mut self) {
        // SAFETY: these actions were initialized successfully and are no longer used.
        unsafe { libc::posix_spawn_file_actions_destroy(&mut self.0) };
    }
}

fn checked(status: c_int) -> io::Result<()> {
    if status == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(status)) }
}

/// Starts one reader on one pipe, which reads until the last write end is closed.
fn reader(descriptor: OwnedFd) -> io::Result<JoinHandle<io::Result<Vec<u8>>>> {
    thread::Builder::new().spawn(move || {
        let mut bytes = Vec::new();
        File::from(descriptor).read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

pub(super) struct Draining {
    stdout: JoinHandle<io::Result<Vec<u8>>>,
    stderr: JoinHandle<io::Result<Vec<u8>>>,
}

impl Draining {
    pub(super) fn collect(self) -> io::Result<Captured> {
        Ok(Captured { stdout: collected(self.stdout)?, stderr: collected(self.stderr)? })
    }
}

fn collected(reader: JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    reader.join().map_err(|_| io::Error::other("a child output reader panicked"))?
}

pub(super) struct Captured {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Captured {
    pub(super) fn forward(self) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&self.stdout)?;
        stdout.flush()?;
        let mut stderr = io::stderr().lock();
        stderr.write_all(&self.stderr)?;
        stderr.flush()
    }
}
