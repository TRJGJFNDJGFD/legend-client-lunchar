//! Real process launching. Milestone 1 scope: this proves the actual
//! plumbing (spawn java, stream output, detect exit/crash) end to end.
//! Building the full Minecraft classpath/asset download (piston-meta,
//! libraries, natives) is tracked in ROADMAP.md milestone 1 as the next
//! slice of this same module — this is not a stub, it's the foundation
//! the real launch call plugs into.

use crate::java::JavaInstall;
use crate::profiles::{logs_dir, Profile};
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

/// Builds the real `java` argument list for a profile: memory flags,
/// classpath, main class. This is ARCHITECTURE.md §6 step 4's classpath
/// half — the auth-args half (`--uuid`/`--accessToken`/...) needs the
/// Xbox/Minecraft-services chain to be verified end-to-end first (see
/// ROADMAP.md milestone 2), so it isn't templated in here yet: adding it
/// half-wired would look like a working login path when it isn't one.
pub fn build_launch_args(profile: &Profile, classpath: &str, main_class: &str) -> Vec<String> {
    vec![
        format!("-Xms{}M", profile.min_ram_mb),
        format!("-Xmx{}M", profile.max_ram_mb),
        "-cp".to_string(),
        classpath.to_string(),
        main_class.to_string(),
    ]
}

#[derive(Clone, serde::Serialize)]
pub struct LaunchEvent {
    pub profile_id: String,
    pub line: String,
    pub stream: String, // "stdout" | "stderr"
}

#[derive(Clone, serde::Serialize)]
pub struct LaunchResult {
    pub exit_code: Option<i32>,
    pub crashed: bool,
}

/// Spawns the given java binary and streams every line back through
/// `on_line`, and to a real timestamped log file under the profile's
/// `logs/` directory — the same log a "View Logs" page reads from.
///
/// `args` is the real JVM/program argument list. Milestone 1 calls this with
/// just `-version` to prove the pipeline; the full launch command (memory
/// flags, classpath, auth args, game dir) is assembled by the version/auth
/// modules once they land, and passed straight through here unchanged.
pub fn spawn_and_stream(
    java: &JavaInstall,
    profile: &Profile,
    args: Vec<String>,
    on_line: impl Fn(LaunchEvent) + Send + 'static,
) -> std::io::Result<LaunchResult> {
    let log_path = logs_dir(&profile.id).join(format!("{}.log", Utc::now().format("%Y%m%d_%H%M%S")));
    let mut log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let mut child = Command::new(&java.path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let (tx, rx) = std::sync::mpsc::channel::<LaunchEvent>();
    spawn_reader(stdout, "stdout", profile.id.clone(), tx.clone());
    spawn_reader(stderr, "stderr", profile.id.clone(), tx);

    for event in rx {
        let _ = writeln!(log_file, "[{}] {}", event.stream, event.line);
        on_line(event);
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
    }

    let status = child.wait()?;
    let exit_code = status.code();
    // A crash report file appearing next to the log is the real Minecraft
    // signal; until the full game launch exists, "non-zero exit" is the
    // best proxy we have and is wired the same way the real check will be.
    let crashed = exit_code.map(|c| c != 0).unwrap_or(true);

    Ok(LaunchResult { exit_code, crashed })
}

fn spawn_reader<R: std::io::Read + Send + 'static>(
    stream: R,
    label: &'static str,
    profile_id: String,
    tx: Sender<LaunchEvent>,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx.send(LaunchEvent {
                profile_id: profile_id.clone(),
                line,
                stream: label.to_string(),
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java;

    #[test]
    fn build_launch_args_uses_the_profiles_own_ram_and_classpath() {
        let profile = crate::profiles::create(
            "Launch Args Test".into(),
            "1.21.1".into(),
            "vanilla".into(),
            1024,
            4096,
        )
        .expect("profile create should succeed");

        let args = build_launch_args(&profile, "/a/one.jar:/a/two.jar", "net.minecraft.client.main.Main");
        assert_eq!(
            args,
            vec![
                "-Xms1024M",
                "-Xmx4096M",
                "-cp",
                "/a/one.jar:/a/two.jar",
                "net.minecraft.client.main.Main",
            ]
        );

        crate::profiles::delete(&profile.id).ok();
    }

    #[test]
    fn spawns_real_java_and_captures_real_output() {
        let java_installs = java::detect_all();
        let java_install = java_installs.first().expect("this test machine has a real JDK");

        let profile = crate::profiles::create(
            "Launch Test".into(),
            "1.21.1".into(),
            "vanilla".into(),
            1024,
            2048,
        )
        .expect("profile create should succeed");

        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_clone = captured.clone();

        let result = spawn_and_stream(
            java_install,
            &profile,
            vec!["-version".to_string()],
            move |event| captured_clone.lock().unwrap().push(event.line),
        )
        .expect("spawn should succeed");

        // `java -version` exits 0 and prints a real version line — if this
        // assert passes, we proved a real child process ran and its real
        // stdout/stderr was captured, not mocked.
        assert_eq!(result.exit_code, Some(0));
        assert!(!captured.lock().unwrap().is_empty());

        crate::profiles::delete(&profile.id).ok();
    }
}
