use sevenz_rust2::encoder_options::{EncoderOptions, LZMA2Options};
use sevenz_rust2::{ArchiveEntry, EncoderConfiguration, EncoderMethod, SourceReader};
use std::io::Cursor;
use std::{
    env, fs,
    io::{self, Read},
    path::Path,
    process, vec,
};

/// EasyTier version to embed — must match a release tag from
/// <https://github.com/burningtnt/EasyTier/releases>
const EASYTIER_VERSION: &str = "v2.5.0-terracotta.2";

fn main() {
    println!("cargo::rerun-if-changed=Cargo.toml");
    println!("cargo::rerun-if-changed=build.rs");

    println!("cargo::rustc-env=TERRACOTTA_ET_VERSION={}", EASYTIER_VERSION);

    download_easytier();
}

fn download_easytier() {
    struct EasytierFiles {
        url: &'static str,
        entry: &'static str,
        cli: &'static str,
        desc: &'static str,
    }

    let version = EASYTIER_VERSION;

    let target_os = get_var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = get_var("CARGO_CFG_TARGET_ARCH").unwrap();

    let conf = match (target_os.as_str(), target_arch.as_str()) {
        ("windows", "x86_64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-windows-x86_64-{V}.zip",
            entry: "easytier-core.exe",
            cli: "easytier-cli.exe",
            desc: "windows-x86_64",
        },
        ("windows", "aarch64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-windows-arm64-{V}.zip",
            entry: "easytier-core.exe",
            cli: "easytier-cli.exe",
            desc: "windows-arm64",
        },
        ("linux", "x86_64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-linux-x86_64-{V}.zip",
            entry: "easytier-core",
            cli: "easytier-cli",
            desc: "linux-x86_64",
        },
        ("linux", "aarch64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-linux-aarch64-{V}.zip",
            entry: "easytier-core",
            cli: "easytier-cli",
            desc: "linux-arm64",
        },
        ("linux", "riscv64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-linux-riscv64-{V}.zip",
            entry: "easytier-core",
            cli: "easytier-cli",
            desc: "linux-riscv64",
        },
        ("linux", "loongarch64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-linux-loongarch64-{V}.zip",
            entry: "easytier-core",
            cli: "easytier-cli",
            desc: "linux-loongarch64",
        },
        ("macos", "x86_64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-macos-x86_64-{V}.zip",
            entry: "easytier-core",
            cli: "easytier-cli",
            desc: "macos-x86_64",
        },
        ("macos", "aarch64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-macos-aarch64-{V}.zip",
            entry: "easytier-core",
            cli: "easytier-cli",
            desc: "macos-arm64",
        },
        ("freebsd", "x86_64") => EasytierFiles {
            url: "https://github.com/burningtnt/EasyTier/releases/download/{V}/easytier-freebsd-13.2-x86_64-{V}.zip",
            entry: "easytier-core",
            cli: "easytier-cli",
            desc: "freebsd-x86_64",
        },
        _ => panic!(
            "Cannot compile terracotta-server on {}-{}: no EasyTier binary available.",
            target_os, target_arch
        ),
    };

    println!("cargo::rerun-if-changed=.easytier");

    let base = Path::new(&get_var("CARGO_MANIFEST_DIR").unwrap())
        .join(".easytier")
        .join(version)
        .join(conf.desc);
    let entry_conf = base.join("entry-conf.v1.txt");
    let cli_conf = base.join("cli-conf.v1.txt");
    let entry_archive = base.join("easytier.7z");

    println!(
        "cargo::rustc-env=TERRACOTTA_ET_ENTRY_CONF={}",
        entry_conf.as_path().to_str().unwrap()
    );
    println!(
        "cargo::rustc-env=TERRACOTTA_ET_CLI_CONF={}",
        cli_conf.as_path().to_str().unwrap()
    );
    println!(
        "cargo::rustc-env=TERRACOTTA_ET_ARCHIVE={}",
        entry_archive.as_path().to_str().unwrap()
    );

    // If already cached, skip download
    if fs::metadata(&entry_conf).is_ok() {
        return;
    }

    // Clean and recreate cache directory
    if fs::metadata(&base).is_ok() {
        fs::remove_dir_all(&base).unwrap();
    }
    fs::create_dir_all(&base).unwrap();

    eprintln!(
        "Downloading EasyTier {} for {} ...",
        version, conf.desc
    );

    let manifest_dir_value = get_var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir_value);
    let local_source = manifest_dir.join(format!("easytier-{}-{}.zip", conf.desc, version));
    let source = if fs::metadata(&local_source).is_ok() {
        eprintln!("Using local EasyTier archive: {}", local_source.display());
        local_source.clone()
    } else {
        let source = Path::new(&env::temp_dir())
            .join(format!("terracotta-server-build-{}.zip", process::id()));

        reqwest::blocking::get(conf.url.replace("{V}", version))
            .unwrap()
            .copy_to(&mut io::BufWriter::new(
                fs::File::create(&source).unwrap(),
            ))
            .inspect_err(|_| {
                let _ = fs::remove_file(&source);
            })
            .unwrap();
        source
    };

    let mut archive = zip::ZipArchive::new(fs::File::open(&source).unwrap()).unwrap();
    let target = base.join("easytier.7z.tmp");
    let mut writer =
        sevenz_rust2::ArchiveWriter::new(fs::File::create(&target).unwrap()).unwrap();
    writer.set_content_methods(vec![
        EncoderConfiguration {
            method: EncoderMethod::LZMA2,
            options: Some(EncoderOptions::LZMA2(LZMA2Options::from_level(9))),
        },
        EncoderConfiguration {
            method: match target_arch.as_str() {
                "x86_64" => EncoderMethod::BCJ_X86_FILTER,
                "aarch64" => EncoderMethod::BCJ_ARM64_FILTER,
                "riscv64" => EncoderMethod::BCJ_RISCV_FILTER,
                _ => EncoderMethod::COPY,
            },
            options: None,
        },
    ]);

    let mut archive_entries: Vec<ArchiveEntry> = vec![];
    let mut archive_readers: Vec<SourceReader<Cursor<Vec<u8>>>> = vec![];
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        if !entry.is_file() {
            continue;
        }
        let mut buf: Vec<u8> = vec![];
        entry.read_to_end(&mut buf).unwrap();

        // Strip all directory prefixes so files land flat in the extraction dir.
        let file_name = entry
            .enclosed_name()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        archive_entries.push(ArchiveEntry::new_file(&file_name));
        archive_readers.push(SourceReader::new(Cursor::new(buf)));
    }
    writer
        .push_archive_entries(archive_entries, archive_readers)
        .unwrap();
    writer.finish().unwrap();

    // Atomic rename
    let r = fs::rename(&target, &entry_archive);
    if fs::metadata(&entry_archive).is_err() {
        r.unwrap();
    }
    fs::write(&entry_conf, conf.entry).unwrap();
    fs::write(&cli_conf, conf.cli).unwrap();

    // Clean up temp zip only when we downloaded one ourselves.
    if source != local_source {
        let _ = fs::remove_file(&source);
    }

    eprintln!("EasyTier {} downloaded and archived.", version);
}

fn get_var<K: AsRef<std::ffi::OsStr>>(key: K) -> Result<String, env::VarError> {
    println!(
        "cargo::rerun-if-env-changed={}",
        key.as_ref().to_string_lossy()
    );
    env::var(key.as_ref())
}
