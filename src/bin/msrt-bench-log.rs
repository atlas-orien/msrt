use std::{
    env, fs,
    fs::File,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("msrt bench log error: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let log_path = log_path()?;
    let mut command = Command::new("cargo");
    command.arg("bench").arg("--bench").arg("protocol");
    command.args(&args);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing cargo bench stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing cargo bench stderr"))?;

    let stdout_log = log_path.clone();
    let stdout_thread = thread::spawn(move || tee_reader(stdout, stdout_log, false));

    let stderr_log = log_path.clone();
    let stderr_thread = thread::spawn(move || tee_reader(stderr, stderr_log, true));

    let status = child.wait()?;
    join_thread(stdout_thread)?;
    join_thread(stderr_thread)?;

    println!("cargo bench log written to {}", log_path.display());

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "cargo bench exited with {status}"
        )))
    }
}

fn log_path() -> io::Result<PathBuf> {
    let dir = Path::new("log");
    fs::create_dir_all(dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();

    Ok(dir.join(format!("cargo-bench-{timestamp}.log")))
}

fn tee_reader<R>(reader: R, log_path: PathBuf, stderr: bool) -> io::Result<()>
where
    R: io::Read,
{
    let mut log = File::options().create(true).append(true).open(log_path)?;

    for line in BufReader::new(reader).lines() {
        let line = line?;
        if stderr {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
        writeln!(log, "{line}")?;
    }

    Ok(())
}

fn join_thread(thread: thread::JoinHandle<io::Result<()>>) -> io::Result<()> {
    thread
        .join()
        .map_err(|_| io::Error::other("bench log thread panicked"))?
}
