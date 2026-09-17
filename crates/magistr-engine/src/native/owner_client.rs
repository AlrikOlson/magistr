//! The minimal local application client: a surface acting through an
//! owner it did not open.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::attach_error::AttachError;
use super::attach_kind::AttachKind;
use super::build_identity::BuildIdentity;
use super::caller_identity::CallerIdentity;
use super::native_config::NativeConfig;
use super::owner_command::OwnerCommand;
use super::owner_generation::OwnerGeneration;
use super::owner_identity::OwnerIdentity;
use super::owner_reply::OwnerReply;
use super::owner_socket::owner_socket_path;
use super::owner_wire::{ClientFrame, CommandRefusal, Hello, OWNER_PROTOCOL_VERSION};
use super::writer_role::WriterRole;

/// A synchronous client on an [`OwnerEndpoint`](super::OwnerEndpoint):
/// one connection, one command in flight, every round trip bounded by a
/// deadline. What a second cockpit's engine port and a one-shot command
/// hold instead of a writer.
///
/// The client never admits anything. It finds the endpoint by the same
/// derivation the owner used ([`owner_socket_path`]), sends its `Hello`,
/// and is welcomed with the owner's [`OwnerGeneration`] — which it then
/// carries on every command. When the owner hangs up ([`AttachError::OwnerGone`])
/// or does not answer in time ([`AttachError::Deadline`]) the connection is
/// dropped and the client is detached until [`Self::reconnect`], whose
/// answer says whether the owner is still the one this client was reading
/// from. A [`AttachError::StaleGeneration`] means it is not: re-read
/// before acting.
///
/// Synchronous on purpose: the cockpit's engine port is synchronous inside
/// its session task, and a local round trip is bounded, not awaited.
#[derive(Debug)]
pub struct OwnerClient {
    path: PathBuf,
    hello: Hello,
    deadline: Duration,
    link: Option<unix::Link>,
    generation: OwnerGeneration,
    owner_pid: u32,
    next_id: u64,
}

impl OwnerClient {
    /// How long a command may take before the connection is given up.
    pub const DEADLINE: Duration = Duration::from_secs(5);

    /// Attach for typed commands to the owner of `role` on the project
    /// `project_id` and the store `config` names, with the default
    /// deadline.
    ///
    /// # Errors
    /// As [`Self::attach_with`].
    pub fn attach(
        config: &NativeConfig,
        project_id: impl Into<String>,
        role: WriterRole,
        caller: CallerIdentity,
    ) -> Result<Self, AttachError> {
        Self::attach_with(config, project_id.into(), role, caller, Self::DEADLINE)
    }

    /// [`Self::attach`] on the project at `config.project_root`, its
    /// identity resolved the way the owner resolved it.
    ///
    /// # Errors
    /// As [`Self::attach_with`].
    pub fn attach_project(
        config: &NativeConfig,
        role: WriterRole,
        caller: CallerIdentity,
    ) -> Result<Self, AttachError> {
        Self::attach(config, config.resolve_project_id(), role, caller)
    }

    /// Attach for typed commands with an explicit `deadline`.
    ///
    /// # Errors
    /// [`AttachError::Unavailable`] when nothing listens at the endpoint,
    /// [`AttachError::Refused`] with the owner's reason, [`AttachError::Deadline`]
    /// when the handshake is not answered in time, [`AttachError::Unsupported`]
    /// off Unix.
    pub fn attach_with(
        config: &NativeConfig,
        project_id: String,
        role: WriterRole,
        caller: CallerIdentity,
        deadline: Duration,
    ) -> Result<Self, AttachError> {
        let path = owner_socket_path(config, &project_id, role);
        let hello = Hello {
            protocol: OWNER_PROTOCOL_VERSION,
            build: BuildIdentity::this_build(),
            identity: OwnerIdentity::Workflow { project_id, role },
            attach: AttachKind::Commands,
            caller,
        };
        let (link, generation, owner_pid) = unix::connect(&path, &hello, deadline)?;
        Ok(Self {
            path,
            hello,
            deadline,
            link: Some(link),
            generation,
            owner_pid,
            next_id: 1,
        })
    }

    /// Attach for the owner's MCP session: the handshake is done here, and
    /// the socket — positioned exactly after the welcome — is handed back
    /// for the caller to speak MCP on, with no timeouts of its own.
    ///
    /// # Errors
    /// As [`Self::attach_with`]; an owner without an MCP session refuses
    /// with [`AttachRefusal::Unsupported`](super::AttachRefusal::Unsupported).
    #[cfg(unix)]
    pub fn attach_mcp(
        config: &NativeConfig,
        project_id: String,
        role: WriterRole,
        caller: CallerIdentity,
        deadline: Duration,
    ) -> Result<(OwnerGeneration, std::os::unix::net::UnixStream), AttachError> {
        let path = owner_socket_path(config, &project_id, role);
        let hello = Hello {
            protocol: OWNER_PROTOCOL_VERSION,
            build: BuildIdentity::this_build(),
            identity: OwnerIdentity::Workflow { project_id, role },
            attach: AttachKind::Mcp,
            caller,
        };
        Self::attach_mcp_at(&path, &hello, deadline)
    }

    /// Attach to the MCP session of an owner of the canonical workspace root.
    /// This opens no workspace store and takes no workflow or workspace lease.
    /// The returned generation names the owner reached by this connection.
    ///
    /// # Errors
    /// As [`Self::attach_with`], plus canonical-root resolution failures.
    #[cfg(unix)]
    pub async fn attach_workspace(
        root: &Path,
        caller: CallerIdentity,
        deadline: Duration,
    ) -> Result<(OwnerGeneration, tokio::net::UnixStream), AttachError> {
        super::owner_workspace_link::attach(root, caller, deadline).await
    }

    #[cfg(unix)]
    fn attach_mcp_at(
        path: &Path,
        hello: &Hello,
        deadline: Duration,
    ) -> Result<(OwnerGeneration, std::os::unix::net::UnixStream), AttachError> {
        let (link, generation, _) = unix::connect(path, hello, deadline)?;
        let stream = link.into_stream();
        stream
            .set_read_timeout(None)
            .and_then(|()| stream.set_write_timeout(None))
            .map_err(AttachError::Unavailable)?;
        Ok((generation, stream))
    }

    /// The socket this client speaks to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The generation this client was welcomed under.
    #[must_use]
    pub const fn generation(&self) -> OwnerGeneration {
        self.generation
    }

    /// The owner process, as it announced itself.
    #[must_use]
    pub const fn owner_pid(&self) -> u32 {
        self.owner_pid
    }

    /// Whether a connection is up. `false` after a deadline, a hang-up or
    /// [`Self::detach`], until [`Self::reconnect`].
    #[must_use]
    pub const fn is_attached(&self) -> bool {
        self.link.is_some()
    }

    /// Connect again and redo the handshake, handing back the generation
    /// the owner now has — the same one when the owner survived, a newer
    /// one when the project changed hands, in which case what this client
    /// last read is stale.
    ///
    /// # Errors
    /// As [`Self::attach_with`].
    pub fn reconnect(&mut self) -> Result<OwnerGeneration, AttachError> {
        self.link = None;
        let (link, generation, owner_pid) = unix::connect(&self.path, &self.hello, self.deadline)?;
        self.link = Some(link);
        self.generation = generation;
        self.owner_pid = owner_pid;
        Ok(generation)
    }

    /// Ask the owner to carry `command` out under the welcomed generation.
    ///
    /// # Errors
    /// [`AttachError::Detached`] when not attached; [`AttachError::Deadline`]
    /// or [`AttachError::OwnerGone`] when the owner did not answer, after
    /// which the client is detached; [`AttachError::StaleGeneration`] when
    /// the owner was replaced; [`AttachError::Engine`] with the verb's own
    /// refusal.
    pub fn command(&mut self, command: OwnerCommand) -> Result<OwnerReply, AttachError> {
        let generation = self.generation;
        self.command_as(generation, command)
    }

    /// [`Self::command`] under an explicit generation — what a caller that
    /// holds a view from an earlier owner sends, so the owner can refuse
    /// it.
    ///
    /// # Errors
    /// As [`Self::command`].
    pub fn command_as(
        &mut self,
        generation: OwnerGeneration,
        command: OwnerCommand,
    ) -> Result<OwnerReply, AttachError> {
        let Some(link) = self.link.as_mut() else {
            return Err(AttachError::Detached);
        };
        let id = self.next_id;
        self.next_id += 1;
        let frame = ClientFrame::Command {
            id,
            generation,
            command: Box::new(command),
        };
        match link.exchange(&frame, id, self.deadline) {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(CommandRefusal::StaleGeneration { current })) => {
                Err(AttachError::StaleGeneration { current })
            }
            Ok(Err(CommandRefusal::Engine(why))) => Err(AttachError::Engine(why)),
            Err(error) => {
                // A late answer could no longer be told from the next
                // command's: the connection is spent.
                self.link = None;
                Err(error)
            }
        }
    }

    /// Tell the owner this client is leaving and close the connection.
    /// The owner cancels nothing on a detach: whatever it is doing for
    /// its own surface or for other clients goes on.
    pub fn detach(&mut self) {
        if let Some(mut link) = self.link.take() {
            link.detach();
        }
    }
}

impl Drop for OwnerClient {
    fn drop(&mut self) {
        self.detach();
    }
}

/// One probe exchange for [`OwnerProbe`](super::OwnerProbe): the handshake
/// as any client does it, then the owner's report line. The connection is
/// dropped afterwards; the owner closes its side as soon as it has written.
///
/// # Errors
/// As [`OwnerClient::attach_with`]; a welcome not followed by a report is
/// [`AttachError::OwnerGone`] or [`AttachError::Deadline`].
pub(super) fn probe_line(
    path: &Path,
    hello: &Hello,
    deadline: Duration,
) -> Result<super::owner_report::OwnerReport, AttachError> {
    let (mut link, _, _) = unix::connect(path, hello, deadline)?;
    link.next_line(deadline)
}

#[cfg(unix)]
mod unix {
    use std::io::{self, BufRead, BufReader, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::time::Duration;

    use crate::native::attach_error::AttachError;
    use crate::native::owner_generation::OwnerGeneration;
    use crate::native::owner_reply::OwnerReply;
    use crate::native::owner_wire::{ClientFrame, CommandRefusal, Greeting, Hello, ReplyFrame};

    /// The longest greeting accepted from an owner.
    const MAX_GREETING: usize = 64 * 1024;

    /// One attached connection: the stream, and a buffered reader over a
    /// clone of it for the command replies.
    #[derive(Debug)]
    pub(super) struct Link {
        stream: UnixStream,
        reader: BufReader<UnixStream>,
    }

    pub(super) fn connect(
        path: &Path,
        hello: &Hello,
        deadline: Duration,
    ) -> Result<(Link, OwnerGeneration, u32), AttachError> {
        let mut stream = UnixStream::connect(path).map_err(AttachError::Unavailable)?;
        stream
            .set_read_timeout(Some(deadline))
            .and_then(|()| stream.set_write_timeout(Some(deadline)))
            .map_err(AttachError::Unavailable)?;
        write_line(&mut stream, hello).map_err(|e| classify(e, deadline))?;
        let greeting: Greeting = read_unbuffered(&mut stream).map_err(|e| classify(e, deadline))?;
        let (generation, owner_pid) = match greeting {
            Greeting::Welcome {
                generation,
                owner_pid,
            } => (generation, owner_pid),
            Greeting::Refused(refusal) => return Err(AttachError::Refused(refusal)),
        };
        let reader = BufReader::new(stream.try_clone().map_err(AttachError::Unavailable)?);
        Ok((Link { stream, reader }, generation, owner_pid))
    }

    impl Link {
        pub(super) fn into_stream(self) -> UnixStream {
            self.stream
        }

        pub(super) fn exchange(
            &mut self,
            frame: &ClientFrame,
            id: u64,
            deadline: Duration,
        ) -> Result<Result<OwnerReply, CommandRefusal>, AttachError> {
            write_line(&mut self.stream, frame).map_err(|e| classify(e, deadline))?;
            let mut line = String::new();
            loop {
                line.clear();
                let read = self
                    .reader
                    .read_line(&mut line)
                    .map_err(|e| classify(e, deadline))?;
                if read == 0 {
                    return Err(AttachError::OwnerGone);
                }
                let reply: ReplyFrame = serde_json::from_str(line.trim_end())
                    .map_err(|e| AttachError::Wire(e.to_string()))?;
                if reply.id == id {
                    return Ok(reply.outcome);
                }
            }
        }

        pub(super) fn detach(&mut self) {
            let _ = write_line(&mut self.stream, &ClientFrame::Detach);
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }

        /// The next line the owner writes, as `T`; EOF is the owner gone.
        pub(super) fn next_line<T: serde::de::DeserializeOwned>(
            &mut self,
            deadline: Duration,
        ) -> Result<T, AttachError> {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .map_err(|e| classify(e, deadline))?;
            if read == 0 {
                return Err(AttachError::OwnerGone);
            }
            serde_json::from_str(line.trim_end()).map_err(|e| AttachError::Wire(e.to_string()))
        }
    }

    fn write_line<T: serde::Serialize>(writer: &mut impl Write, value: &T) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(value)?;
        bytes.push(b'\n');
        writer.write_all(&bytes)?;
        writer.flush()
    }

    /// One line read byte by byte, so nothing after it is taken from a
    /// stream that is about to carry another protocol.
    fn read_unbuffered<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if reader.read(&mut byte)? == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            if byte[0] == b'\n' {
                break;
            }
            line.push(byte[0]);
            if line.len() > MAX_GREETING {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "greeting line too long",
                ));
            }
        }
        serde_json::from_slice(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// What an I/O failure on the link means to the caller.
    fn classify(error: io::Error, deadline: Duration) -> AttachError {
        match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => AttachError::Deadline(deadline),
            io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::NotConnected => AttachError::OwnerGone,
            io::ErrorKind::InvalidData => AttachError::Wire(error.to_string()),
            _ => AttachError::Unavailable(error),
        }
    }
}

#[cfg(not(unix))]
mod unix {
    use std::path::Path;
    use std::time::Duration;

    use crate::native::attach_error::AttachError;
    use crate::native::owner_generation::OwnerGeneration;
    use crate::native::owner_reply::OwnerReply;
    use crate::native::owner_wire::{ClientFrame, CommandRefusal, Hello};

    #[derive(Debug)]
    pub(super) struct Link;

    pub(super) fn connect(
        _path: &Path,
        _hello: &Hello,
        _deadline: Duration,
    ) -> Result<(Link, OwnerGeneration, u32), AttachError> {
        Err(AttachError::Unsupported)
    }

    impl Link {
        pub(super) fn exchange(
            &mut self,
            _frame: &ClientFrame,
            _id: u64,
            _deadline: Duration,
        ) -> Result<Result<OwnerReply, CommandRefusal>, AttachError> {
            Err(AttachError::Unsupported)
        }

        pub(super) fn detach(&mut self) {}

        pub(super) fn next_line<T: serde::de::DeserializeOwned>(
            &mut self,
            _deadline: Duration,
        ) -> Result<T, AttachError> {
            Err(AttachError::Unsupported)
        }
    }
}
