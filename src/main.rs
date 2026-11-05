use std::fs::{self, File, FileTimes, OpenOptions};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone};
use clap::Parser;
use exif::{In, Tag, Value};
use walkdir::WalkDir;

/// Check that the EXIF date of .jpg files matches their file created/modified dates.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Directory to scan recursively
    dir: PathBuf,

    /// Set the file's created/modified dates to the EXIF date instead of only listing discrepancies
    #[arg(long)]
    update: bool,
}

#[derive(Default)]
struct Stats {
    scanned: usize,
    no_exif: usize,
    in_sync: usize,
    mismatched: usize,
    updated: usize,
    errors: usize,
}

fn main() -> ExitCode {
    let args = Args::parse();

    if !args.dir.is_dir() {
        eprintln!("error: '{}' is not a directory", args.dir.display());
        return ExitCode::from(2);
    }

    let mut stats = Stats::default();

    for entry in WalkDir::new(&args.dir) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("error: {e}");
                stats.errors += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() || !is_jpeg(entry.path()) {
            continue;
        }
        stats.scanned += 1;
        if let Err(e) = process(entry.path(), args.update, &mut stats) {
            eprintln!("error: {}: {e}", entry.path().display());
            stats.errors += 1;
        }
    }

    println!();
    println!("Scanned:    {}", stats.scanned);
    println!("No EXIF:    {}", stats.no_exif);
    println!("In sync:    {}", stats.in_sync);
    println!("Mismatched: {}", stats.mismatched);
    if args.update {
        println!("Updated:    {}", stats.updated);
    }
    println!("Errors:     {}", stats.errors);

    if stats.errors > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn is_jpeg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
}

fn process(path: &Path, update: bool, stats: &mut Stats) -> Result<(), Box<dyn std::error::Error>> {
    let Some(exif_time) = read_exif_date(path) else {
        stats.no_exif += 1;
        return Ok(());
    };

    let meta = fs::metadata(path)?;
    let modified: DateTime<Local> = meta.modified()?.into();
    let created: Option<DateTime<Local>> = meta.created().ok().map(Into::into);

    // EXIF dates have one-second resolution, so compare whole seconds only.
    let exif_secs = exif_time.timestamp();
    let modified_ok = modified.timestamp() == exif_secs;
    let created_ok = created.is_none_or(|c| c.timestamp() == exif_secs);

    if modified_ok && created_ok {
        stats.in_sync += 1;
        return Ok(());
    }

    stats.mismatched += 1;
    println!("{}", path.display());
    println!("    exif:     {}", fmt(exif_time.with_timezone(&Local)));
    if let Some(c) = created {
        println!("    created:  {}{}", fmt(c), if created_ok { "" } else { "  <-- differs" });
    }
    println!("    modified: {}{}", fmt(modified), if modified_ok { "" } else { "  <-- differs" });

    if update {
        set_file_times(path, SystemTime::from(exif_time))?;
        stats.updated += 1;
        println!("    updated");
    }
    Ok(())
}

fn fmt(t: DateTime<Local>) -> String {
    t.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Returns DateTimeOriginal (falling back to DateTimeDigitized, then DateTime).
/// EXIF times are local unless an offset tag is present.
fn read_exif_date(path: &Path) -> Option<DateTime<FixedOffset>> {
    let file = File::open(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .ok()?;

    let candidates = [
        (Tag::DateTimeOriginal, Tag::OffsetTimeOriginal),
        (Tag::DateTimeDigitized, Tag::OffsetTimeDigitized),
        (Tag::DateTime, Tag::OffsetTime),
    ];

    for (date_tag, offset_tag) in candidates {
        let Some(field) = exif.get_field(date_tag, In::PRIMARY) else {
            continue;
        };
        let Value::Ascii(ref v) = field.value else {
            continue;
        };
        let Some(raw) = v.first() else { continue };
        let Ok(mut dt) = exif::DateTime::from_ascii(raw) else {
            continue;
        };
        if let Some(of) = exif.get_field(offset_tag, In::PRIMARY) {
            if let Value::Ascii(ref ov) = of.value {
                if let Some(o) = ov.first() {
                    let _ = dt.parse_offset(o);
                }
            }
        }
        if let Some(t) = to_datetime(&dt) {
            return Some(t);
        }
    }
    None
}

fn to_datetime(dt: &exif::DateTime) -> Option<DateTime<FixedOffset>> {
    let naive = NaiveDate::from_ymd_opt(dt.year.into(), dt.month.into(), dt.day.into())?
        .and_hms_opt(dt.hour.into(), dt.minute.into(), dt.second.into())?;
    match dt.offset {
        Some(minutes) => FixedOffset::east_opt(i32::from(minutes) * 60)?
            .from_local_datetime(&naive)
            .single(),
        None => Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|t| t.fixed_offset()),
    }
}

fn set_file_times(path: &Path, time: SystemTime) -> std::io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    #[allow(unused_mut)]
    let mut times = FileTimes::new().set_modified(time);
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTimesExt;
        times = times.set_created(time);
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::darwin::fs::FileTimesExt;
        times = times.set_created(time);
    }
    file.set_times(times)
}
