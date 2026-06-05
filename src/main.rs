use regex::Regex;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let mut collector: Vec<&str> = Vec::new();

    // Path to GitHub Desktop executable
    let github_desktop_path = Path::new(r"C:\Users\joefa\AppData\Local\GitHubDesktop\bin\github");

    // Regex pattern for matching the GitHub Desktop version string
    let github_desktop_version_regex =
        Regex::new(r"/(app-.*)/resources/app/static/github.sh").unwrap();

    let mut text = String::new();
    let mut file = File::open(&github_desktop_path).unwrap();
    file.read_to_string(&mut text).unwrap();

    // Extract the version string from the GitHub Desktop executable
    let version_results = github_desktop_version_regex.captures(&text).unwrap();
    let github_desktop_version = version_results.get(1).unwrap().as_str().to_string();
    assert!(github_desktop_version.starts_with("app-"));

    collector.push(&github_desktop_version);

    // Path to Git for Windows executable
    let git_for_windows_path = PathBuf::from(&github_desktop_path)
        .join("..")
        .join("..")
        .join(&github_desktop_version)
        .join("resources")
        .join("app")
        .join("git")
        .join("cmd")
        .join("git.exe")
        .canonicalize()
        .unwrap();
    assert!(git_for_windows_path.is_file());

    // Current working directory
    let current_working_directory = std::env::current_dir().unwrap();

    // Path to the current executable
    let current_executable_path = std::env::current_exe().unwrap();

    collector.push(git_for_windows_path.to_str().unwrap());
    collector.push(current_working_directory.to_str().unwrap());
    collector.push(current_executable_path.to_str().unwrap());

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        // Execute the Git command with the specified arguments
        let status = Command::new(&git_for_windows_path)
            .current_dir(current_working_directory.clone())
            .args(&args)
            .status()
            .unwrap_or_else(|_| {
                eprintln!(
                    "[error] failed to execute Git command\n[debug] collector: {:?}",
                    collector
                );
                std::process::exit(1);
            });
        if status.success() {
            eprintln!("[status] operation completed")
        } else {
            eprintln!(
                "[status] operation failed - {}\n[debug] collector: {:?}",
                status, collector
            );
        }
    } else {
        // Print the path to the Git executable if no arguments are specified
        println!("{}", git_for_windows_path.to_string_lossy());
    }
}
