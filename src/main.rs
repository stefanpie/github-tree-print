use clap::Parser;
use dotenv::dotenv;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// GitHub repository in the format "username/repo"
    #[arg(help = "GitHub repository in the format 'username/repo'")]
    repo_name: String,

    /// GitHub token for authentication (optional if using GITHUB_TOKEN env variable)
    #[arg(
        long,
        help = "GitHub token for authentication (if you would like to explicitly provide it)"
    )]
    token: Option<String>,

    /// Output file path to write the directory structure (optional)
    #[arg(long, help = "Output file path to write the directory structure")]
    output_file: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok(); // Load environment variables from .env if available

    let args = Args::parse();

    // Get the GitHub token from args or environment variable
    let github_token = args.token
        .or_else(|| env::var("GITHUB_TOKEN").ok())
        .expect("GitHub token not provided. Set it as an argument, in the GITHUB_TOKEN environment variable, or in a .env file as GITHUB_TOKEN");

    let (owner, repo) = parse_repo_name(&args.repo_name)?;

    // Initialize the reqwest client
    let client = Client::new();

    // Step 1: Get the default branch name
    let repo_info_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let repo_info: RepoInfo = client
        .get(&repo_info_url)
        .header("Authorization", format!("token {}", github_token))
        .header("User-Agent", "reqwest")
        .send()?
        .json()?;

    let default_branch = repo_info.default_branch.ok_or("Default branch not found")?;

    // Step 2: Fetch the tree of the default branch with `recursive=1`
    let tree_url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
        owner, repo, default_branch
    );
    let tree_response: GitTreeResponse = client
        .get(&tree_url)
        .header("Authorization", format!("token {}", github_token))
        .header("User-Agent", "reqwest")
        .send()?
        .json()?;

    let tree_formatted = format_tree(tree_response.tree);
    if let Some(output_file) = args.output_file {
        let mut file = std::fs::File::create(output_file)?;
        write!(file, "{}", tree_formatted)?;
    } else {
        println!("{}", tree_formatted);
    }

    Ok(())
}

// Helper function to parse the repo name in the format "username/repo"
fn parse_repo_name(repo_name: &str) -> Result<(&str, &str), &'static str> {
    let parts: Vec<&str> = repo_name.split('/').collect();
    if parts.len() == 2 {
        Ok((parts[0], parts[1]))
    } else {
        Err("Repository name must be in the format 'username/repo'")
    }
}

// Struct for repository information to get the default branch
#[derive(Debug, Deserialize)]
struct RepoInfo {
    default_branch: Option<String>,
}

// Struct for the Git tree response
#[derive(Debug, Deserialize)]
pub struct GitTreeResponse {
    pub sha: String,          // SHA of the tree
    pub url: String,          // URL to access the tree
    pub truncated: bool,      // Whether the response was truncated
    pub tree: Vec<TreeEntry>, // Vector of TreeEntry objects representing the file structure
}

// Struct for each entry in the tree
#[derive(Debug, Deserialize)]
pub struct TreeEntry {
    pub path: String, // Path of the file in the tree
    pub mode: String, // Mode of the file (e.g., "040000" for directories)
    #[serde(rename = "type")]
    pub type_field: String, // Type of the entry ("tree" for folders, "blob" for files)
    pub sha: String,  // SHA of the entry
    pub size: Option<u64>, // Size of the entry (may be absent for folders)
    pub url: Option<String>, // URL to access the blob (for files only)
}

fn format_tree(tree: Vec<TreeEntry>) -> String {
    // fixed width columns, left is either DIR or FILE, right is the path
    let mut output = String::new();
    for entry in tree {
        let left = match entry.type_field.as_str() {
            "tree" => "DIR",
            "blob" => "FILE",
            _ => "UNK",
        };
        output.push_str(&format!("{:<} {}\n", left, entry.path));
    }
    output
}
