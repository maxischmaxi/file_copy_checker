#[derive(Debug)]
pub struct ReadFile {
  pub path: std::path::PathBuf,
  pub hash: String,
}