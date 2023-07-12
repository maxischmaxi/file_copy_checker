use std::path::Path;
use std::fs::read_dir;
use std::fs::remove_file;
use std::os::unix::fs::symlink;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use md5hash::MD5Hasher;
use std::time::{Duration, Instant};
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use dialoguer::{FuzzySelect, theme::ColorfulTheme, MultiSelect};
use ignore::{should_ignore_file, should_ignore_files};
use duplicate::Duplicate;
use filecountresult::FileCountResult;
use readfile::ReadFile;

mod duplicate;
mod ignore;
mod filecountresult;
mod readfile;

/**
 * Collects all files in the given path and returns them as a vector of ReadFile structs.
 */
fn collect_files(base_path: &Path, files: &mut Vec<ReadFile>, pb: &ProgressBar) {
    pb.set_message(format!("Collecting files in {}", base_path.display()));

    let dir = read_dir(base_path).unwrap();

    for entry in dir {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            collect_files(&path, files, pb);
            continue;
        }

        if should_ignore_file(&path) {
            continue;
        }
        
        let file = File::open(&path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).unwrap();

        let mut hasher = MD5Hasher::new();
        hasher.digest(&buffer);
        let digest = hasher.finish();
        let hash: String = format!("{:x}", digest).to_string();

        files.push(ReadFile { path, hash });
    }
}

/**
 * Checks for duplicates in the given vector of ReadFile structs and returns them as a vector of Duplicate structs.
 */
fn check_duplicates(files: &mut Vec<ReadFile>, duplicates: &mut Vec<Duplicate>, pb: &ProgressBar) {
    pb.set_message(format!("Checking duplicates"));

    let mut i = 0;
    while i < files.len() {
        let mut j = i + 1;
        while j < files.len() {
            if files[i].hash == files[j].hash {
                duplicates.push(Duplicate { path1: files[i].path.clone(), path2: files[j].path.clone() });
            }
            j += 1;
        }
        i += 1;
    }
}

/**
 * Calculates the number of files that are beeing checked
 */
fn calculate_file_count(path: &Path, pb: &ProgressBar) -> FileCountResult {
    if path.is_file() {
        return FileCountResult { files: 1, folders: 0 };
    }

    pb.set_message(format!("Calculating file count in {}", path.display()));

    let dir = read_dir(path).unwrap();
    let mut file_count: u64 = 0;
    let mut folder_count: u64 = 0;

    for entry in dir {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            folder_count += 1;
            let new_counts =  calculate_file_count(&path, pb);
            file_count += new_counts.files;
            folder_count += new_counts.folders;
            continue;
        } 
        
        if should_ignore_file(&path) {
            continue;
        }
        file_count += 1;
    }

    return FileCountResult { files: file_count, folders: folder_count };
}

/**
 * Delete all files from the duplicates array
 */
fn remove_files(duplicates: &Vec<Duplicate>, spinner_style: &ProgressStyle) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[3/4]");
    pb.set_message(format!("Removing duplicates"));

    for duplicate in duplicates {
        should_ignore_files(&duplicate.path1, &duplicate.path2);

        remove_file(&duplicate.path2).unwrap();
        pb.set_message(format!("Removed {}", duplicate.path2.display()));
    }

    pb.finish();
}

/**
 * Delete all files from the duplicates array and create symbolic links to the original files
 */
fn remove_files_and_create_symbolic_links(duplicates: &Vec<Duplicate>, spinner_style: &ProgressStyle) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[3/4]");
    pb.set_message(format!("Removing duplicates and creating symbolic links"));

    for duplicate in duplicates {
        
        if should_ignore_files(&duplicate.path1, &duplicate.path2) {
            continue;
        }

        remove_file(&duplicate.path2).unwrap();
        symlink(&duplicate.path1, &duplicate.path2).unwrap();

        pb.set_message(format!("Removed {} and created symbolic link to {}", duplicate.path2.display(), duplicate.path1.display()));
    }

    pb.finish();
}

/**
 * Aborts the Program
 */
fn abort() {
    println!("Aborted");
    std::process::exit(0);
}

/**
 * Finishes the Program
 */
fn done(spinner_style: &ProgressStyle) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[5/5]");
    pb.set_message("Done!");
    pb.finish();
    std::process::exit(0);
}

fn main() {
    // get current working directory
    let cwd = std::env::current_dir().unwrap();
    let base_path: &Path = cwd.as_path();

    let spinner_style: ProgressStyle = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}").unwrap().tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈");

    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[1/5]");
    pb.set_message(format!("Looking for files..."));
    let start = Instant::now();
    let result = calculate_file_count(&base_path, &pb);    
    let elapsed = start.elapsed();
    let elapsed: HumanDuration = HumanDuration(elapsed);
    pb.set_message(format!("Found {} files and {} folders in {}", result.files, result.folders, elapsed));
    pb.finish();

    // collect files
    let mut files: Vec<ReadFile> = Vec::new();
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[2/5]");
    let start: Instant = Instant::now();
    collect_files(&base_path, &mut files,  &pb);
    let elapsed: Duration = start.elapsed();
    let elapsed: HumanDuration = HumanDuration(elapsed);
    pb.set_message(format!("Collected {} files in {}", result.files, elapsed));
    pb.finish();

    // check duplicates
    let mut duplicates: Vec<Duplicate> = Vec::new();
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[3/5]");
    let start: Instant = Instant::now();
    check_duplicates(&mut files, &mut duplicates, &pb);
    let elapsed: Duration = start.elapsed();
    let elapsed: HumanDuration = HumanDuration(elapsed);
    pb.set_message(format!("No duplicates found in {}", elapsed));
    pb.finish();
    
    if duplicates.len() == 0 {
        let pb = ProgressBar::new_spinner();
        pb.set_style(spinner_style.clone());
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_prefix("[4/5]");
        pb.set_message("No duplicates found");
        pb.finish();
        done(&spinner_style);
    }

    pb.set_message(format!("Found {} duplicates in {} files in {}", duplicates.len(), result.files, elapsed));
    pb.finish();
    
    let items = vec![
        "Remove duplicates",
        "Remove duplicates and create symbolic links",
        "Select duplicates to remove",
        "Select duplicates to remove and create symbolic links",
        "Abort"
    ];

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("What do you want to do?")
        .default(1)
        .items(&items[..])
        .interact()
        .unwrap();

    if selection == 4 {
        abort();
    }

    if selection == 3 {
        let mut options: Vec<String> = Vec::new();
        for (i, duplicate) in duplicates.iter().enumerate() {
            if i >= 10 {
                break;
            }
            options.push(format!("{} and {}", duplicate.path1.display(), duplicate.path2.display()));
        }
        let chosen: Vec<usize> = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Which files do you want to delete and create symbolic links? (Only the first 10 are shown)")
            .items(&options[..])
            .interact()
            .unwrap();

        if chosen.len() == 0 {
            abort();
        }

        let pb = ProgressBar::new_spinner();
        pb.set_style(spinner_style.clone());
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_prefix("[4/5]");
        pb.set_message(format!("Deleting {} files", chosen.len()));
        for i in chosen {
            let paths: Vec<&str> = options[i].split(" and ").collect();
            let path1 = Path::new(paths[0]);
            let path2 = Path::new(paths[1]);

            if !ignore::should_ignore_file(path1) {
                pb.set_message(format!("Deleting {}", path1.display()));
                remove_file(path2).unwrap();
                pb.set_message(format!("Creating symbolic link for {}", path2.display()));
                symlink(path1, path2).unwrap();
            }
        }
        pb.finish();
        done(&spinner_style);
    }

    if selection == 2 {
        let mut options: Vec<String> = Vec::new();
        for (i, duplicate) in duplicates.iter().enumerate() {
            if i >= 10 {
                break;
            }
            options.push(format!("{} and {}", duplicate.path1.display(), duplicate.path2.display()));
        }
        let chosen: Vec<usize> = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Which files do you want to delete? (Only the first 10 are shown)")
            .items(&options[..])
            .interact()
            .unwrap();

        if chosen.len() == 0 {
            abort();
        }

        let pb = ProgressBar::new_spinner();
        pb.set_style(spinner_style.clone());
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_prefix("[4/5]");
        pb.set_message(format!("Deleting {} files", chosen.len()));
        for i in chosen {
            let paths: Vec<&str> = options[i].split(" and ").collect();
            let path1 = Path::new(paths[0]);
            let path2 = Path::new(paths[1]);

            if !should_ignore_file(path1) {
                pb.set_message(format!("Deleting {}", path1.display()));
                remove_file(path2).unwrap();
            }
        }
        pb.finish();
        done(&spinner_style);
    }

    if selection == 1 {
        remove_files_and_create_symbolic_links(&duplicates, &spinner_style);
        done(&spinner_style);
    }

    if selection == 0 {
        remove_files(&duplicates, &spinner_style);
        done(&spinner_style);
    }
}
