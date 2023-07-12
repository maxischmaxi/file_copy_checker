use std::path::Path;
use std::fs::read_dir;
use std::fs::remove_file;
use std::os::unix::fs::symlink;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;
use md5hash::MD5Hasher;
use std::time::{Duration, Instant};
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use dialoguer::{FuzzySelect, Input, theme::ColorfulTheme, MultiSelect};

#[derive(Debug)]
struct ReadFile {
  hash: String,
  first_path: PathBuf,
  paths: Vec<PathBuf>,
  size: u64,
}

#[derive(Debug)]
struct FileCountResult {
  folders: u64,
  files: u64,
}

/**
 * check if the current file should be ignored
 */
fn should_ignore_file(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    
    if !path.is_file() {
        return true;
    }
    
    let name = std::env::current_exe().unwrap();
    
    if path == name {
        return true;
    }
    
    let name = path.file_name().unwrap();
    
    if name == ".DS_Store" {
        return true;
    }
    
    if name == ".localized" {
        return true;
    }
    
    if name == "Thumbs.db" {
        return true;
    }
    
    if name == ".gitignore" {
        return true;
    }
    
    if name == ".svn" {
        return true;
    }
    
    if name == ".idea" {
        return true;
    }
    return false;
}

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
        
        let file: File = File::open(&path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).unwrap();

        let mut hasher = MD5Hasher::new();
        hasher.digest(&buffer);
        let digest = hasher.finish();
        let hash: String = format!("{:x}", digest).to_string();

        let mut found = false;
        for (i, f) in files.iter().enumerate() {
            if f.hash == hash {
                files[i].paths.push(path.to_path_buf());
                pb.set_message(format!("Found duplicate {}", path.display()));
                found = true;
                break;
            }
        }

        if found {
            continue;
        }

        files.push(ReadFile { 
            hash,
            first_path: path.to_path_buf(),
            paths: vec![],
            size: 0
        });
        pb.set_message(format!("Found new file {}", path.display()));
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
fn remove_files(files: &Vec<ReadFile>, spinner_style: &ProgressStyle) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[3/4]");
    pb.set_message(format!("Removing duplicates"));

    for file in files {
        for (_, p) in file.paths.iter().enumerate() {
            let path = Path::new(&p);
            if should_ignore_file(&path) {
                continue;
            }
            remove_file(&path).unwrap();
            pb.set_message(format!("Removed {}", p.display()));
        }
    }

    pb.finish();
}

/**
 * Delete all files from the duplicates array and create symbolic links to the original files
 */
fn remove_files_and_create_symbolic_links(files: &Vec<ReadFile>, spinner_style: &ProgressStyle) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[3/4]");
    pb.set_message(format!("Removing duplicates and creating symbolic links"));

    for file in files {
        for (_, p) in file.paths.iter().enumerate() {
            let path = Path::new(&p);
            if should_ignore_file(&path) {
                continue;
            }
            remove_file(&path).unwrap();
            symlink(file.first_path.as_path(), &path).unwrap();
            pb.set_message(format!("Removed {} and created symbolic link to {}", file.first_path.display(), path.display()));
        }      

    }

    pb.finish();
}

/**
 * Removes the file and creates a symbolic link to the original file
 */
fn remove_file_and_symlink(original_file: &Path, file: &Path, pb: &ProgressBar) {
    pb.set_message(format!("Removing {} and creating symbolic link to {}", file.display(), original_file.display()));
    remove_file(&file).unwrap();
    symlink(&original_file, &file).unwrap();
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

/**
 * Generates the options for the user to select from
 */
fn generate_options(files: &Vec<ReadFile>) -> Vec<String> {
    let mut options: Vec<String> = Vec::new();
    for (i, file) in files.iter().enumerate() {
        if i >= 10 {
            break;
        }
        let option = format!("{}, {} mal, HASH: [{}]", file.first_path.display(), file.paths.len(), file.hash);
        options.push(option);
    }
    return options;
}

fn main() {
    // get current working directory
    let cwd = std::env::current_dir().unwrap();
    let base_path: &Path = cwd.as_path();

    let spinner_style: ProgressStyle = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}").unwrap().tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈");

    // calculate file count
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[1/5]");
    pb.set_message(format!("Looking for files..."));
    let start = Instant::now();
    let result = calculate_file_count(&base_path, &pb);    
    let elapsed = start.elapsed();
    pb.set_message(format!("Found {} files and {} folders in {}", result.files, result.folders, HumanDuration(elapsed)));
    pb.finish();

    // collect files
    let mut files: Vec<ReadFile> = Vec::new();
    let pb: ProgressBar = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_prefix("[2/5]");
    let start: Instant = Instant::now();
    collect_files(&base_path, &mut files,  &pb);
    files.retain(|f| f.paths.len() > 0);
    for file in files.iter_mut() {
        file.size = filesize::file_real_size(file.first_path.as_path()).unwrap();
    };
    files.sort_by(|a, b| b.size.cmp(&a.size));
    let mut duplicate_count = 0;
    for file in files.iter() {
        duplicate_count += file.paths.len();
    }
    let elapsed: Duration = start.elapsed();
    let mut message = format!("Collected {} files ({} duplicates) in {}", files.len(), duplicate_count, HumanDuration(elapsed));
    if files.len() == 0 {
        pb.set_message(format!("No duplicates found"));
        pb.finish();
        abort();
    }
    if files.len() == 1 {
        message = format!("Collected {} file ({} duplicates) in {}", files.len(), duplicate_count, HumanDuration(elapsed));
    }
    pb.set_message(message);
    pb.finish();


    let items = vec![
        "Remove duplicates",
        "Remove duplicates and create symbolic links",
        "Select duplicates to remove",
        "Select duplicates to remove and create symbolic links",
        "Generate Report and exit",
        "Abort"
    ];

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("What do you want to do?")
        .default(1)
        .items(&items[..])
        .interact()
        .unwrap();

    // abort
    if selection == 5 {
        abort();
    }

    // generate report
    if selection == 4 {
        let font_family = genpdf::fonts::from_files(&"fonts", "Roboto", None).unwrap();
        let mut doc = genpdf::Document::new(font_family);
        doc.set_title("Duplicate File Finder Report");
        doc.set_font_size(13);
        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(10);
        doc.set_page_decorator(decorator);
        let title = genpdf::elements::Paragraph::new("Duplicate File Finder Report");
        doc.push(title);
        let mut table = genpdf::elements::TableLayout::new(vec![1, 1]);
        table.set_cell_decorator(genpdf::elements::FrameCellDecorator::new(true, true, false));
        for file in files.iter() {
            let size = humansize::format_size(file.size, humansize::DECIMAL);
            let mut row = table.row();
            let filename: String = file.first_path.file_name().unwrap().to_str().unwrap().to_string();
            row.push_element(genpdf::elements::Paragraph::new(filename));
            row.push_element(genpdf::elements::Paragraph::new(size));
            row.push().unwrap();
        }
        doc.push(table);

        let homedir = home::home_dir().unwrap();
        let input: String = Input::new()
            .with_prompt("Where do you want to save the report?")
            .with_initial_text(homedir.display().to_string())
            .default(homedir.display().to_string())
            .interact_text()
            .unwrap();

        let path = Path::new(&input);

        if !path.exists() {
            println!("Path does not exist");
            abort();
        }

        if !path.is_dir() {
            println!("Path is not a directory");
            abort();
        }
        
        let filename = format!("file_copy_checker_report_{}.pdf", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
        doc.render_to_file(path.join(filename)).unwrap();
        done(&spinner_style);
    }

    // remove selected duplicates and create symlinks
    if selection == 3 {
        let options = generate_options(&files);
        let chosen: Vec<usize> = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Which files do you want to delete and create symbolic links? (Only the first 10 are shown)")
            .items(&options)
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
            let items: Vec<&str> = options[i].split("HASH: [").collect();
            let hash = items[1].replace("]", "");
            let mut paths: Vec<&Path> = Vec::new();
            let mut first_path: &Path = Path::new("");
            let mut found = false;
            for file in &files {
                if file.hash == hash {
                    found = true;
                    for path in &file.paths {
                        paths.push(Path::new(path));
                    }
                    first_path = Path::new(&file.first_path);
                }
            }

            if !found {
                continue;
            }
            
            for path in paths {
                remove_file_and_symlink(first_path, &path, &pb)
            }
        }
        pb.finish();
        done(&spinner_style);
    }

    // remove selected duplicates
    if selection == 2 {
        let options = generate_options(&files);
        let chosen: Vec<usize> = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Which files do you want to delete? (Only the first 10 are shown)")
            .items(&options)
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
            let items: Vec<&str> = options[i].split("[").collect();
            let hash = items[1].replace("]", "");
            let mut paths: Vec<&Path> = Vec::new();
            let mut original_file: &Path = Path::new("");
            for file in &files {
                if file.hash == hash {
                    for path in &file.paths {
                        paths.push(Path::new(path));
                    }
                    original_file = Path::new(&file.first_path);
                }
            }

            for path in paths {
                pb.set_message(format!("Deleting {}", path.display()));
                remove_file(path).unwrap();
            }
            pb.set_message(format!("Deleting {}", original_file.display()));
            remove_file(original_file).unwrap();
        }
        pb.finish();
        done(&spinner_style);
    }

    // remove all duplicates and create symlinks
    if selection == 1 {
        remove_files_and_create_symbolic_links(&files, &spinner_style);
        done(&spinner_style);
    }

    // remove all duplicates
    if selection == 0 {
        remove_files(&files, &spinner_style);
        done(&spinner_style);
    }
}
