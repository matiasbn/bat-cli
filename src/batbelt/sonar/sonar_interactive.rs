use crate::batbelt::git::GitCommit;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::metadata::traits_metadata::TraitMetadata;
use crate::batbelt::metadata::{BatMetadataParser, BatMetadataType};
use crate::batbelt::path::BatFolder;
use crate::batbelt::sonar::{BatSonarError, SonarResultType};
use crate::batbelt::BatEnumerator;

use colored::Colorize;
use dialoguer::console::{style, Emoji};
use error_stack::{Result, ResultExt};
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};

use std::thread;
use std::time::{Duration, Instant};

static BAT: Emoji<'_, '_> = Emoji("🦇", "BatSonar");
static FOLDER: Emoji<'_, '_> = Emoji("📂", "Program Folder");
static WAVE: Emoji<'_, '_> = Emoji("〰", "-");
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum BatSonarInteractive {
    SonarStart { sonar_result_type: SonarResultType },
    ParseMetadata,
}

impl BatSonarInteractive {
    pub fn print_interactive(&self) -> Result<(), BatSonarError> {
        match self {
            BatSonarInteractive::SonarStart { sonar_result_type } => {
                self.sonar_start(*sonar_result_type)?
            }
            BatSonarInteractive::ParseMetadata => self.parse_metadata()?,
        }
        Ok(())
    }

    fn sonar_start(&self, sonar_result_type: SonarResultType) -> Result<(), BatSonarError> {
        let pb = ProgressBar::new_spinner();
        let result_type_colorized = match sonar_result_type {
            SonarResultType::Function => BatMetadataType::Function.get_colored_name(true),
            SonarResultType::Struct => BatMetadataType::Struct.get_colored_name(true),
            SonarResultType::Trait => BatMetadataType::Trait.get_colored_name(true),
            _ => sonar_result_type.to_string().bright_cyan(),
        };
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.blue} {msg}")
                .unwrap()
                .tick_strings(&[
                    &format!("{}                  {}{}", FOLDER, WAVE, BAT),
                    &format!("{}                {}  {}", FOLDER, WAVE, BAT),
                    &format!("{}              {}    {}", FOLDER, WAVE, BAT),
                    &format!("{}            {}      {}", FOLDER, WAVE, BAT),
                    &format!("{}          {}        {}", FOLDER, WAVE, BAT),
                    &format!("{}        {}          {}", FOLDER, WAVE, BAT),
                    &format!("{}      {}            {}", FOLDER, WAVE, BAT),
                    &format!("{}    {}              {}", FOLDER, WAVE, BAT),
                    &format!("{}  {}                {}", FOLDER, WAVE, BAT),
                    &format!("{}{}                  {}", FOLDER, WAVE, BAT),
                    &format!("{}  {}                {}", FOLDER, WAVE, BAT),
                    &format!("{}    {}              {}", FOLDER, WAVE, BAT),
                    &format!("{}      {}            {}", FOLDER, WAVE, BAT),
                    &format!("{}        {}          {}", FOLDER, WAVE, BAT),
                    &format!("{}          {}        {}", FOLDER, WAVE, BAT),
                    &format!("{}            {}      {}", FOLDER, WAVE, BAT),
                    &format!("{}              {}    {}", FOLDER, WAVE, BAT),
                    &format!("{}                {}  {}", FOLDER, WAVE, BAT),
                    &format!("{}                  {}{}", FOLDER, WAVE, BAT),
                    &format!("{} {}", FOLDER, BAT),
                ]),
        );
        pb.set_message(format!(
            "Looking for {} with {}...",
            result_type_colorized,
            "BatSonar".red(),
        ));
        thread::sleep(Duration::from_millis(1800));
        pb.finish_with_message(format!("{} search finished ", result_type_colorized,));
        Ok(())
    }

    fn parse_metadata(&self) -> Result<(), BatSonarError> {
        let started = Instant::now();
        let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files_dir_entries(false, None, None)
            .change_context(BatSonarError)?;
        println!(
            "Analizing {} files",
            style(format!("{}", program_dir_entries.len())).bold().dim(),
        );
        let m = MultiProgress::new();
        let metadata_types_vec = BatMetadataType::get_type_vec();
        let metadata_types_colored = BatMetadataType::get_colorized_type_vec(true);
        let handles: Vec<_> = (0..metadata_types_vec.len())
            .map(|i| {
                let mut structs_result = vec![];
                let mut functions_result = vec![];
                let mut traits_result = vec![];
                let program_dir_clone = program_dir_entries.clone();
                let metadata_type = metadata_types_vec[i];
                let metadata_type_color = metadata_types_colored[i].clone();
                let pb = m.add(ProgressBar::new(program_dir_clone.len() as u64));
                let mut total = 0;
                pb.set_style(spinner_style.clone());
                thread::spawn(move || {
                    for (idx, entry) in program_dir_clone.iter().enumerate().clone() {
                        pb.set_prefix(format!("[{}/{}]", idx + 1, program_dir_clone.len()));
                        pb.set_message(format!(
                            "Getting {} from: {}",
                            metadata_type_color.clone(),
                            entry.clone().path().to_str().unwrap().clone()
                        ));
                        match metadata_type {
                            BatMetadataType::Struct => {
                                let mut struct_res =
                                    StructMetadata::create_metadata_from_dir_entry(entry.clone())
                                        .unwrap();
                                total += struct_res.len();
                                structs_result.append(&mut struct_res);
                            }
                            BatMetadataType::Function => {
                                let mut func_res =
                                    FunctionMetadata::create_metadata_from_dir_entry(entry.clone())
                                        .unwrap();
                                total += func_res.len();
                                functions_result.append(&mut func_res);
                            }
                            BatMetadataType::Trait => {
                                let mut trait_res =
                                    TraitMetadata::create_metadata_from_dir_entry(entry.clone())
                                        .unwrap();

                                total += trait_res.len();
                                traits_result.append(&mut trait_res);
                            }
                        };
                        pb.inc(1);
                        thread::sleep(Duration::from_millis(200));
                    }
                    pb.finish_with_message(format!("{} {} found", total, metadata_type_color));
                    StructMetadata::update_markdown_from_metadata_vec(&mut structs_result).unwrap();
                    FunctionMetadata::update_markdown_from_metadata_vec(&mut functions_result)
                        .unwrap();
                    TraitMetadata::update_markdown_from_metadata_vec(&mut traits_result).unwrap();
                })
            })
            .collect();
        for h in handles {
            let _ = h.join();
        }
        // m.clear().unwrap();

        println!("{} Done in {}", SPARKLE, HumanDuration(started.elapsed()));

        GitCommit::UpdateMetadata {
            metadata_type: BatMetadataType::Struct,
        }
        .create_commit()
        .change_context(BatSonarError)?;

        GitCommit::UpdateMetadata {
            metadata_type: BatMetadataType::Function,
        }
        .create_commit()
        .change_context(BatSonarError)?;

        GitCommit::UpdateMetadata {
            metadata_type: BatMetadataType::Trait,
        }
        .create_commit()
        .change_context(BatSonarError)?;

        Ok(())
    }
    // }
}

#[cfg(test)]
mod sonar_interactive_test {
    use super::*;

    use dialoguer::console::{style, Emoji, Style, Term};
    use indicatif::{
        HumanDuration, MultiProgress, ProgressBar, ProgressIterator, ProgressState, ProgressStyle,
    };
    use rand::seq::SliceRandom;
    use rand::{thread_rng, Rng};
    use std::cmp::min;
    use std::fmt::Write;
    use std::io::{BufRead, BufReader};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};
    use std::{process, thread};

    static PACKAGES: &[&str] = &[
        "fs-events",
        "my-awesome-module",
        "emoji-speaker",
        "wrap-ansi",
        "stream-browserify",
        "acorn-dynamic-import",
    ];

    static COMMANDS: &[&str] = &[
        "cmake .",
        "make",
        "make clean",
        "gcc foo.c -o foo",
        "gcc bar.c -o bar",
        "./helper.sh rebuild-cache",
        "make all-clean",
        "make test",
    ];

    static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍  ", "");
    static TRUCK: Emoji<'_, '_> = Emoji("🚚  ", "");
    static CLIP: Emoji<'_, '_> = Emoji("🔗  ", "");
    static PAPER: Emoji<'_, '_> = Emoji("📃  ", "");
    static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");
    #[test]
    fn test_yarnish() {
        let mut rng = rand::thread_rng();
        let started = Instant::now();
        let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

        println!(
            "{} {}Resolving packages...",
            style("[1/4]").bold().dim(),
            LOOKING_GLASS
        );
        println!(
            "{} {}Fetching packages...",
            style("[2/4]").bold().dim(),
            TRUCK
        );

        println!(
            "{} {}Linking dependencies...",
            style("[3/4]").bold().dim(),
            CLIP
        );
        let deps = 1232;
        let pb = ProgressBar::new(deps);
        for _ in 0..deps {
            pb.inc(1);
            thread::sleep(Duration::from_millis(3));
        }
        pb.finish_and_clear();

        println!(
            "{} {}Building fresh packages...",
            style("[4/4]").bold().dim(),
            PAPER
        );
        let m = MultiProgress::new();
        let handles: Vec<_> = (0..4u32)
            .map(|i| {
                let count = rng.gen_range(30..80);
                let pb = m.add(ProgressBar::new(count));
                pb.set_style(spinner_style.clone());
                pb.set_prefix(format!("[{}/?]", i + 1));
                thread::spawn(move || {
                    let mut rng = rand::thread_rng();
                    let pkg = PACKAGES.choose(&mut rng).unwrap();
                    for _ in 0..count {
                        let cmd = COMMANDS.choose(&mut rng).unwrap();
                        pb.set_message(format!("{pkg}: {cmd}"));
                        pb.inc(1);
                        thread::sleep(Duration::from_millis(rng.gen_range(25..200)));
                    }
                    pb.finish_with_message("waiting...");
                })
            })
            .collect();
        for h in handles {
            let _ = h.join();
        }
        // m.clear().unwrap();

        println!("{} Done in {}", SPARKLE, HumanDuration(started.elapsed()));
    }

    #[test]
    fn test_download() {
        let mut downloaded = 0;
        let total_size = 231231231;

        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
                .progress_chars("#>-"));

        while downloaded < total_size {
            let new = min(downloaded + 223211, total_size);
            downloaded = new;
            pb.set_position(new);
            thread::sleep(Duration::from_millis(12));
        }

        pb.finish_with_message("downloaded");
    }

    #[test]
    fn test_cargowrap() {
        let started = Instant::now();

        println!("Compiling package in release mode...");

        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(200));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.dim.bold} cargo: {wide_msg}")
                .unwrap()
                .tick_chars("/|\\- "),
        );

        let mut p = process::Command::new("cargo")
            .arg("build")
            .arg("--release")
            .stderr(process::Stdio::piped())
            .spawn()
            .unwrap();

        for line in BufReader::new(p.stderr.take().unwrap()).lines() {
            let line = line.unwrap();
            let stripped_line = line.trim();
            if !stripped_line.is_empty() {
                pb.set_message(stripped_line.to_owned());
            }
            pb.tick();
        }

        p.wait().unwrap();

        pb.finish_and_clear();

        println!("Done in {}", HumanDuration(started.elapsed()));
    }

    #[test]
    fn test_finebars() {
        let styles = [
            ("Rough bar:", "█  ", "red"),
            ("Fine bar: ", "█▉▊▋▌▍▎▏  ", "yellow"),
            ("Vertical: ", "█▇▆▅▄▃▂▁  ", "green"),
            ("Fade in:  ", "█▓▒░  ", "blue"),
            ("Blocky:   ", "█▛▌▖  ", "magenta"),
        ];

        let m = MultiProgress::new();

        let handles: Vec<_> = styles
            .iter()
            .map(|s| {
                let pb = m.add(ProgressBar::new(512));
                pb.set_style(
                    ProgressStyle::with_template(&format!(
                        "{{prefix:.bold}}▕{{bar:.{}}}▏{{msg}}",
                        s.2
                    ))
                    .unwrap()
                    .progress_chars(s.1),
                );
                pb.set_prefix(s.0);
                let wait = Duration::from_millis(thread_rng().gen_range(10..30));
                thread::spawn(move || {
                    for i in 0..512 {
                        pb.inc(1);
                        pb.set_message(format!("{:3}%", 100 * i / 512));
                        thread::sleep(wait);
                    }
                    pb.finish_with_message("100%");
                })
            })
            .collect();

        for h in handles {
            let _ = h.join();
        }
    }

    #[test]
    fn test_log() {
        let pb = ProgressBar::new(100);
        for i in 0..100 {
            thread::sleep(Duration::from_millis(25));
            pb.println(format!("[+] finished #{i}"));
            pb.inc(1);
        }
        pb.finish_with_message("done");
    }

    static CRATES: &[(&str, &str)] = &[
        ("console", "v0.14.1"),
        ("lazy_static", "v1.4.0"),
        ("libc", "v0.2.93"),
        ("regex", "v1.4.6"),
        ("regex-syntax", "v0.6.23"),
        ("terminal_size", "v0.1.16"),
        ("libc", "v0.2.93"),
        ("unicode-width", "v0.1.8"),
        ("lazy_static", "v1.4.0"),
        ("number_prefix", "v0.4.0"),
        ("regex", "v1.4.6"),
        ("rand", "v0.8.3"),
        ("getrandom", "v0.2.2"),
        ("cfg-if", "v1.0.0"),
        ("libc", "v0.2.93"),
        ("rand_chacha", "v0.3.0"),
        ("ppv-lite86", "v0.2.10"),
        ("rand_core", "v0.6.2"),
        ("getrandom", "v0.2.2"),
        ("rand_core", "v0.6.2"),
        ("tokio", "v1.5.0"),
        ("bytes", "v1.0.1"),
        ("pin-project-lite", "v0.2.6"),
        ("slab", "v0.4.3"),
        ("indicatif", "v0.15.0"),
    ];

    #[test]
    fn test_cargo() {
        // number of cpus
        const NUM_CPUS: usize = 4;
        let start = Instant::now();

        // mimic cargo progress bar although it behaves a bit different
        let pb = ProgressBar::new(CRATES.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                // note that bar size is fixed unlike cargo which is dynamic
                // and also the truncation in cargo uses trailers (`...`)
                if Term::stdout().size().1 > 80 {
                    "{prefix:>12.cyan.bold} [{bar:57}] {pos}/{len} {wide_msg}"
                } else {
                    "{prefix:>12.cyan.bold} [{bar:57}] {pos}/{len}"
                },
            )
            .unwrap()
            .progress_chars("=> "),
        );
        pb.set_prefix("Building");

        // process in another thread
        // crates to be iterated but not exactly a tree
        let crates = Arc::new(Mutex::new(CRATES.iter()));
        let (tx, rx) = mpsc::channel();
        for n in 0..NUM_CPUS {
            let tx = tx.clone();
            let crates = crates.clone();
            thread::spawn(move || {
                let mut rng = rand::thread_rng();
                loop {
                    let krate = crates.lock().unwrap().next();
                    // notify main thread if n thread is processing a crate
                    tx.send((n, krate)).unwrap();
                    if let Some(krate) = krate {
                        thread::sleep(Duration::from_millis(
                            // last compile and linking is always slow, let's mimic that
                            if CRATES.last() == Some(krate) {
                                rng.gen_range(1_000..2_000)
                            } else {
                                rng.gen_range(250..1_000)
                            },
                        ));
                    } else {
                        break;
                    }
                }
            });
        }
        // drop tx to stop waiting
        drop(tx);

        let green_bold = Style::new().green().bold();

        // do progress drawing in main thread
        let mut processing = vec![None; NUM_CPUS];
        while let Ok((n, krate)) = rx.recv() {
            processing[n] = krate;
            let crates: Vec<&str> = processing
                .iter()
                .filter_map(|t| t.copied().map(|(name, _)| name))
                .collect();
            pb.set_message(crates.join(", "));
            if let Some((name, version)) = krate {
                // crate is being built
                let line = format!(
                    "{:>12} {} {}",
                    green_bold.apply_to("Compiling"),
                    name,
                    version
                );
                pb.println(line);

                pb.inc(1);
            }
        }
        pb.finish_and_clear();

        // compilation is finished
        println!(
            "{:>12} dev [unoptimized + debuginfo] target(s) in {}",
            green_bold.apply_to("Finished"),
            HumanDuration(start.elapsed())
        );
    }

    #[test]
    fn test_download_continued() {
        let mut downloaded = 69369369;
        let total_size = 231231231;

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        pb.set_position(downloaded);
        pb.reset_eta();

        while downloaded < total_size {
            downloaded = min(downloaded + 123211, total_size);
            pb.set_position(downloaded);
            thread::sleep(Duration::from_millis(12));
        }

        pb.finish_with_message("downloaded");
    }

    #[test]
    fn test_download_speed() {
        let mut downloaded = 0;
        let total_size = 231231231;

        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("#>-"));

        while downloaded < total_size {
            let new = min(downloaded + 223211, total_size);
            downloaded = new;
            pb.set_position(new);
            thread::sleep(Duration::from_millis(12));
        }

        pb.finish_with_message("downloaded");
    }

    #[test]
    fn test_fastbar() {
        let n: u64 = 1 << 20;
        let label = "Default progress bar ";
        let pb = ProgressBar::new(n);

        let mut sum = 0;
        for i in 0..n {
            // Any quick computation, followed by an update to the progress bar.
            sum += 2 * i + 3;
            pb.inc(1);
        }
        pb.finish();

        println!("[{}] Sum ({}) calculated in {:?}", label, sum, pb.elapsed());
    }

    #[test]
    fn test_iterator() {
        // Default styling, attempt to use Iterator::size_hint to count input size
        for _ in (0..1000).progress() {
            // ...
            thread::sleep(Duration::from_millis(5));
        }

        // Provide explicit number of elements in iterator
        for _ in (0..1000).progress_count(1000) {
            // ...
            thread::sleep(Duration::from_millis(5));
        }

        // Provide a custom bar style
        let pb = ProgressBar::new(1000);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] ({pos}/{len}, ETA {eta})",
            )
                .unwrap(),
        );
        for _ in (0..1000).progress_with(pb) {
            // ...
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn test_long_spinner() {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.blue} {msg}")
                .unwrap()
                // For more spinners check out the cli-spinners project:
                // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
                .tick_strings(&[
                    "▹▹▹▹▹",
                    "▸▹▹▹▹",
                    "▹▸▹▹▹",
                    "▹▹▸▹▹",
                    "▹▹▹▸▹",
                    "▹▹▹▹▸",
                    "▪▪▪▪▪",
                ]),
        );
        pb.set_message("Calculating...");
        thread::sleep(Duration::from_secs(5));
        pb.finish_with_message("Done");
    }

    #[test]
    fn test_morebars() {
        let m = Arc::new(MultiProgress::new());
        let sty = ProgressStyle::with_template("{bar:40.green/yellow} {pos:>7}/{len:7}").unwrap();

        let pb = m.add(ProgressBar::new(5));
        pb.set_style(sty.clone());

        // make sure we show up at all.  otherwise no rendering
        // event.
        pb.tick();
        for _ in 0..5 {
            let pb2 = m.add(ProgressBar::new(128));
            pb2.set_style(sty.clone());
            for _ in 0..128 {
                pb2.inc(1);
                thread::sleep(Duration::from_millis(5));
            }
            pb2.finish();
            pb.inc(1);
        }
        pb.finish_with_message("done");
    }

    #[test]
    fn test_multi() {
        let m = MultiProgress::new();
        let sty = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-");

        let pb = m.add(ProgressBar::new(128));
        pb.set_style(sty.clone());

        let pb2 = m.insert_after(&pb, ProgressBar::new(128));
        pb2.set_style(sty.clone());

        let pb3 = m.insert_after(&pb2, ProgressBar::new(1024));
        pb3.set_style(sty);

        m.println("starting!").unwrap();

        let m_clone = m.clone();
        let h1 = thread::spawn(move || {
            for i in 0..128 {
                pb.set_message(format!("item #{}", i + 1));
                pb.inc(1);
                thread::sleep(Duration::from_millis(15));
            }
            m_clone.println("pb1 is done!").unwrap();
            pb.finish_with_message("done");
        });

        let m_clone = m.clone();
        let h2 = thread::spawn(move || {
            for _ in 0..3 {
                pb2.set_position(0);
                for i in 0..128 {
                    pb2.set_message(format!("item #{}", i + 1));
                    pb2.inc(1);
                    thread::sleep(Duration::from_millis(8));
                }
            }
            m_clone.println("pb2 is done!").unwrap();
            pb2.finish_with_message("done");
        });

        let m_clone = m.clone();
        let h3 = thread::spawn(move || {
            for i in 0..1024 {
                pb3.set_message(format!("item #{}", i + 1));
                pb3.inc(1);
                thread::sleep(Duration::from_millis(2));
            }
            m_clone.println("pb3 is done!").unwrap();
            pb3.finish_with_message("done");
        });

        let _ = h1.join();
        let _ = h2.join();
        let _ = h3.join();
        m.clear().unwrap();
    }

    mod multi_tree {
        use super::*;
        use clap::__macro_refs::once_cell::sync::Lazy;
        use rand::rngs::ThreadRng;
        use rand::RngCore;

        #[derive(Debug, Clone)]
        enum Action {
            AddProgressBar(usize),
            IncProgressBar(usize),
        }

        #[derive(Clone, Debug)]
        struct Elem {
            key: String,
            index: usize,
            indent: usize,
            progress_bar: ProgressBar,
        }

        static ELEMENTS: Lazy<[Elem; 9]> = Lazy::new(|| {
            [
                Elem {
                    indent: 1,
                    index: 0,
                    progress_bar: ProgressBar::new(32),
                    key: "jumps".to_string(),
                },
                Elem {
                    indent: 2,
                    index: 1,
                    progress_bar: ProgressBar::new(32),
                    key: "lazy".to_string(),
                },
                Elem {
                    indent: 0,
                    index: 0,
                    progress_bar: ProgressBar::new(32),
                    key: "the".to_string(),
                },
                Elem {
                    indent: 3,
                    index: 3,
                    progress_bar: ProgressBar::new(32),
                    key: "dog".to_string(),
                },
                Elem {
                    indent: 2,
                    index: 2,
                    progress_bar: ProgressBar::new(32),
                    key: "over".to_string(),
                },
                Elem {
                    indent: 2,
                    index: 1,
                    progress_bar: ProgressBar::new(32),
                    key: "brown".to_string(),
                },
                Elem {
                    indent: 1,
                    index: 1,
                    progress_bar: ProgressBar::new(32),
                    key: "quick".to_string(),
                },
                Elem {
                    indent: 3,
                    index: 5,
                    progress_bar: ProgressBar::new(32),
                    key: "a".to_string(),
                },
                Elem {
                    indent: 3,
                    index: 3,
                    progress_bar: ProgressBar::new(32),
                    key: "fox".to_string(),
                },
            ]
        });

        fn get_action(rng: &mut dyn RngCore, tree: &Mutex<Vec<&Elem>>) -> Option<Action> {
            let elem_len = ELEMENTS.len() as u64;
            let list_len = tree.lock().unwrap().len() as u64;
            let sum_free = tree
                .lock()
                .unwrap()
                .iter()
                .map(|e| {
                    let pos = e.progress_bar.position();
                    let len = e.progress_bar.length().unwrap();
                    len - pos
                })
                .sum::<u64>();

            if sum_free == 0 && list_len == elem_len {
                // nothing to do more
                None
            } else if sum_free == 0 && list_len < elem_len {
                // there is no place to make an increment
                Some(Action::AddProgressBar(tree.lock().unwrap().len()))
            } else {
                loop {
                    let list = tree.lock().unwrap();
                    let k = rng.gen_range(0..17);
                    if k == 0 && list_len < elem_len {
                        return Some(Action::AddProgressBar(list.len()));
                    } else {
                        let l = (k % list_len) as usize;
                        let pos = list[l].progress_bar.position();
                        let len = list[l].progress_bar.length();
                        if pos < len.unwrap() {
                            return Some(Action::IncProgressBar(l));
                        }
                    }
                }
            }
        }

        #[test]
        fn test_multi_tree() {
            let mp = Arc::new(MultiProgress::new());
            let sty_main =
                ProgressStyle::with_template("{bar:40.green/yellow} {pos:>4}/{len:4}").unwrap();
            let sty_aux =
                ProgressStyle::with_template("{spinner:.green} {msg} {pos:>4}/{len:4}").unwrap();

            let pb_main = mp.add(ProgressBar::new(
                ELEMENTS
                    .iter()
                    .map(|e| e.progress_bar.length().unwrap())
                    .sum(),
            ));
            pb_main.set_style(sty_main);
            for elem in ELEMENTS.iter() {
                elem.progress_bar.set_style(sty_aux.clone());
            }

            let tree: Arc<Mutex<Vec<&Elem>>> =
                Arc::new(Mutex::new(Vec::with_capacity(ELEMENTS.len())));
            let tree2 = Arc::clone(&tree);

            let mp2 = Arc::clone(&mp);
            let _ = thread::spawn(move || {
                let mut rng = ThreadRng::default();
                pb_main.tick();
                loop {
                    match get_action(&mut rng, &tree) {
                        None => {
                            // all elements were exhausted
                            pb_main.finish();
                            return;
                        }
                        Some(Action::AddProgressBar(el_idx)) => {
                            let elem = &ELEMENTS[el_idx];
                            let pb = mp2.insert(elem.index + 1, elem.progress_bar.clone());
                            pb.set_message(format!("{}  {}", "  ".repeat(elem.indent), elem.key));
                            tree.lock().unwrap().insert(elem.index, elem);
                        }
                        Some(Action::IncProgressBar(el_idx)) => {
                            let elem = &tree.lock().unwrap()[el_idx];
                            elem.progress_bar.inc(1);
                            let pos = elem.progress_bar.position();
                            if pos >= elem.progress_bar.length().unwrap() {
                                elem.progress_bar.finish_with_message(format!(
                                    "{}{} {}",
                                    "  ".repeat(elem.indent),
                                    "✔",
                                    elem.key
                                ));
                            }
                            pb_main.inc(1);
                        }
                    }
                    thread::sleep(Duration::from_millis(15));
                }
            })
            .join();

            println!("===============================");
            println!("the tree should be the same as:");
            for elem in tree2.lock().unwrap().iter() {
                println!("{}  {}", "  ".repeat(elem.indent), elem.key);
            }
        }
    }

    mod multi_tree_ext {
        use super::*;
        use clap::__macro_refs::once_cell::sync::Lazy;
        use indicatif::MultiProgressAlignment;
        use rand::rngs::ThreadRng;
        use rand::RngCore;
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Debug, Clone)]
        enum Action {
            ModifyTree(usize),
            IncProgressBar(usize),
            Stop,
        }

        #[derive(Clone, Debug)]
        enum Elem {
            AddItem(Item),
            RemoveItem(Index),
        }

        #[derive(Clone, Debug)]
        struct Item {
            key: String,
            index: usize,
            indent: usize,
            progress_bar: ProgressBar,
        }

        #[derive(Clone, Debug)]
        struct Index(usize);

        const PB_LEN: u64 = 32;
        static ELEM_IDX: AtomicUsize = AtomicUsize::new(0);

        static ELEMENTS: Lazy<[Elem; 27]> = Lazy::new(|| {
            [
                Elem::AddItem(Item {
                    indent: 9,
                    index: 0,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "dog".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 0,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_1".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 8,
                    index: 1,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "lazy".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 1,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_2".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 1,
                    index: 0,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "the".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 0,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_3".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 7,
                    index: 3,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "a".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 3,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_4".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 6,
                    index: 2,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "over".to_string(),
                }),
                Elem::RemoveItem(Index(6)),
                Elem::RemoveItem(Index(4)),
                Elem::RemoveItem(Index(3)),
                Elem::RemoveItem(Index(0)),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 2,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_5".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 4,
                    index: 1,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "fox".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 1,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_6".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 2,
                    index: 1,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "quick".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 1,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_7".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 5,
                    index: 5,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "jumps".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 5,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_8".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 3,
                    index: 4,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "brown".to_string(),
                }),
                Elem::AddItem(Item {
                    indent: 0,
                    index: 3,
                    progress_bar: ProgressBar::new(PB_LEN),
                    key: "temp_9".to_string(),
                }),
                Elem::RemoveItem(Index(10)),
                Elem::RemoveItem(Index(7)),
                Elem::RemoveItem(Index(4)),
                Elem::RemoveItem(Index(3)),
                Elem::RemoveItem(Index(1)),
            ]
        });

        /// The example demonstrates the usage of `MultiProgress` and further extends `multi-tree` example.
        /// Now the example has 3 different actions implemented, and the item tree can be modified
        /// by inserting or removing progress bars. The progress bars to be removed eventually
        /// have messages with pattern `"temp_*"`.
        ///
        /// Also the command option `--bottom-alignment` is used to control the vertical alignment of the
        /// `MultiProgress`. To enable this run it with
        /// ```ignore
        /// cargo run --example multi-tree-ext -- --bottom-alignment
        /// ```
        #[test]
        fn test_multi_tree_ext() {
            let mp = Arc::new(MultiProgress::new());
            // let alignment = MultiProgressAlignment::Bottom;
            let alignment = MultiProgressAlignment::Top;
            mp.set_alignment(alignment);
            let sty_main =
                ProgressStyle::with_template("{bar:40.green/yellow} {pos:>4}/{len:4}").unwrap();
            let sty_aux =
                ProgressStyle::with_template("[{pos:>2}/{len:2}] {prefix}{spinner:.green} {msg}")
                    .unwrap();
            let sty_fin = ProgressStyle::with_template("[{pos:>2}/{len:2}] {prefix}{msg}").unwrap();

            let pb_main = mp.add(ProgressBar::new(
                ELEMENTS
                    .iter()
                    .map(|e| match e {
                        Elem::AddItem(item) => item.progress_bar.length().unwrap(),
                        Elem::RemoveItem(_) => 1,
                    })
                    .sum(),
            ));

            pb_main.set_style(sty_main);
            for e in ELEMENTS.iter() {
                match e {
                    Elem::AddItem(item) => item.progress_bar.set_style(sty_aux.clone()),
                    Elem::RemoveItem(_) => {}
                }
            }

            let mut items: Vec<&Item> = Vec::with_capacity(ELEMENTS.len());

            let mp2 = Arc::clone(&mp);
            let mut rng = ThreadRng::default();
            pb_main.tick();
            loop {
                match get_action(&mut rng, &items) {
                    Action::Stop => {
                        // all elements were exhausted
                        pb_main.finish();
                        return;
                    }
                    Action::ModifyTree(elem_idx) => match &ELEMENTS[elem_idx] {
                        Elem::AddItem(item) => {
                            let pb = mp2.insert(item.index, item.progress_bar.clone());
                            pb.set_prefix("  ".repeat(item.indent));
                            pb.set_message(&item.key);
                            items.insert(item.index, item);
                        }
                        Elem::RemoveItem(Index(index)) => {
                            let item = items.remove(*index);
                            let pb = &item.progress_bar;
                            mp2.remove(pb);
                            pb_main.inc(pb.length().unwrap() - pb.position());
                        }
                    },
                    Action::IncProgressBar(item_idx) => {
                        let item = &items[item_idx];
                        item.progress_bar.inc(1);
                        let pos = item.progress_bar.position();
                        if pos >= item.progress_bar.length().unwrap() {
                            item.progress_bar.set_style(sty_fin.clone());
                            item.progress_bar.finish_with_message(format!(
                                "{} {}",
                                style("✔").green(),
                                item.key
                            ));
                        }
                        pb_main.inc(1);
                    }
                }
                thread::sleep(Duration::from_millis(20));
            }
        }

        /// The function guarantees to return the action, that is valid for the current tree.
        fn get_action(rng: &mut dyn RngCore, items: &[&Item]) -> Action {
            let elem_idx = ELEM_IDX.load(Ordering::SeqCst);
            // the indices of those items, that not completed yet
            let uncompleted = items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    let pos = item.progress_bar.position();
                    pos < item.progress_bar.length().unwrap()
                })
                .map(|(idx, _)| idx)
                .collect::<Vec<usize>>();
            let k = rng.gen_range(0..16);
            if (k > 0 || k == 0 && elem_idx == ELEMENTS.len()) && !uncompleted.is_empty() {
                let idx = rng.gen_range(0..uncompleted.len() as u64) as usize;
                Action::IncProgressBar(uncompleted[idx])
            } else if elem_idx < ELEMENTS.len() {
                ELEM_IDX.fetch_add(1, Ordering::SeqCst);
                Action::ModifyTree(elem_idx)
            } else {
                // nothing to do more
                Action::Stop
            }
        }
    }
}
