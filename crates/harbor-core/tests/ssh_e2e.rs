//! End-to-end tests that exercise the **real** russh transport and russh-sftp
//! client against a locally-spawned OpenSSH `sshd`.
//!
//! These tests run only when an `sshd` binary is available *and* can be started
//! unprivileged (true on most Linux dev boxes and CI). When it isn't, they print
//! a skip notice and pass, so `cargo test` stays green everywhere.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use harbor_core::application::ports::{
    CancelFlag, ConnectionParams, KnownHostsStore, ProgressFn, ShellEvent, ShellInput,
};
use harbor_core::application::SessionService;
use harbor_core::domain::auth::Credential;
use harbor_core::domain::host_key::{HostKey, KnownHostEntry};
use harbor_core::domain::session::PtySize;
use harbor_core::infrastructure::host_key_policy::StrictPolicy;
use harbor_core::infrastructure::known_hosts::host_pattern;
use harbor_core::infrastructure::ssh::RusshTransport;
use harbor_core::testing::InMemoryKnownHosts;

/// A throwaway sshd instance and the material needed to connect to it.
struct Sshd {
    _dir: tempfile::TempDir,
    child: Child,
    port: u16,
    username: String,
    host_pub: String,
    client_key: PathBuf,
}

impl Drop for Sshd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn sshd_bin() -> Option<&'static str> {
    ["/usr/sbin/sshd", "/usr/local/sbin/sshd", "/sbin/sshd"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn keygen(path: &std::path::Path) -> bool {
    Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-f",
            path.to_str().unwrap(),
            "-N",
            "",
            "-q",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Provision and start an unprivileged sshd. Returns `None` if the environment
/// can't support it (no binary, no ssh-keygen, failed to bind, …).
fn start_sshd() -> Option<Sshd> {
    let bin = sshd_bin()?;
    let dir = tempfile::tempdir().ok()?;
    let d = dir.path();

    let host = d.join("host");
    let id = d.join("id");
    if !keygen(&host) || !keygen(&id) {
        return None;
    }
    std::fs::copy(d.join("id.pub"), d.join("authorized_keys")).ok()?;
    let host_pub = std::fs::read_to_string(d.join("host.pub")).ok()?;

    let port = free_port();
    let config = format!(
        "ListenAddress 127.0.0.1\n\
         HostKey {host}\n\
         PidFile {pid}\n\
         AuthorizedKeysFile {ak}\n\
         StrictModes no\n\
         UsePAM no\n\
         PasswordAuthentication no\n\
         PubkeyAuthentication yes\n\
         PrintMotd no\n\
         Subsystem sftp internal-sftp\n",
        host = host.display(),
        pid = d.join("sshd.pid").display(),
        ak = d.join("authorized_keys").display(),
    );
    let config_path = d.join("sshd_config");
    std::fs::write(&config_path, config).ok()?;

    let log = std::fs::File::create(d.join("sshd.log")).ok()?;
    let child = Command::new(bin)
        .args([
            "-D",
            "-e",
            "-f",
            config_path.to_str().unwrap(),
            "-p",
            &port.to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .ok()?;

    Some(Sshd {
        _dir: dir,
        child,
        port,
        username: std::env::var("USER").unwrap_or_else(|_| "root".into()),
        host_pub,
        client_key: id,
    })
}

impl Sshd {
    fn host_key(&self) -> HostKey {
        let mut it = self.host_pub.split_whitespace();
        HostKey::new(it.next().unwrap_or(""), it.next().unwrap_or(""))
    }

    async fn wait_ready(&self) -> bool {
        for _ in 0..60 {
            if tokio::net::TcpStream::connect(("127.0.0.1", self.port))
                .await
                .is_ok()
            {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        false
    }

    fn params(&self) -> ConnectionParams {
        ConnectionParams {
            host: "127.0.0.1".into(),
            port: self.port,
            username: self.username.clone(),
            credential: Credential::PrivateKey {
                key_path: self.client_key.clone(),
                passphrase: None,
            },
        }
    }
}

fn service(known: Arc<dyn KnownHostsStore>) -> SessionService {
    SessionService::new(Arc::new(RusshTransport), known, Arc::new(StrictPolicy))
}

async fn read_until(output: &mut tokio::sync::mpsc::Receiver<ShellEvent>, marker: &str) -> String {
    let mut buf = String::new();
    for _ in 0..40 {
        match tokio::time::timeout(Duration::from_millis(500), output.recv()).await {
            Ok(Some(ShellEvent::Output(bytes))) => {
                buf.push_str(&String::from_utf8_lossy(&bytes));
                if buf.contains(marker) {
                    return buf;
                }
            }
            Ok(Some(ShellEvent::Closed { .. })) | Ok(None) => return buf,
            Err(_) => {}
        }
    }
    buf
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_to_end_shell_and_sftp() {
    let Some(sshd) = start_sshd() else {
        eprintln!("skipping: no unprivileged sshd available");
        return;
    };
    assert!(sshd.wait_ready().await, "sshd did not start listening");

    // Pre-trust the real host key so verification yields Trusted (no prompt).
    let known = Arc::new(InMemoryKnownHosts::with_entries(vec![KnownHostEntry {
        host_field: host_pattern("127.0.0.1", sshd.port),
        key: sshd.host_key(),
        hashed: false,
        marker: None,
    }]));
    let svc = service(known);

    // --- connect with public-key auth -------------------------------------
    let info = svc
        .connect(sshd.params(), None, "e2e")
        .await
        .expect("connect + pubkey auth should succeed");
    assert!(svc.is_connected(info.id).await);

    // --- interactive shell -------------------------------------------------
    let mut shell = svc.open_shell(info.id, PtySize::default()).await.unwrap();
    shell
        .input
        .send(ShellInput::Data(b"echo HARBOR_E2E_OK\n".to_vec()))
        .await
        .unwrap();
    let out = read_until(&mut shell.output, "HARBOR_E2E_OK").await;
    assert!(out.contains("HARBOR_E2E_OK"), "shell output was: {out:?}");
    let _ = shell.input.send(ShellInput::Data(b"exit\n".to_vec())).await;

    // --- SFTP upload / download / dir ops ----------------------------------
    let sftp = svc.sftp(info.id).await.unwrap();
    let home = sftp.canonicalize(".").await.unwrap();
    assert!(!home.is_empty());

    let work = sshd._dir.path().join("xfer");
    std::fs::create_dir_all(&work).unwrap();
    let src = work.join("src.bin");
    let content: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&src, &content).unwrap();
    let remote = work.join("remote.bin").to_string_lossy().into_owned();

    let progress_calls = Arc::new(AtomicU64::new(0));
    let pc = Arc::clone(&progress_calls);
    let progress: ProgressFn = Arc::new(move |_t, _total| {
        pc.fetch_add(1, Ordering::SeqCst);
    });
    let cancel: CancelFlag = Arc::new(AtomicBool::new(false));

    let uploaded = sftp
        .upload(&src, &remote, Arc::clone(&progress), Arc::clone(&cancel))
        .await
        .unwrap();
    assert_eq!(uploaded, content.len() as u64);
    assert!(
        progress_calls.load(Ordering::SeqCst) > 0,
        "no progress reported"
    );
    // Same machine: the uploaded file is byte-identical on disk.
    assert_eq!(std::fs::read(&remote).unwrap(), content);

    let dl = work.join("dl.bin");
    let downloaded = sftp
        .download(&remote, &dl, Arc::clone(&progress), Arc::clone(&cancel))
        .await
        .unwrap();
    assert_eq!(downloaded, content.len() as u64);
    assert_eq!(std::fs::read(&dl).unwrap(), content);

    // Directory operations.
    let subdir = work.join("sub").to_string_lossy().into_owned();
    sftp.mkdir(&subdir).await.unwrap();
    let listing = sftp.read_dir(&work.to_string_lossy()).await.unwrap();
    assert!(listing.iter().any(|e| e.name == "sub" && e.is_dir()));
    assert!(listing.iter().any(|e| e.name == "src.bin"));

    let renamed = work.join("renamed.bin").to_string_lossy().into_owned();
    sftp.rename(&remote, &renamed).await.unwrap();
    assert!(std::fs::metadata(&renamed).is_ok());

    sftp.remove_file(&renamed).await.unwrap();
    sftp.remove_dir(&subdir).await.unwrap();

    svc.disconnect(info.id).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_key_mismatch_refuses_connection() {
    let Some(sshd) = start_sshd() else {
        eprintln!("skipping: no unprivileged sshd available");
        return;
    };
    assert!(sshd.wait_ready().await, "sshd did not start listening");

    // Seed known_hosts with a DIFFERENT key for this host → must be refused.
    let wrong = HostKey::new(
        "ssh-ed25519",
        "AAAAC3NzaC1lZDI1NTE5AAAAIHR1VLZ8uFbq13WOvgPLijFCD1COlDFmkWX2Eq4fzXON",
    );
    let known = Arc::new(InMemoryKnownHosts::with_entries(vec![KnownHostEntry {
        host_field: host_pattern("127.0.0.1", sshd.port),
        key: wrong,
        hashed: false,
        marker: None,
    }]));
    let svc = service(known);

    let err = svc
        .connect(sshd.params(), None, "e2e")
        .await
        .expect_err("a changed host key must refuse the connection");
    assert_eq!(
        err.code(),
        "host_key_mismatch",
        "expected a host-key mismatch, got: {err}"
    );
}
