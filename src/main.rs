use std::path::Path;
use std::fs;
use std::os::unix::fs::symlink;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use indicatif::HumanDuration;
use md5hash::MD5Hasher;
use std::time::{Duration, Instant};
use csv::Writer;
use std::thread;
use dialoguer::{FuzzySelect, Input, theme::ColorfulTheme, MultiSelect, Select};

#[derive(Debug)]
struct ReadFile {
  hash: String,
  first_path: PathBuf,
  paths: Vec<PathBuf>,
}

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

fn hash_file(path: &Path) -> String {
    let file: File = File::open(&path).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).unwrap();

    let mut hasher = MD5Hasher::new();
    hasher.digest(&buffer);
    let digest = hasher.finish();
    let hash: String = format!("{:x}", digest).to_string();
    return hash;
}

fn process_file(path: &Path, files: &mut Arc<Mutex<Vec<ReadFile>>>, duplicated_count: &mut Arc<Mutex<u64>>) {
    if should_ignore_file(&path) {
        return;
    }

    let hash = hash_file(&path);

    let mut found = false;
    for (i, f) in files.lock().unwrap().iter().enumerate() {
        if f.hash == hash {
            files.lock().unwrap()[i].paths.push(path.to_path_buf());
            *duplicated_count.lock().unwrap() += 1;
            found = true;
            break;
        }
    }

    if found {
        return;
    }

    // let size = filesize::file_real_size(path.clone()).unwrap();
    files.lock().unwrap().push(ReadFile { 
        hash,
        first_path: path.to_path_buf(),
        paths: vec![],
    });
}

fn process_directory(base_path: &Path, files: &mut Arc<Mutex<Vec<ReadFile>>>, duplicated_count: &mut Arc<Mutex<u64>>) {
    let dir = fs::read_dir(base_path).unwrap();
    let files = Arc::clone(files);
    let duplicated_count = Arc::clone(duplicated_count);
    let mut files = Arc::clone(&files);
    let mut duplicated_count = Arc::clone(&duplicated_count);
    
    let handle = thread::spawn(move || {
        for entry in dir {
            let entry = entry.unwrap();
            let path = entry.path();
                
            if path.is_dir() {
                process_directory(&path, &mut files, &mut duplicated_count);
                return;
            }
    
            process_file(&path, &mut files, &mut duplicated_count);
            return;
        }
    });
    handle.join().unwrap();
    println!("processed {}", base_path.display());
}

fn remove_files(files: &Vec<ReadFile>) {
    for file in files {
        for (_, p) in file.paths.iter().enumerate() {
            let path = Path::new(&p);
            if should_ignore_file(&path) {
                continue;
            }
            fs::remove_file(&path).unwrap();
        }
    }
}

fn remove_files_and_create_symbolic_links(files: &Vec<ReadFile>) {
    for file in files {
        for (_, p) in file.paths.iter().enumerate() {
            let path = Path::new(&p);
            if should_ignore_file(&path) {
                continue;
            }
            fs::remove_file(&path).unwrap();
            symlink(file.first_path.as_path(), &path).unwrap();
        }      

    }
}

fn remove_file_and_symlink(original_file: &Path, file: &Path) {
    fs::remove_file(&file).unwrap();
    symlink(&original_file, &file).unwrap();
}

fn abort() {
    println!("Aborted");
    std::process::exit(0);
}

fn done() {
    println!("Done");
    std::process::exit(0);
}

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

fn generate_pdf(files: &Vec<ReadFile>, show_file_sizes: bool) {
    let start = Instant::now();
    let font_family = genpdf::fonts::from_files(&"fonts", "Roboto", None).unwrap();
    let mut doc = genpdf::Document::new(font_family);
    let mut decorator = genpdf::SimplePageDecorator::new();
    let title = genpdf::elements::Paragraph::new("Duplicate File Finder Report");
    let mut column_vec = vec![1, 1, 1];
    if show_file_sizes {
        column_vec.push(1);
    }
    let mut table = genpdf::elements::TableLayout::new(column_vec);
    
    doc.set_title("Duplicate File Finder Report");
    doc.set_font_size(13);
    decorator.set_margins(10);
    doc.set_page_decorator(decorator);
    doc.push(title);
    table.set_cell_decorator(genpdf::elements::FrameCellDecorator::new(true, true, false));

    for file in files.iter() {
        let mut row = table.row();
        let path = genpdf::elements::Paragraph::new(file.first_path.file_name().unwrap().to_str().unwrap().to_string());
        let count = genpdf::elements::Paragraph::new(format!("{} weitere", file.paths.len().to_string()));
        let hash = genpdf::elements::Paragraph::new(file.hash.clone());
        row.push_element(path);
        row.push_element(count);
        row.push_element(hash);
        
        if show_file_sizes {
            let file_size = filesize::file_real_size(file.first_path.clone()).unwrap();
            let file_size = humansize::format_size(file_size, humansize::DECIMAL);
            let file_size = genpdf::elements::Paragraph::new(file_size);
            row.push_element(file_size);
        }
        
        row.push().unwrap();
    }

    doc.push(table);
    let elapsed = start.elapsed();
    println!("Generating PDF took {} seconds", HumanDuration(elapsed));
    

    let homedir = home::home_dir().unwrap().display().to_string();
    let input: String = Input::new()
        .with_prompt("Where do you want to save the report?")
        .with_initial_text(&homedir)
        .default(homedir)
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
}

fn generate_csv(files: &Vec<ReadFile>, show_file_sizes: bool) {
    let homedir = home::home_dir().unwrap().display().to_string();
    let input: String = Input::new()
        .with_prompt("Where do you want to save the report?")
        .with_initial_text(&homedir)
        .default(homedir)
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

    let filename = format!("file_copy_checker_report_{}.csv", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
    let start = Instant::now();
    let mut wrt = Writer::from_path(path.join(filename)).unwrap();
    let mut headers = vec!["Filename", "Duplicates", "Hash"];
    if show_file_sizes {
        headers.push("Size");
    }
    wrt.write_record(&headers).unwrap();
    for file in files {
        let mut row = Vec::new();
        row.push(file.first_path.display().to_string());
        row.push(file.paths.len().to_string());
        row.push(file.hash.to_string());
        if show_file_sizes {
            println!("Getting file size for {}", file.first_path.display().to_string());
            let file_size = filesize::file_real_size(file.first_path.clone()).unwrap();
            let file_size = humansize::format_size(file_size, humansize::DECIMAL);
            row.push(file_size);
        }
        wrt.write_record(&row).unwrap();
    }
    let elapsed = start.elapsed();
    println!("CSV generation took {} seconds", elapsed.as_secs());
}

fn main() {
    // get current working directory
    let cwd = std::env::current_dir().unwrap();

    let files: Arc<Mutex<Vec<ReadFile>>> = Arc::new(Mutex::new(Vec::new()));
    let duplicate_count: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let mut files_guard = Arc::clone(&files);
    let mut duplicates_guard = Arc::clone(&duplicate_count);

    let start: Instant = Instant::now();
    let handle = thread::spawn(move || {
        let base_path: &Path = cwd.as_path();
        process_directory(&base_path, &mut files_guard, &mut duplicates_guard);
    });
    handle.join().unwrap();
    let elapsed: Duration = start.elapsed();
    let files = files.lock().unwrap();
    let duplicate_count = duplicate_count.lock().unwrap();
    println!("Found {} duplicates in {} files in {} seconds", duplicate_count, files.len(), elapsed.as_secs());
    
    if files.len() == 0 {
        abort();
    }


    let items = vec![
        "Remove duplicates",
        "Remove duplicates and create symbolic links",
        "Select duplicates to remove",
        "Select duplicates to remove and create symbolic links",
        "Generate PDF Report and exit",
        "Generate PDF Report with file sizes and exit",
        "Generate CSV Report and exit",
        "Generate CSV Report with file sizes and exit",
        "Show Duplicates of a specific file",
        "Show Duplicates of a specific file with file size",
        "Abort"
    ];

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("What do you want to do?")
        .default(1)
        .items(&items[..])
        .interact()
        .unwrap();

    // abort
    if selection == 10 {
        abort();
    }

    // show duplicates of a specific file with file size
    if selection == 9 {
        let options = generate_options(&files);
        
        let index = Select::with_theme(&ColorfulTheme::default())
            .items(&options)
            .interact()
            .unwrap();

        let option = &options[index];
        let items: Vec<&str> = option.split("HASH: [").collect();
        let hash = items[1].replace("]", "");

        let file = files.iter().find(|&f| f.hash == hash).unwrap();
        let size = filesize::file_real_size(file.first_path.clone()).unwrap();
        let size = humansize::format_size(size, humansize::DECIMAL);
        println!("Listing files...");
        println!("{} {}", file.first_path.display().to_string(), size);
        for path in file.paths.iter() {
            let size = filesize::file_real_size(path.clone()).unwrap();
            let size = humansize::format_size(size, humansize::DECIMAL);
            println!("{} {}", path.display().to_string(), size);
        }
        done();
    }

    // show duplicates of a specific file
    if selection == 8 {
        let options = generate_options(&files);
        
        let index = Select::with_theme(&ColorfulTheme::default())
            .items(&options)
            .interact()
            .unwrap();

        let option = &options[index];
        let items: Vec<&str> = option.split("HASH: [").collect();
        let hash = items[1].replace("]", "");

        let file = files.iter().find(|&f| f.hash == hash).unwrap();
        println!("Listing files...");
        println!("{}", file.first_path.display().to_string());
        for path in file.paths.iter() {
            println!("{}", path.display().to_string());
        }
        done();
    }

    // generate csv report with file sizes
    if selection == 7 {
        generate_csv(&files, true);
        done();
    }

    // generate csv report
    if selection == 6 {
        generate_csv(&files, false);
        done();
    }

    // generate report with file sizes
    if selection == 5 {
        generate_pdf(&files, true);
        done();
    }

    // generate report
    if selection == 4 {
        generate_pdf(&files, false);
        done();
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

        for i in chosen {
            let items: Vec<&str> = options[i].split("HASH: [").collect();
            let hash = items[1].replace("]", "");
            let mut paths: Vec<&Path> = Vec::new();
            let mut first_path: &Path = Path::new("");
            let mut found = false;
            for file in files.iter() {
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
                remove_file_and_symlink(first_path, &path)
            }
        }
        done();
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

        for i in chosen {
            let items: Vec<&str> = options[i].split("[").collect();
            let hash = items[1].replace("]", "");
            let mut paths: Vec<&Path> = Vec::new();
            let mut original_file: &Path = Path::new("");
            for file in files.iter() {
                if file.hash == hash {
                    for path in &file.paths {
                        paths.push(Path::new(path));
                    }
                    original_file = Path::new(&file.first_path);
                }
            }

            for path in paths {
                fs::remove_file(path).unwrap();
            }
            fs::remove_file(original_file).unwrap();
        }
        done();
    }

    // remove all duplicates and create symlinks
    if selection == 1 {
        remove_files_and_create_symbolic_links(&files);
        done();
    }

    // remove all duplicates
    if selection == 0 {
        remove_files(&files);
        done();
    }
}
