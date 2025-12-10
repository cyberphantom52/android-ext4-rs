use android_ext4::{DirectoryWalker, EntryAttributes, FileType, Volume, WalkItem};
use clap::Parser;
use indicatif::ProgressBar;
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
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

    /// Number of threads to use for extraction (defaults to num_cpus / 4)
    #[arg(short = 't', long, default_value_t = num_cpus())]
    num_threads: usize,
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().div_ceil(4))
        .unwrap_or(1)
}

fn default_output_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    PathBuf::from(format!("output-{}", timestamp))
}

/// Main extractor
struct Extractor<F: Fn() -> BufReader<File> + Sync + Send> {
    volume: Volume<BufReader<File>, F>,
    arguments: Arguments,
    mount_name: String,
    fsconfig: BufWriter<File>,
    contexts: BufWriter<File>,
}

impl<F: Fn() -> BufReader<File> + Sync + Send> Extractor<F> {
    fn new(reader_factory: F, arguments: Arguments) -> io::Result<Self> {
        let volume = Volume::new(reader_factory)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}", e)))?;
        let mount_name = volume.name().unwrap_or(
            arguments
                .image
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string(),
        );

        let config_dir = arguments.output_dir.join("config");
        let extract_dir = arguments.output_dir.join(&mount_name);

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&extract_dir)?;

        let fsconfig = BufWriter::new(File::create(
            config_dir.join(format!("{}_fs_config", mount_name)),
        )?);
        let contexts = BufWriter::new(File::create(
            config_dir.join(format!("{}_file_contexts", mount_name)),
        )?);

        Ok(Self {
            volume,
            arguments,
            mount_name,
            fsconfig,
            contexts,
        })
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
        let items: Vec<WalkItem> = DirectoryWalker::from_path(&self.volume, "/")
            .map_err(|e| io::Error::other(format!("Walker error: {}", e)))?
            .par_bridge()
            .filter_map(Result::ok)
            .collect();

        spinner.finish_with_message(format!("Found {} entries", items.len()));

        // Process entries
        let pb = self.create_progress_bar(items.len() as u64, "Extracting");

        let attributes: Vec<(PathBuf, EntryAttributes)> = items
            .into_par_iter()
            .filter_map(|item| {
                pb.inc(1);
                self.process_item(&item).ok()
            })
            .collect();

        // Add root entries
        writeln!(self.fsconfig, "/ 0 0 0755")?;
        writeln!(self.fsconfig, "{} 0 0 0755", self.mount_name)?;

        for (path, attr) in attributes {
            let fs_path = format!("{}{}", self.mount_name, path.display());
            let escaped = escape_regex(&fs_path);

            // fs_config
            writeln!(
                self.fsconfig,
                "{} {} {} {}",
                fs_path,
                attr.uid(),
                attr.gid(),
                attr.mode_with_caps()
            )?;

            // SELinux contexts
            if let Some(selabel) = attr.selinux() {
                writeln!(self.contexts, "/{} {}", escaped, selabel)?;
                // directory
                if matches!(attr.mode().file_type(), Some(FileType::Directory)) {
                    writeln!(self.contexts, "/{}(/.*)? {}", escaped, selabel)?;
                }
            }

            // System-as-Root (SAR)
            if matches!(attr.mode().file_type(), Some(FileType::RegularFile))
                && path.to_string_lossy().contains("/system/build.prop")
            {
                let dir_escaped = escape_regex(&self.mount_name);
                writeln!(self.contexts, "/{} u:object_r:rootfs:s0", dir_escaped)?;
                writeln!(self.contexts, "/{}(/.*)? u:object_r:rootfs:s0", dir_escaped)?;
            }
        }

        self.fsconfig.flush()?;
        self.contexts.flush()?;

        pb.finish_with_message("Extraction complete");

        if !self.arguments.quiet {
            eprintln!("✓ Extraction completed successfully!");
            eprintln!("  Output: {}", self.arguments.output_dir.display());
        }

        Ok(())
    }

    fn process_item(&self, item: &WalkItem) -> io::Result<(PathBuf, EntryAttributes)> {
        let path = item.path();

        let extract_dir = self.extract_dir();
        let target = extract_dir.join(path.strip_prefix("/").unwrap_or(path));

        if let Some(parent) = target.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }

        match item.r#type() {
            FileType::RegularFile => {
                let mut file = File::create(&target)?;
                let mut file_reader = self.volume.open_file(item.path()).map_err(|e| {
                    io::Error::other(format!(
                        "Failed to open file {}: {}",
                        item.path().display(),
                        e
                    ))
                })?;
                io::copy(&mut file_reader, &mut file)?;
            }
            FileType::SymbolicLink => {
                let mut file_reader = self.volume.open_file(item.path()).map_err(|e| {
                    io::Error::other(format!(
                        "Failed to read symlink {}: {}",
                        item.path().display(),
                        e
                    ))
                })?;

                let mut link_target = String::new();
                file_reader.read_to_string(&mut link_target).map_err(|e| {
                    io::Error::other(format!(
                        "Failed to read symlink target {}: {}",
                        item.path().display(),
                        e
                    ))
                })?;

                let _ = fs::remove_file(&target);
                Self::create_symlink(&link_target, &target);
            }
            FileType::Directory => fs::create_dir_all(&target)?,
            _ => {}
        }

        Ok((item.path().to_owned(), item.attributes().clone()))
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

    rayon::ThreadPoolBuilder::new()
        .num_threads(args.num_threads)
        .build_global()
        .unwrap();

    if !args.quiet {
        eprintln!("Using {} threads for extraction", args.num_threads);
    }

    // Create an Arc-wrapped path for the reader factory
    let image_path = Arc::new(args.image.clone());

    Extractor::new(
        move || {
            let file = File::open(image_path.as_ref()).expect("Failed to open image file");
            BufReader::new(file)
        },
        args,
    )?
    .run()?;

    Ok(())
}
