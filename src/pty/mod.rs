use anyhow::Result;
use nix::libc::{dup2, winsize};
use nix::pty::openpty;
use nix::unistd::{execv, fork, read, setsid, write, ForkResult};
use std::ffi::CString;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::sync::mpsc;

pub struct PtyContext {
    pub master: OwnedFd,
    pub rx: mpsc::Receiver<Vec<u8>>,
}

impl PtyContext {
    pub fn new(win_size: winsize) -> Self {
        let res = openpty(&win_size, None).expect("Opening pty failed");
        unsafe {
            let fork_result = fork().expect("Could not fork process");
            match fork_result {
                ForkResult::Parent { child: _ } => {
                    let (tx, rx) = mpsc::channel();
                    let master_clone = res.master.try_clone().expect("Could not clone master fd");

                    std::thread::spawn(move || {
                        loop {
                            let mut buf = vec![0u8; 4096];
                            let n = read(&master_clone, &mut buf)
                                .expect("Could not read from master fd");
                            buf.truncate(n);
                            tx.send(buf).unwrap();
                        }
                    });

                    Self {
                        master: res.master,
                        rx,
                    }
                }
                ForkResult::Child => {
                    setsid().expect("Could not set a new session");
                    dup2(res.slave.as_raw_fd(), 0);
                    dup2(res.slave.as_raw_fd(), 1);
                    dup2(res.slave.as_raw_fd(), 2);
                    let shell_path = CString::new("/bin/zsh").expect("Could not create shell path");
                    execv(&shell_path, &[shell_path.as_c_str()]).expect("Could not exec shell");
                    unreachable!()
                }
            }
        }
    }
    pub fn write(&self, data: &[u8]) -> Result<()> {
        write(&self.master, data).expect("Could not write to master fd");
        Ok(())
    }
    pub fn read(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; 4096];
        let n = read(&self.master, &mut buf).expect("Could not read from master fd");
        buf.truncate(n);
        Ok(buf)
    }
    pub fn resize(&self, size: &winsize) -> Result<()> {
        unsafe {
            nix::libc::ioctl(self.master.as_raw_fd(), nix::libc::TIOCSWINSZ, size);
            Ok(())
        }
    }
}
