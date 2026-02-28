use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Sandbox {
    pub workspace_root: PathBuf,
    pub overlay_root: PathBuf,
    pub run_dir: PathBuf,
    pub run_id: String,
}

impl Sandbox {
    pub fn new(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let runs_dir = workspace_root.join(".gorkforge").join("runs");
        fs::create_dir_all(&runs_dir).context("create .gorkforge/runs")?;

        let run_id = format!(
            "run_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
        let run_dir = runs_dir.join(&run_id);
        fs::create_dir_all(&run_dir).context("create run dir")?;
        let overlay_root = run_dir.join("overlay");
        fs::create_dir_all(&overlay_root).context("create overlay")?;
        copy_dir_filtered(
            &workspace_root,
            &overlay_root,
            &[".git", "target", ".gorkforge"],
        )?;

        Ok(Self {
            workspace_root,
            overlay_root,
            run_dir,
            run_id,
        })
    }

    pub fn workspace_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.workspace_root.join(rel)
    }

    pub fn overlay_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.overlay_root.join(rel)
    }

    pub fn log_path(&self) -> PathBuf {
        self.run_dir.join("sandbox.log")
    }

    pub fn log(&self, line: &str) -> Result<()> {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())?;
        writeln!(f, "{}", line)?;
        Ok(())
    }

    pub fn refresh_overlay(&self) -> Result<()> {
        if self.overlay_root.exists() {
            fs::remove_dir_all(&self.overlay_root).context("clear overlay")?;
        }
        fs::create_dir_all(&self.overlay_root).context("recreate overlay")?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<String> {
        copy_dir_filtered(&self.overlay_root, &self.workspace_root, &[])?;
        let _ = run_command(&self.workspace_root, "git", &["add", "-A"])?;
        let output = run_command(&self.workspace_root, "git", &["commit", "-m", message])?;
        self.log(&format!("git commit: {}", message))?;
        Ok(output)
    }

    pub fn push(&self, remote: &str, branch: &str) -> Result<String> {
        let output = run_command(&self.workspace_root, "git", &["push", remote, branch])?;
        self.log(&format!("git push: {} {}", remote, branch))?;
        Ok(output)
    }
}

pub fn run_command(cwd: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .context("run command")?;

    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "command failed: {} {}{}",
            program,
            args.join(" "),
            if combined.is_empty() {
                String::new()
            } else {
                format!("\n{}", combined)
            }
        ));
    }

    Ok(combined)
}

fn copy_dir_filtered(src: &Path, dst: &Path, skip: &[&str]) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst).context("create destination")?;
    }

    if !src.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(src).context("read source")? {
        let entry = entry?;
        let _name = entry.file_name();
        let _name = _name.to_string_lossy();

        if skip.iter().any(|s| matches_skip(s, &entry.path(), src)) {
            continue;
        }

        let next_src = entry.path();
        let next_dst = dst.join(entry.file_name());
        let metadata = entry.metadata().context("read entry metadata")?;

        if metadata.is_dir() {
            copy_dir_filtered(&next_src, &next_dst, skip)?;
            continue;
        }

        if let Some(parent) = next_dst.parent() {
            fs::create_dir_all(parent).context("create parent directory")?;
        }

        let mut f = File::open(&next_src).context("open source file")?;
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;

        let mut out = File::create(&next_dst).context("create destination file")?;
        out.write_all(&bytes)?;
    }

    Ok(())
}

fn matches_skip(skip_entry: &str, candidate: &Path, base: &Path) -> bool {
    let rel = candidate.strip_prefix(base).ok();
    if let Some(rel) = rel {
        let rel = rel.to_string_lossy();
        if rel == skip_entry {
            return true;
        }
        let normalized = skip_entry.replace('\\', "/");
        if rel.starts_with(&format!("{}/", normalized)) {
            return true;
        }
    }
    false
}
