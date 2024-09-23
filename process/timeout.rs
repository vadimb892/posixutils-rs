//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use nix::{
    errno::Errno,
    sys::wait::{waitpid, WaitPidFlag, WaitStatus},
    unistd::{execvp, fork, ForkResult},
};
use plib::PROJECT_NAME;
use std::{
    error::Error,
    ffi::CString,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Mutex,
    },
    time::Duration,
};

#[cfg(target_os = "macos")]
const SIGLIST: [(&str, i32); 31] = [
    ("HUP", 1),
    ("INT", 2),
    ("QUIT", 3),
    ("ILL", 4),
    ("TRAP", 5),
    ("ABRT", 6),
    ("EMT", 7),
    ("FPE", 8),
    ("KILL", 9),
    ("BUS", 10),
    ("SEGV", 11),
    ("SYS", 12),
    ("PIPE", 13),
    ("ALRM", 14),
    ("TERM", 15),
    ("URG", 16),
    ("STOP", 17),
    ("TSTP", 18),
    ("CONT", 19),
    ("CHLD", 20),
    ("TTIN", 21),
    ("TTOU", 22),
    ("IO", 23),
    ("XCPU", 24),
    ("XFSZ", 25),
    ("VTALRM", 26),
    ("PROF", 27),
    ("WINCH", 28),
    ("INFO", 29),
    ("USR1", 30),
    ("USR2", 31),
];

#[cfg(target_os = "linux")]
const SIGLIST: [(&str, i32); 32] = [
    ("HUP", 1),
    ("INT", 2),
    ("QUIT", 3),
    ("ILL", 4),
    ("TRAP", 5),
    ("ABRT", 6),
    ("IOT", 6),
    ("BUS", 7),
    ("FPE", 8),
    ("KILL", 9),
    ("USR1", 10),
    ("SEGV", 11),
    ("USR2", 12),
    ("PIPE", 13),
    ("ALRM", 14),
    ("TERM", 15),
    ("STKFLT", 16),
    ("CHLD", 17),
    ("CONT", 18),
    ("STOP", 19),
    ("TSTP", 20),
    ("TTIN", 21),
    ("TTOU", 22),
    ("URG", 23),
    ("XCPU", 24),
    ("XFSZ", 25),
    ("VTALRM", 26),
    ("PROF", 27),
    ("WINCH", 28),
    ("IO", 29),
    ("PWR", 30),
    ("SYS", 31),
];

static FOREGROUND: AtomicBool = AtomicBool::new(false);
static FIRST_SIGNAL: AtomicI32 = AtomicI32::new(libc::SIGTERM);
static KILL_AFTER: Mutex<Option<Duration>> = Mutex::new(None);
static MONITORED_PID: AtomicI32 = AtomicI32::new(0);
static TIMED_OUT: AtomicBool = AtomicBool::new(false);

/// timeout — execute a utility with a time limit
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Only time out the utility itself, not its descendants.
    #[arg(short = 'f', long)]
    foreground: bool,

    /// Always preserve (mimic) the wait status of the executed utility, even if the time limit was reached.
    #[arg(short = 'p', long)]
    preserve_status: bool,

    /// Send a SIGKILL signal if the child process created to execute the utility has not terminated after the time period
    /// specified by time has elapsed since the first signal was sent. The value of time shall be interpreted as specified for
    /// the duration operand.
    #[arg(short = 'k', long, value_parser = parse_duration)]
    kill_after: Option<Duration>,

    /// Specify the signal to send when the time limit is reached, using one of the symbolic names defined in the <signal.h> header.
    /// Values of signal shall be recognized in a case-independent fashion, without the SIG prefix. By default, SIGTERM shall be sent.
    #[arg(short = 's', long, default_value = "TERM", value_parser = parse_signal)]
    signal_name: i32,

    /// The maximum amount of time to allow the utility to run, specified as a decimal number with an optional decimal fraction and an optional suffix.
    #[arg(name = "DURATION", value_parser = parse_duration)]
    duration: Duration,

    /// The name of a utility that is to be executed.
    #[arg(name = "UTILITY")]
    utility: String,

    /// Any string to be supplied as an argument when executing the utility named by the utility operand.
    #[arg(name = "ARGUMENT", trailing_var_arg = true)]
    arguments: Vec<String>,
}

/// Parses string slice into [Duration].
///
/// # Arguments
///
/// * `s` - [str] that represents duration.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
///
/// Returns the parsed [Duration] value.
fn parse_duration(s: &str) -> Result<Duration, String> {
    let (value, suffix) = s.split_at(
        s.find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(s.len()),
    );

    let value: f64 = value
        .parse()
        .map_err(|_| format!("invalid duration format '{s}'"))?;

    let multiplier = match suffix {
        "s" | "" => 1.0,
        "m" => 60.0,
        "h" => 3600.0,
        "d" => 86400.0,
        _ => return Err(format!("invalid duration format '{s}'")),
    };

    Ok(Duration::from_secs_f64(value * multiplier))
}

/// Parses [str] into [Signal].
///
/// # Arguments
///
/// * `s` - [str] that represents the signal name.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
///
/// Returns the parsed [Signal] value.
fn parse_signal(signal_name: &str) -> Result<i32, String> {
    let normalized = signal_name.trim().to_uppercase();
    let normalized = normalized.strip_prefix("SIG").unwrap_or(&normalized);

    for (name, num) in SIGLIST.iter() {
        if name == &normalized {
            return Ok(*num);
        }
    }
    Err(format!("invalid signal name '{signal_name}'"))
}

/// Starts the timeout after which [Signal::SIGALRM] will be send.
///
/// # Arguments
///
/// * `duration` - [Duration] value of
fn set_timeout(duration: Duration) {
    if !duration.is_zero() {
        unsafe { libc::alarm(duration.as_secs() as libc::c_uint) };
    }
}

/// Sends a signal to the process or process group.
fn send_signal(pid: i32, signal: i32) {
    if pid == 0 {
        unsafe { libc::signal(signal, libc::SIG_IGN) };
    }
    unsafe {
        libc::kill(pid, signal);
    }
}

/// Signal [Signal::SIGCHLD] handler.
extern "C" fn chld_handler(_signal: i32) {}

/// Timeout signal handler.
///
/// # Arguments
///
/// * `signal` - integer value of incoming signal.
extern "C" fn handler(mut signal: i32) {
    // When timeout receives [libc::SIGALRM], this will be considered as timeout reached and
    // timeout will send prepared signal
    if signal == libc::SIGALRM {
        TIMED_OUT.store(true, Ordering::SeqCst);
        signal = FIRST_SIGNAL.load(Ordering::SeqCst);
    }
    match MONITORED_PID.load(Ordering::SeqCst).cmp(&0) {
        std::cmp::Ordering::Less => {}
        std::cmp::Ordering::Equal => std::process::exit(128 + signal),
        std::cmp::Ordering::Greater => {
            let mut kill_after = KILL_AFTER.lock().unwrap();
            if let Some(duration) = *kill_after {
                FIRST_SIGNAL.store(libc::SIGKILL, Ordering::SeqCst);
                set_timeout(duration);
                *kill_after = None;
            }

            // Propagating incoming signal
            send_signal(MONITORED_PID.load(Ordering::SeqCst), signal);

            if !FOREGROUND.load(Ordering::SeqCst) {
                send_signal(0, signal);
                if signal != libc::SIGKILL && signal != libc::SIGCONT {
                    send_signal(MONITORED_PID.load(Ordering::SeqCst), libc::SIGCONT);
                    send_signal(0, libc::SIGCONT);
                }
            }
        }
    }
}

fn get_empty_sig_set() -> libc::sigset_t {
    let mut sig_set = std::mem::MaybeUninit::uninit();
    let _ = unsafe { libc::sigemptyset(sig_set.as_mut_ptr()) };
    unsafe { sig_set.assume_init() }
}

/// Unblocks incoming signal by adding it to empty signals mask.
///
/// # Arguments
///
/// `signal` - signal of type [Signal] that needs to be unblocked.
fn unblock_signal(signal: i32) {
    unsafe {
        let mut sig_set = get_empty_sig_set();

        libc::sigaddset(&mut sig_set, signal);
        if libc::sigprocmask(
            libc::SIG_UNBLOCK,
            &sig_set,
            std::ptr::null_mut::<libc::sigset_t>(),
        ) != 0
        {
            eprintln!("timeout: failed to set unblock signals mask");
            std::process::exit(125)
        }
    }
}

/// Installs handler for [Signal::SIGCHLD] signal to receive child's exit status code from parent (timeout).
fn set_chld() {
    unsafe {
        let mut sig_action = std::mem::MaybeUninit::<libc::sigaction>::uninit();
        let p_sa = sig_action.as_mut_ptr();
        (*p_sa).sa_sigaction = chld_handler as *const extern "C" fn(libc::c_int) as usize;
        (*p_sa).sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut (*p_sa).sa_mask);
        let sig_action = sig_action.assume_init();

        libc::sigaction(
            libc::SIGCHLD,
            &sig_action,
            std::ptr::null_mut::<libc::sigaction>(),
        );
    }

    unblock_signal(libc::SIGCHLD);
}

/// Installs handler ([handler]) for incoming [Signal] and other signals.
///
/// # Arguments
///
/// `signal` - signal of type [Signal] that needs to be handled.
fn set_handler(signal: i32) {
    unsafe {
        let mut sig_action = std::mem::MaybeUninit::<libc::sigaction>::uninit();
        let p_sa = sig_action.as_mut_ptr();
        (*p_sa).sa_sigaction = handler as *const extern "C" fn(libc::c_int) as usize;
        (*p_sa).sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut (*p_sa).sa_mask);
        let sig_action = sig_action.assume_init();

        libc::sigaction(
            libc::SIGALRM,
            &sig_action,
            std::ptr::null_mut::<libc::sigaction>(),
        );
        libc::sigaction(
            libc::SIGINT,
            &sig_action,
            std::ptr::null_mut::<libc::sigaction>(),
        );
        libc::sigaction(
            libc::SIGQUIT,
            &sig_action,
            std::ptr::null_mut::<libc::sigaction>(),
        );
        libc::sigaction(
            libc::SIGHUP,
            &sig_action,
            std::ptr::null_mut::<libc::sigaction>(),
        );
        libc::sigaction(
            libc::SIGTERM,
            &sig_action,
            std::ptr::null_mut::<libc::sigaction>(),
        );
        libc::sigaction(signal, &sig_action, std::ptr::null_mut::<libc::sigaction>());
    }
}

/// Blocks incoming signal and stores previous signals mask.
///
/// # Arguments
///
/// `signal` - signal of type [Signal] that needs to be handled.
///
/// `old_set` - mutable reference to set of gidnals of type [SigSet] into which will be placed previous mask.
fn block_handler_and_chld(signal: i32, old_set: &mut libc::sigset_t) {
    unsafe {
        let mut block_set = get_empty_sig_set();

        libc::sigaddset(&mut block_set, libc::SIGALRM);
        libc::sigaddset(&mut block_set, libc::SIGINT);
        libc::sigaddset(&mut block_set, libc::SIGQUIT);
        libc::sigaddset(&mut block_set, libc::SIGHUP);
        libc::sigaddset(&mut block_set, libc::SIGTERM);
        libc::sigaddset(&mut block_set, signal);

        libc::sigaddset(&mut block_set, libc::SIGCHLD);

        if libc::sigprocmask(libc::SIG_BLOCK, &block_set, old_set) != 0 {
            eprintln!("timeout: failed to set block signals mask");
            std::process::exit(125)
        }
    }
}

/// Tries to disable core dumps for current process.
///
/// # Returns
///
/// `true` is successfull, `false` otherwise.
fn disable_core_dumps() -> bool {
    #[cfg(target_os = "linux")]
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } == 0 {
        return true;
    }
    let rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    (unsafe { libc::setrlimit(libc::RLIMIT_CORE, &rlim) } == 0)
}

/// Searches for the executable utility in the directories specified by the `PATH` environment variable.
///
/// # Arguments
///
/// * `utility` - name of the utility to search for.
///
/// # Returns
///
/// `Option<String>` - full path to the utility if found, or `None` if not found.
fn search_in_path(utility: &str) -> Option<String> {
    if let Ok(paths) = std::env::var("PATH") {
        for path in paths.split(':') {
            let full_path = std::path::Path::new(path).join(utility);
            if full_path.is_file() {
                if let Ok(metadata) = std::fs::metadata(&full_path) {
                    // Check if the file is executable
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return Some(full_path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}

/// Main timeout function that creates child and processes its return exit status.
///
/// # Arguments
///
/// `args` - structure of timeout options and operands.
///
/// # Return
///
/// [i32] - exit status code of timeout utility.
fn timeout(args: Args) -> i32 {
    let Args {
        foreground,
        mut preserve_status,
        kill_after,
        signal_name,
        duration,
        utility,
        mut arguments,
    } = args;

    FOREGROUND.store(foreground, Ordering::SeqCst);
    FIRST_SIGNAL.store(signal_name, Ordering::SeqCst);
    *KILL_AFTER.lock().unwrap() = kill_after;

    // Ensures, this process is process leader so all subprocesses can be killed.s
    if !foreground {
        unsafe { libc::setpgid(0, 0) };
    }

    // Setup handlers before to catch signals before fork()
    set_handler(signal_name);
    unsafe {
        libc::signal(libc::SIGTTIN, libc::SIG_IGN);
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
    }
    set_chld();

    // To be able to handle SIGALRM (will be send after timeout)
    unblock_signal(libc::SIGALRM);

    let mut original_set = get_empty_sig_set();
    block_handler_and_chld(signal_name, &mut original_set);

    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            // Restore original mask for child.= process.
            unsafe {
                libc::sigprocmask(
                    libc::SIG_SETMASK,
                    &original_set,
                    std::ptr::null_mut::<libc::sigset_t>(),
                )
            };

            unsafe {
                libc::signal(libc::SIGTTIN, libc::SIG_DFL);
                libc::signal(libc::SIGTTOU, libc::SIG_DFL);
            }

            let utility_path = if Path::new(&utility).is_file() {
                utility.clone()
            } else {
                match search_in_path(&utility) {
                    Some(path) => path,
                    None => {
                        eprintln!("timeout: utility '{utility}' not found");
                        return 127;
                    }
                }
            };

            let utility_c = CString::new(utility_path.clone()).unwrap();
            let mut arguments_c: Vec<CString> = arguments
                .drain(..)
                .map(|arg| CString::new(arg).unwrap())
                .collect();
            arguments_c.insert(0, utility_c.clone());
            match execvp(&utility_c, &arguments_c) {
                Ok(_) => 0,
                Err(Errno::ENOENT) => {
                    eprintln!("timeout: utility '{utility}' not found");
                    127
                }
                Err(_) => {
                    eprintln!("timeout: unable to run the utility '{utility}'");
                    126
                }
            }
        }
        Ok(ForkResult::Parent { child }) => {
            MONITORED_PID.store(child.as_raw(), Ordering::SeqCst);

            set_timeout(duration);

            let mut wait_status: WaitStatus;
            loop {
                match waitpid(
                    child,
                    Some(WaitPidFlag::WNOHANG | WaitPidFlag::WCONTINUED | WaitPidFlag::WUNTRACED),
                ) {
                    Ok(ws) => wait_status = ws,
                    Err(_) => {
                        eprintln!("timeout: failed to wait for child");
                        return 125;
                    }
                }
                match wait_status {
                    WaitStatus::StillAlive | WaitStatus::Continued(_) => {
                        unsafe { libc::sigsuspend(&original_set) };
                    }
                    WaitStatus::Stopped(_, _s) => {
                        send_signal(MONITORED_PID.load(Ordering::SeqCst), libc::SIGCONT);
                        TIMED_OUT.store(true, Ordering::SeqCst);
                    }
                    _ => {
                        break;
                    }
                }
            }
            let status = match wait_status {
                WaitStatus::Exited(_, status) => status,
                WaitStatus::Signaled(_, rec_signal, _) => {
                    if !TIMED_OUT.load(Ordering::SeqCst) && disable_core_dumps() {
                        unsafe { libc::signal(rec_signal as i32, libc::SIG_DFL) };
                        unblock_signal(rec_signal as i32);
                        unsafe { libc::raise(rec_signal as i32) };
                    }
                    if TIMED_OUT.load(Ordering::SeqCst) && rec_signal as i32 == libc::SIGKILL {
                        preserve_status = true;
                    }
                    128 + rec_signal as i32
                }
                _ => 125,
            };

            if TIMED_OUT.load(Ordering::SeqCst) && !preserve_status {
                124
            } else {
                status
            }
        }
        Err(_) => 125,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::try_parse().unwrap_or_else(|err| match err.kind() {
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
            print!("{err}");
            std::process::exit(0);
        }
        _ => {
            eprintln!(
                "timeout: {}",
                err.source()
                    .map_or_else(|| err.kind().to_string(), |err| err.to_string())
            );
            std::process::exit(125);
        }
    });

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = timeout(args);
    std::process::exit(exit_code);
}
