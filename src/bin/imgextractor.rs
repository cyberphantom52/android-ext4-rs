use android_ext4::{DirectoryWalker, FileType, Volume, WalkItem};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::symlink;

/// Android ext4 image extractor
#[derive(Parser, Debug)]
#[command(name = "imgextractor.rs")]
#[command(
    author = "Inam Ul Haq",
    version = "1.0",
    about,
    long_about = None
)]
#[command(arg_required_else_help = true)]
struct Arguments {
    /// Path to the raw ext4 image
    image: PathBuf,

    /// Output directory (defaults to output-{timestamp})
    #[arg(short, long, default_value=default_output_path().into_os_string())]
    output_dir: PathBuf,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Suppress progress bars
    #[arg(short, long)]
    quiet: bool,
}

fn default_output_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    PathBuf::from(format!("output-{}", timestamp))
}

/// Path information for an entry
struct EntryPaths {
    /// Path with spaces replaced by underscores
    safe: String,
    /// Full filesystem path (mount_name + safe_path)
    fs_path: String,
}

impl EntryPaths {
    fn new(path_str: &str, mount_name: &str) -> Self {
        let safe = path_str.replace(' ', "_");
        let fs_path = format!("{}{}", mount_name, safe);
        Self { safe, fs_path }
    }

    fn target_path(&self, base: &std::path::Path) -> PathBuf {
        base.join(self.safe.trim_start_matches('/'))
    }
}

/// File handles for config output
struct ConfigWriters {
    fsconfig: BufWriter<File>,
    contexts: BufWriter<File>,
}

/// Main extractor
struct Extractor<R: Read + Seek> {
    volume: Volume<R>,
    arguments: Arguments,
    mount_name: String,
}

impl<R: Read + Seek> Extractor<R> {
    fn new(reader: R, arguments: Arguments) -> io::Result<Self> {
        let volume = Volume::new(reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}", e)))?;
        let mount_name = volume.name();
        let mount_name = if mount_name.is_empty() {
            arguments
                .image
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            mount_name
        };

        let extractor = Self {
            volume,
            arguments,
            mount_name,
        };

        fs::create_dir_all(extractor.config_dir())?;
        fs::create_dir_all(extractor.extract_dir())?;

        Ok(extractor)
    }

    fn config_dir(&self) -> PathBuf {
        self.arguments.output_dir.join("config")
    }

    fn extract_dir(&self) -> PathBuf {
        self.arguments.output_dir.join(&self.mount_name)
    }

    fn create_progress_bar(&self, len: u64, msg: &str) -> ProgressBar {
        if self.arguments.quiet {
            return ProgressBar::hidden();
        }

        let pb = ProgressBar::new(len);

        pb.set_message(msg.to_string());
        pb
    }

    fn create_spinner(&self, msg: &str) -> ProgressBar {
        if self.arguments.quiet {
            return ProgressBar::hidden();
        }

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb
    }

    fn run(mut self) -> io::Result<()> {
        if !self.arguments.quiet {
            eprintln!("Volume: {}", self.mount_name);
        }

        let spinner = self.create_spinner("Scanning filesystem...");

        // Collect all entries first
        let items: Vec<WalkItem> = DirectoryWalker::from_path(&mut self.volume, "/")
            .map_err(|e| io::Error::other(format!("Walker error: {}", e)))?
            .filter_map(Result::ok)
            .collect();

        spinner.finish_with_message(format!("Found {} entries", items.len()));

        // Open config file handles
        let config_dir = self.config_dir();
        let mut writers = ConfigWriters {
            fsconfig: BufWriter::new(File::create(
                config_dir.join(format!("{}_fs_config", self.mount_name)),
            )?),
            contexts: BufWriter::new(File::create(
                config_dir.join(format!("{}_file_contexts", self.mount_name)),
            )?),
        };

        // Add root entries
        writeln!(writers.fsconfig, "/ 0 0 0755")?;
        writeln!(writers.fsconfig, "{} 0 0 0755", self.mount_name)?;

        // Process entries
        let pb = self.create_progress_bar(items.len() as u64, "Extracting");

        for item in &items {
            self.process_item(&item, &mut writers)?;
            pb.inc(1);
        }

        writers.fsconfig.flush()?;
        writers.contexts.flush()?;

        pb.finish_with_message("Extraction complete");

        if !self.arguments.quiet {
            eprintln!("✓ Extraction completed successfully!");
            eprintln!("  Output: {}", self.arguments.output_dir.display());
        }

        Ok(())
    }

    fn process_item(&mut self, item: &WalkItem, writers: &mut ConfigWriters) -> io::Result<()> {
        let path_str = item.path().to_string_lossy();

        let paths = EntryPaths::new(&path_str, &self.mount_name);
        let meta = item.attributes();

        // Detect System-as-Root (SAR) and write context entries
        if matches!(item.r#type(), FileType::RegularFile) && path_str.contains("/system/build.prop")
        {
            let dir_escaped = escape_regex(&self.mount_name);
            writeln!(writers.contexts, "/{} u:object_r:rootfs:s0", dir_escaped)?;
            writeln!(
                writers.contexts,
                "/{}(/.*)? u:object_r:rootfs:s0",
                dir_escaped
            )?;
        }

        let extract_dir = self.extract_dir();
        let escaped = escape_regex(&paths.fs_path);
        let target = paths.target_path(&extract_dir);

        if let Some(parent) = target.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        match item.r#type() {
            FileType::RegularFile => {
                let mut file = File::create(target)?;
                let mut file_reader = self.volume.open_file(item.path()).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to open file {}: {}", item.path().display(), e),
                    )
                })?;
                io::copy(&mut file_reader, &mut file)?;
            }
            FileType::Directory => {
                fs::create_dir_all(target)?;
                if let Some(selabel) = meta.selinux() {
                    writeln!(writers.contexts, "/{}(/.*)? {}", escaped, selabel)?;
                }
            }
            FileType::SymbolicLink => {
                let link_target = self.volume.read_symlink(item.inode()).ok();

                let _ = fs::remove_file(&target);

                if let Some(link) = link_target {
                    Self::create_symlink(&link, &target);
                }
            }
            _ => {}
        }

        let mode = meta.mode_with_caps();
        writeln!(
            writers.fsconfig,
            "{} {} {} {}",
            paths.fs_path,
            meta.uid(),
            meta.gid(),
            mode
        )?;
        if let Some(selabel) = meta.selinux() {
            writeln!(writers.contexts, "/{} {}", escaped, selabel)?;
        }

        Ok(())
    }

    #[cfg(unix)]
    fn create_symlink(link: &str, target: &PathBuf) {
        let _ = symlink(link, target);
    }

    #[cfg(windows)]
    fn create_symlink(link: &str, target: &PathBuf) {
        if let Ok(mut file) = File::create(target) {
            let marker = b"!<symlink>\xff\xfe";
            let _ = file.write_all(marker);
            for c in link.encode_utf16() {
                let _ = file.write_all(&c.to_le_bytes());
            }
            let _ = file.write_all(&[0, 0]);
        }
    }
}

/// Escape special regex characters for file_contexts
fn escape_regex(s: &str) -> String {
    const SPECIAL: &[char] = &[
        '\\', '^', '$', '.', '|', '?', '*', '+', '(', ')', '{', '}', '[', ']',
    ];
    let mut result = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        if SPECIAL.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();

    if !args.image.exists() {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Image file not found: {}", args.image.display()),
        )));
    }

    let file = File::open(&args.image)?;

    Extractor::new(BufReader::new(file), args)?.run()?;

    Ok(())
}
