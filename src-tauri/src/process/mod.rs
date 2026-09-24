// Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
use std::{
    ffi::OsString,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use tokio::{
    fs,
    fs::OpenOptions,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;

use crate::error::{Result, SplatError};

#[cfg(unix)]
fn signal_process_group(process_id: u32, signal: libc::c_int) -> std::io::Result<()> {
    // The child is made the leader of a new process group before spawn, so a
    // negative PID targets it and every descendant that stays in that group.
    let result = unsafe { libc::kill(-(process_id as libc::pid_t), signal) };
    if result == 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(windows)]
mod windows_job {
    use std::{io, mem::size_of, ptr};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{
                OpenProcess, OpenThread, ResumeThread, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
                THREAD_SUSPEND_RESUME,
            },
        },
    };

    pub struct WindowsJob(HANDLE);

    // Windows kernel handles are process-wide values and may be moved between
    // executor threads. Ownership still remains unique through this wrapper.
    unsafe impl Send for WindowsJob {}

    impl WindowsJob {
        pub fn create() -> io::Result<Self> {
            // SAFETY: Both optional pointers are null and the returned owned handle is
            // closed in Drop. SetInformation receives a correctly sized initialized struct.
            unsafe {
                let handle = CreateJobObjectW(ptr::null(), ptr::null());
                if handle.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    let error = io::Error::last_os_error();
                    CloseHandle(handle);
                    return Err(error);
                }
                Ok(Self(handle))
            }
        }

        pub fn assign(&self, process_id: u32) -> io::Result<()> {
            // SAFETY: OpenProcess returns an owned handle for the supplied child PID;
            // it remains valid through assignment and is closed on every path.
            unsafe {
                let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, process_id);
                if process.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let result = AssignProcessToJobObject(self.0, process);
                let error = if result == 0 {
                    Some(io::Error::last_os_error())
                } else {
                    None
                };
                CloseHandle(process);
                error.map_or(Ok(()), Err)
            }
        }

        pub fn resume_primary_thread(&self, process_id: u32) -> io::Result<()> {
            // SAFETY: Snapshot and thread handles are closed on every path, and the
            // enumeration structure advertises its exact size to Windows.
            unsafe {
                let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
                if snapshot == INVALID_HANDLE_VALUE {
                    return Err(io::Error::last_os_error());
                }
                let mut entry: THREADENTRY32 = std::mem::zeroed();
                entry.dwSize = size_of::<THREADENTRY32>() as u32;
                let mut has_entry = Thread32First(snapshot, &mut entry) != 0;
                while has_entry {
                    if entry.th32OwnerProcessID == process_id {
                        let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                        if thread.is_null() {
                            let error = io::Error::last_os_error();
                            CloseHandle(snapshot);
                            return Err(error);
                        }
                        let result = ResumeThread(thread);
                        let error = (result == u32::MAX).then(io::Error::last_os_error);
                        CloseHandle(thread);
                        CloseHandle(snapshot);
                        return error.map_or(Ok(()), Err);
                    }
                    has_entry = Thread32Next(snapshot, &mut entry) != 0;
                }
                CloseHandle(snapshot);
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "找不到暂停的子进程主线程",
                ))
            }
        }

        pub fn terminate(&self) {
            // SAFETY: self.0 is a live job handle owned by this value.
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for WindowsJob {
        fn drop(&mut self) {
            // SAFETY: The handle is owned and closed exactly once here.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub enum ProcessUpdate {
    Started { process_id: u32 },
    Line { stream: ProcessStream, line: String },
    Heartbeat { elapsed_ms: u64 },
}

pub type ProcessObserver = Arc<dyn Fn(ProcessUpdate) + Send + Sync>;

#[derive(Clone)]
pub struct ProcessSpec {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub working_directory: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
    pub observer: Option<ProcessObserver>,
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessOutput {
    /// Returns a bounded tail of the engine output so command errors retain the
    /// useful cause without sending an unbounded log through the Tauri bridge.
    pub fn failure_detail(&self) -> String {
        const MAX_CHARS: usize = 4_096;
        let source = if self.stderr.trim().is_empty() {
            self.stdout.trim()
        } else {
            self.stderr.trim()
        };
        let mut tail = source.chars().rev().take(MAX_CHARS).collect::<Vec<_>>();
        tail.reverse();
        tail.into_iter().collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProcessManager {
    cancellation: CancellationToken,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            cancellation: CancellationToken::new(),
        }
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub fn child_token(&self) -> CancellationToken {
        self.cancellation.child_token()
    }

    pub async fn run(&self, spec: ProcessSpec) -> Result<ProcessOutput> {
        self.run_inner(spec, false).await
    }

    /// Brush/indicatif only emits iteration progress to a terminal. Give stderr
    /// a PTY while keeping the direct child in our cancellable process group.
    pub async fn run_with_terminal(&self, spec: ProcessSpec) -> Result<ProcessOutput> {
        self.run_inner(spec, true).await
    }

    async fn run_inner(&self, spec: ProcessSpec, terminal: bool) -> Result<ProcessOutput> {
        if !spec.executable.is_file() {
            return Err(SplatError::EngineMissing(
                spec.executable.display().to_string(),
            ));
        }

        let started = Instant::now();
        #[cfg(windows)]
        let job = windows_job::WindowsJob::create().map_err(|error| {
            SplatError::Process(format!("无法创建 Windows Job Object：{error}"))
        })?;
        let mut command = Command::new(&spec.executable);
        command
            .args(&spec.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        let terminal_reader = if terminal {
            let (reader, writer) = open_progress_terminal()?;
            command
                .stderr(Stdio::from(writer))
                .env("TERM", "xterm-256color");
            Some(tokio::fs::File::from_std(reader))
        } else {
            None
        };
        if let Some(directory) = &spec.working_directory {
            command.current_dir(directory);
        }

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_SUSPENDED: u32 = 0x0000_0004;
            command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|error| SplatError::EngineStart {
            engine: spec.executable.display().to_string(),
            detail: error.to_string(),
        })?;
        // Command retains its Stdio handle after spawn; closing the parent's
        // PTY slave is essential for the master to reach EOF when Brush exits.
        drop(command);
        let process_id = child
            .id()
            .ok_or_else(|| SplatError::Process("无法读取子进程 ID".into()))?;
        #[cfg(windows)]
        {
            if let Err(error) = job.assign(process_id) {
                job.terminate();
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(SplatError::Process(format!(
                    "无法把子进程加入 Windows Job Object：{error}"
                )));
            }
            if let Err(error) = job.resume_primary_thread(process_id) {
                job.terminate();
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(SplatError::Process(format!(
                    "无法恢复子进程主线程：{error}"
                )));
            }
        }
        if let Some(observer) = &spec.observer {
            observer(ProcessUpdate::Started { process_id });
        }

        let log_file = if let Some(path) = &spec.log_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await?;
            let args = spec
                .args
                .iter()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n  ");
            file.write_all(
                format!(
                    "executable: {}\narguments:\n  {}\n",
                    spec.executable.display(),
                    args
                )
                .as_bytes(),
            )
            .await?;
            Some(Arc::new(Mutex::new(file)))
        } else {
            None
        };
        let stdout_task = tokio::spawn(pump_stream(
            child.stdout.take().expect("stdout is piped"),
            ProcessStream::Stdout,
            spec.observer.clone(),
            log_file.clone(),
        ));
        let stderr_reader: Box<dyn AsyncRead + Unpin + Send> = {
            #[cfg(unix)]
            if let Some(reader) = terminal_reader {
                Box::new(reader)
            } else {
                Box::new(child.stderr.take().expect("stderr is piped"))
            }
            #[cfg(not(unix))]
            Box::new(child.stderr.take().expect("stderr is piped"))
        };
        let stderr_task = tokio::spawn(pump_stream_inner(
            stderr_reader,
            ProcessStream::Stderr,
            spec.observer.clone(),
            log_file.clone(),
            terminal,
        ));
        let finished = Arc::new(AtomicBool::new(false));
        let heartbeat_task = spec.observer.clone().map(|observer| {
            let finished = finished.clone();
            tokio::spawn(async move {
                while !finished.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    if !finished.load(Ordering::Relaxed) {
                        observer(ProcessUpdate::Heartbeat {
                            elapsed_ms: started.elapsed().as_millis() as u64,
                        });
                    }
                }
            })
        });

        let status = tokio::select! {
            status = child.wait() => status?,
            _ = self.cancellation.cancelled() => {
                #[cfg(windows)]
                job.terminate();
                #[cfg(unix)]
                {
                    let _ = signal_process_group(process_id, libc::SIGTERM);
                    if tokio::time::timeout(Duration::from_secs(3), child.wait()).await.is_err() {
                        let _ = signal_process_group(process_id, libc::SIGKILL);
                        let _ = child.wait().await;
                    } else {
                        // The direct child can exit while a descendant ignores SIGTERM.
                        // The process-group ID remains usable until the last member exits.
                        let _ = signal_process_group(process_id, libc::SIGKILL);
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
                finished.store(true, Ordering::Relaxed);
                stdout_task.abort();
                stderr_task.abort();
                if let Some(task) = heartbeat_task { task.abort(); }
                if let Some(log_path) = &spec.log_path {
                    if let Some(parent) = log_path.parent() { fs::create_dir_all(parent).await?; }
                    let mut file = OpenOptions::new().create(true).append(true).open(log_path).await?;
                    file.write_all(format!("cancelled after {} ms\n\n", started.elapsed().as_millis()).as_bytes()).await?;
                }
                return Err(SplatError::Cancelled);
            }
        };
        finished.store(true, Ordering::Relaxed);
        if let Some(task) = heartbeat_task {
            let _ = task.await;
        }

        let stdout = stdout_task
            .await
            .map_err(|error| SplatError::Process(error.to_string()))??;
        let stderr = stderr_task
            .await
            .map_err(|error| SplatError::Process(error.to_string()))??;
        let output = ProcessOutput {
            success: status.success(),
            exit_code: status.code(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        };

        if let Some(file) = log_file {
            file.lock()
                .await
                .write_all(
                    format!(
                        "exit_code: {:?}\nelapsed_ms: {}\n\n",
                        output.exit_code,
                        started.elapsed().as_millis()
                    )
                    .as_bytes(),
                )
                .await?;
        }

        Ok(output)
    }
}

async fn pump_stream<R: AsyncRead + Unpin>(
    reader: R,
    stream: ProcessStream,
    observer: Option<ProcessObserver>,
    log: Option<Arc<Mutex<tokio::fs::File>>>,
) -> std::io::Result<Vec<u8>> {
    pump_stream_inner(reader, stream, observer, log, false).await
}

#[cfg(unix)]
fn open_progress_terminal() -> std::io::Result<(std::fs::File, std::fs::File)> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let (mut master, mut slave) = (-1, -1);
    let mut size = libc::winsize {
        ws_row: 30,
        ws_col: 180,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: valid output pointers and window size; no termios or name buffer.
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: openpty returned two distinct owned descriptors. File closes each once.
    let pair = unsafe {
        (
            std::fs::File::from_raw_fd(master),
            std::fs::File::from_raw_fd(slave),
        )
    };
    for file in [&pair.0, &pair.1] {
        // Prevent inherited master/slave copies from keeping the terminal alive.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(pair)
}

fn clean_terminal_line(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars();
    let mut clean = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.next() == Some('[') {
                for code in chars.by_ref() {
                    if ('@'..='~').contains(&code) {
                        break;
                    }
                }
            }
        } else if !ch.is_control() || ch == '\t' {
            clean.push(ch);
        }
    }
    clean.trim().to_owned()
}

async fn pump_stream_inner<R: AsyncRead + Unpin>(
    mut reader: R,
    stream: ProcessStream,
    observer: Option<ProcessObserver>,
    log: Option<Arc<Mutex<tokio::fs::File>>>,
    terminal: bool,
) -> std::io::Result<Vec<u8>> {
    let mut collected = Vec::new();
    let mut line = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(read) => read,
            // Some Unix PTYs signal slave closure with EIO instead of EOF.
            #[cfg(unix)]
            Err(error) if terminal && error.raw_os_error() == Some(libc::EIO) => 0,
            Err(error) => return Err(error),
        };
        if read == 0 {
            break;
        }
        collected.extend_from_slice(&buffer[..read]);
        if collected.len() > 4 * 1024 * 1024 {
            collected.drain(..collected.len() - 4 * 1024 * 1024);
        }
        if let Some(file) = &log {
            file.lock().await.write_all(&buffer[..read]).await?;
        }
        for &byte in &buffer[..read] {
            if byte == b'\n' || byte == b'\r' {
                if let Some(observer) = &observer {
                    let text = clean_terminal_line(&line);
                    if !text.is_empty() {
                        observer(ProcessUpdate::Line { stream, line: text });
                    }
                }
                line.clear();
            } else {
                line.push(byte);
            }
        }
    }
    if let Some(observer) = &observer {
        let text = clean_terminal_line(&line);
        if !text.is_empty() {
            observer(ProcessUpdate::Line { stream, line: text });
        }
    }
    Ok(collected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn failure_detail_prefers_stderr_and_keeps_only_a_bounded_tail() {
        let output = ProcessOutput {
            success: false,
            exit_code: Some(1),
            stdout: "less useful stdout".into(),
            stderr: format!("{}early eof", "x".repeat(5_000)),
        };
        let detail = output.failure_detail();
        assert!(detail.ends_with("early eof"));
        assert_eq!(detail.chars().count(), 4_096);
        assert!(!detail.contains("less useful stdout"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn terminal_streams_live_carriage_returns_and_preserves_exit_status() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let observer: ProcessObserver = Arc::new(move |update| {
            if let ProcessUpdate::Line { line, .. } = update {
                let _ = tx.send(line);
            }
        });
        let run = tokio::spawn(async move {
            ProcessManager::new().run_with_terminal(ProcessSpec {
                executable: "/bin/sh".into(),
                args: vec!["-c".into(), r"test -t 2 || exit 9; printf '\033[2K12/100 Steps\r' >&2; sleep 1; printf '\033[2K100/100 Steps\r' >&2; exit 7".into()],
                working_directory: None, log_path: None, observer: Some(observer),
            }).await.unwrap()
        });
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .unwrap()
                .unwrap(),
            "12/100 Steps"
        );
        assert!(!run.is_finished(), "progress must arrive before child exit");
        let output = tokio::time::timeout(Duration::from_secs(5), run)
            .await
            .unwrap()
            .unwrap();
        assert!(!output.success);
        assert_eq!(output.exit_code, Some(7));
        assert_eq!(rx.recv().await.unwrap(), "100/100 Steps");
    }

    #[cfg(windows)]
    fn system_executable(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join("System32")
            .join(name)
    }

    #[cfg(windows)]
    fn process_has_exited(process_id: u32) -> bool {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, WAIT_OBJECT_0},
            System::Threading::{OpenProcess, WaitForSingleObject},
        };
        // SAFETY: The process handle is opened only for synchronization, checked for null,
        // observed without blocking, and closed exactly once.
        unsafe {
            const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
            let process = OpenProcess(SYNCHRONIZE_ACCESS, 0, process_id);
            if process.is_null() {
                return true;
            }
            let result = WaitForSingleObject(process, 0) == WAIT_OBJECT_0;
            CloseHandle(process);
            result
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn suspended_windows_process_is_resumed_after_job_assignment() {
        let output = ProcessManager::new()
            .run(ProcessSpec {
                executable: system_executable("cmd.exe"),
                args: vec!["/C".into(), "echo resumed".into()],
                working_directory: None,
                log_path: None,
                observer: None,
            })
            .await
            .unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("resumed"));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn cancellation_terminates_a_windows_descendant_created_immediately() {
        let descendant_pid = Arc::new(std::sync::Mutex::new(None::<u32>));
        let observer: ProcessObserver = {
            let descendant_pid = descendant_pid.clone();
            Arc::new(move |update| {
                if let ProcessUpdate::Line { line, .. } = update {
                    if let Ok(pid) = line.parse() {
                        *descendant_pid.lock().unwrap() = Some(pid);
                    }
                }
            })
        };
        let manager = ProcessManager::new();
        let running_manager = manager.clone();
        let run = tokio::spawn(async move {
            running_manager
                .run(ProcessSpec {
                    executable: system_executable("WindowsPowerShell\\v1.0\\powershell.exe"),
                    args: vec![
                        "-NoProfile".into(),
                        "-Command".into(),
                        "$p = Start-Process -PassThru -WindowStyle Hidden ping -ArgumentList '-t','127.0.0.1'; $p.Id; Wait-Process -Id $p.Id".into(),
                    ],
                    working_directory: None,
                    log_path: None,
                    observer: Some(observer),
                })
                .await
        });
        for _ in 0..100 {
            if descendant_pid.lock().unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let process_id = descendant_pid.lock().unwrap().expect("descendant PID");
        manager.cancel();
        assert!(matches!(run.await.unwrap(), Err(SplatError::Cancelled)));
        for _ in 0..100 {
            if process_has_exited(process_id) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("descendant {process_id} escaped the Windows Job Object");
    }

    #[tokio::test]
    async fn streams_stdout_and_stderr_concurrently_and_persists_log() {
        let directory = tempfile::tempdir().unwrap();
        let log_path = directory.path().join("process.log");
        let log = Arc::new(Mutex::new(
            tokio::fs::File::create(&log_path).await.unwrap(),
        ));
        let updates = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let observer: ProcessObserver = {
            let updates = updates.clone();
            Arc::new(move |update| {
                if let ProcessUpdate::Line { line, .. } = update {
                    updates.lock().unwrap().push(line);
                }
            })
        };
        let (mut stdout_writer, stdout_reader) = tokio::io::duplex(256);
        let (mut stderr_writer, stderr_reader) = tokio::io::duplex(256);
        let stdout_task = tokio::spawn(pump_stream(
            stdout_reader,
            ProcessStream::Stdout,
            Some(observer.clone()),
            Some(log.clone()),
        ));
        let stderr_task = tokio::spawn(pump_stream(
            stderr_reader,
            ProcessStream::Stderr,
            Some(observer),
            Some(log.clone()),
        ));
        stdout_writer
            .write_all(b"frame=1\nframe=2\n")
            .await
            .unwrap();
        stderr_writer.write_all(b"warning line\n").await.unwrap();
        drop(stdout_writer);
        drop(stderr_writer);
        stdout_task.await.unwrap().unwrap();
        stderr_task.await.unwrap().unwrap();
        log.lock().await.sync_all().await.unwrap();

        let captured = updates.lock().unwrap().clone();
        assert_eq!(captured.len(), 3);
        assert!(captured.contains(&"frame=1".to_string()));
        assert!(captured.contains(&"warning line".to_string()));
        let disk = tokio::fs::read_to_string(log_path).await.unwrap();
        assert!(disk.contains("frame=2"));
        assert!(disk.contains("warning line"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_terminates_descendant_processes() {
        for terminal in [false, true] {
            let descendant_pid = Arc::new(std::sync::Mutex::new(None::<u32>));
            let observer: ProcessObserver = {
                let descendant_pid = descendant_pid.clone();
                Arc::new(move |update| {
                    if let ProcessUpdate::Line { line, .. } = update {
                        if let Ok(pid) = line.parse() {
                            *descendant_pid.lock().unwrap() = Some(pid);
                        }
                    }
                })
            };
            let manager = ProcessManager::new();
            let running_manager = manager.clone();
            let run = tokio::spawn(async move {
                running_manager
                    .run_inner(
                        ProcessSpec {
                            executable: PathBuf::from("/bin/sh"),
                            args: vec!["-c".into(), "sleep 30 & echo $!; wait".into()],
                            working_directory: None,
                            log_path: None,
                            observer: Some(observer),
                        },
                        terminal,
                    )
                    .await
            });

            for _ in 0..50 {
                if descendant_pid.lock().unwrap().is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let pid = descendant_pid.lock().unwrap().expect("descendant PID");
            manager.cancel();
            assert!(matches!(run.await.unwrap(), Err(SplatError::Cancelled)));

            let mut terminated = false;
            for _ in 0..150 {
                if descendant_is_terminated(pid) {
                    terminated = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert!(
                terminated,
                "descendant {pid} was still running after cancellation"
            );
        }
    }

    /// A killed descendant is reparented to PID 1, and PID 1 does not reap in every
    /// container, so the process can linger as a zombie with its `/proc` entry intact.
    /// The entry being gone and the entry being a zombie both mean the process stopped
    /// running, which is what cancellation has to guarantee.
    #[cfg(target_os = "linux")]
    fn descendant_is_terminated(pid: u32) -> bool {
        let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
            return true;
        };
        status
            .lines()
            .find_map(|line| line.strip_prefix("State:"))
            .map(|state| state.trim_start().starts_with('Z'))
            .unwrap_or(true)
    }

    /// Unix targets without `/proc` reap orphans through PID 1, so a terminated
    /// descendant disappears outright rather than lingering as a visible zombie.
    #[cfg(all(unix, not(target_os = "linux")))]
    fn descendant_is_terminated(pid: u32) -> bool {
        // SAFETY: signal 0 only runs the existence and permission checks for the PID
        // and delivers nothing, so no process state is observed or modified.
        let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
        !alive
    }
}
