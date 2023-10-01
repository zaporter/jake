use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::process::Command;

pub fn edit_content<S:AsRef<str>>(initial_content: S) -> anyhow::Result<String> {
    // Create a temporary file.
    let mut temp_file = tempfile::NamedTempFile::new()?;

    // Write the initial content to the temp file.
    temp_file.write_all(initial_content.as_ref().as_bytes())?;

    // Retrieve the path of the temporary file.
    let temp_path = temp_file.path().to_path_buf();

    // Get the user's preferred editor.
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

    // Open the user's preferred editor with the temp file.
    let status = Command::new(editor)
        .arg(&temp_path)
        .status()?;

    if !status.success() {
        anyhow::bail!("Editor did not exit successfully");
    }

    // Read the content of the temp file after the user has made edits.
    let mut updated_content = String::new();
    File::open(temp_path.clone())?.read_to_string(&mut updated_content)?;

    // Cleanup: Remove the temp file.
    fs::remove_file(temp_path)?;

    Ok(updated_content.trim().to_string())
}
