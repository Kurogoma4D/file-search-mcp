use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, schemars, tool};
use std::fs;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{STORED, Schema, TextFieldIndexing, TextOptions, Value};
use tantivy::{Index, TantivyDocument, doc};

// Search parameters: directory path and search keyword
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "Path to the directory to search")]
    pub directory: String,
    #[schemars(description = "Keyword to search for")]
    pub keyword: String,
}

// Main tool struct
#[derive(Debug, Clone)]
pub struct SearchTool;

#[tool(tool_box)]
impl SearchTool {
    pub fn new() -> Self {
        Self {}
    }

    /// Perform full-text search for keywords on text files (such as .txt, .md, etc.) in the specified directory
    #[tool(description = "Search for keywords in text files within the specified directory")]
    async fn search(&self, #[tool(aggr)] params: SearchParams) -> Result<String, String> {
        // 1. Define schema for Tantivy (file paths and content)
        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", STORED);

        // Improve content field settings: explicitly set indexing options
        let text_indexing = TextFieldIndexing::default().set_tokenizer("default");
        let text_options = TextOptions::default()
            .set_indexing_options(text_indexing)
            .set_stored();
        let content_field = schema_builder.add_text_field("content", text_options);

        let schema = schema_builder.build();

        // 2. Create in-memory index
        let index = Index::create_in_ram(schema.clone());

        // 3. Create index writer (adjust buffer size as needed)
        let mut index_writer = index
            .writer(50_000_000)
            .map_err(|e| format!("Index writer error: {}", e))?;

        // Count the number of files added to the index
        let mut indexed_files_count = 0;
        // Track directory processing status (for debugging)
        let mut found_files_count = 0;
        let mut skipped_files_count = 0;

        // 4. Read text files in the specified directory and add them to the index
        let dir_path = Path::new(&params.directory);
        if !dir_path.is_dir() {
            return Err(format!(
                "The specified path '{}' is not a directory",
                params.directory
            ));
        }

        // Blacklist of extensions likely to be binary files
        // Skip extensions that are clearly binary files
        let binary_extensions = [
            "exe", "dll", "so", "dylib", "bin", "obj", "o", "a", "lib", "png", "jpg", "jpeg",
            "gif", "bmp", "tiff", "webp", "ico", "mp3", "mp4", "wav", "ogg", "flac", "avi", "mov",
            "mkv", "zip", "gz", "tar", "7z", "rar", "jar", "war", "pdf", "doc", "docx", "xls",
            "xlsx", "ppt", "pptx", "db", "sqlite", "mdb", "iso", "dmg", "class",
        ];

        // Function to determine if a file is a text file
        fn is_text_file(path: &Path, binary_extensions: &[&str]) -> bool {
            // 1. First check extensions that are clearly binary
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if binary_extensions.iter().any(|&bin_ext| bin_ext == ext_str) {
                    return false;
                }
            }

            // 2. Read the beginning of the file and determine if it is binary
            match fs::read(path) {
                Ok(bytes) if !bytes.is_empty() => {
                    // Sample size (read up to 8KB)
                    let sample_size = std::cmp::min(bytes.len(), 8192);
                    let sample = &bytes[..sample_size];

                    // Detect binary characteristics
                    // 1. Detect NULL bytes (text files do not have NULL bytes)
                    if sample.iter().any(|&b| b == 0) {
                        return false;
                    }

                    // 2. Check the ratio of control characters
                    let control_chars_count = sample
                        .iter()
                        .filter(|&&b| {
                            b < 32 && b != 9 && b != 10 && b != 13 // Exclude Tab, LF, CR
                        })
                        .count();

                    // If the ratio of control characters is too high, consider it binary
                    if (control_chars_count as f32 / sample_size as f32) > 0.3 {
                        return false;
                    }

                    // 3. Check if it is valid UTF-8
                    let is_valid_utf8 = std::str::from_utf8(sample).is_ok();

                    // 4. Check the ASCII ratio
                    let ascii_ratio =
                        sample.iter().filter(|&&b| b <= 127).count() as f32 / sample_size as f32;

                    // Valid UTF-8 with a high ASCII ratio, or specific non-UTF-8 encoding characteristics
                    is_valid_utf8 || ascii_ratio > 0.8
                }
                _ => false, // Do not consider files with read errors or size 0 as text
            }
        }

        // Function to recursively process directory entries
        fn process_directory(
            dir_path: &Path,
            index_writer: &mut tantivy::IndexWriter,
            path_field: tantivy::schema::Field,
            content_field: tantivy::schema::Field,
            binary_extensions: &[&str],
            indexed_files_count: &mut usize,
            found_files_count: &mut usize,
            skipped_files_count: &mut usize,
        ) -> Result<(), String> {
            for entry in fs::read_dir(dir_path)
                .map_err(|e| format!("Directory read error '{}': {}", dir_path.display(), e))?
            {
                let entry = entry.map_err(|e| format!("Entry read error: {}", e))?;
                let path = entry.path();

                if path.is_dir() {
                    // Recursively process subdirectories (add depth limit if needed)
                    process_directory(
                        &path,
                        index_writer,
                        path_field,
                        content_field,
                        binary_extensions,
                        indexed_files_count,
                        found_files_count,
                        skipped_files_count,
                    )?;
                } else if path.is_file() {
                    *found_files_count += 1;

                    // More universal text file determination
                    if is_text_file(&path, binary_extensions) {
                        match fs::read_to_string(&path) {
                            Ok(content) => {
                                if !content.trim().is_empty() {
                                    index_writer
                                        .add_document(doc!(
                                            path_field => path.to_string_lossy().to_string(),
                                            content_field => content,
                                        ))
                                        .map_err(|e| format!("Document addition error: {}", e))?;
                                    *indexed_files_count += 1;
                                    println!("Indexed: {}", path.display());
                                } else {
                                    *skipped_files_count += 1;
                                    println!("Skipped (empty file): {}", path.display());
                                }
                            }
                            Err(e) => {
                                // Skip and continue on read errors
                                *skipped_files_count += 1;
                                println!("Skipped (read error): {} - {}", path.display(), e);
                            }
                        }
                    } else {
                        *skipped_files_count += 1;
                        println!("Skipped (non-text): {}", path.display());
                    }
                }
            }
            Ok(())
        }

        // Execute directory processing
        println!("Target directory for search: {}", dir_path.display());
        process_directory(
            dir_path,
            &mut index_writer,
            path_field,
            content_field,
            &binary_extensions,
            &mut indexed_files_count,
            &mut found_files_count,
            &mut skipped_files_count,
        )?;

        println!(
            "Processing complete: Found files={}, Indexed={}, Skipped={}",
            found_files_count, indexed_files_count, skipped_files_count
        );

        // Return an error if no files were indexed
        if indexed_files_count == 0 {
            return Ok(format!(
                "No text files suitable for indexing were found in the specified directory '{}'.\nFound files: {}, Skipped: {}\nSupported extensions: {:?}",
                params.directory, found_files_count, skipped_files_count, binary_extensions
            ));
        }

        // 5. Commit the index
        index_writer
            .commit()
            .map_err(|e| format!("Commit error: {}", e))?;

        // 6. Generate reader and searcher for searching
        let reader = index.reader().map_err(|e| e.to_string())?;
        let searcher = reader.searcher();

        // 7. Parse query containing the keyword
        let query_parser = QueryParser::for_index(&index, vec![content_field]);

        // Ensure the keyword is not empty
        if params.keyword.trim().is_empty() {
            return Err("Search keyword is empty. Please enter a valid keyword.".into());
        }

        let query = query_parser
            .parse_query(&params.keyword)
            .map_err(|e| format!("Query parse error: {}", e))?;

        // 8. Retrieve top 10 search results
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(10))
            .map_err(|e| format!("Search error: {}", e))?;

        // 9. Concatenate file paths from search results into a string
        let mut result_str = String::new();
        for (score, doc_address) in &top_docs {
            let retrieved_doc: TantivyDocument =
                searcher.doc(*doc_address).map_err(|e| e.to_string())?;
            let path_value = retrieved_doc
                .get_first(path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown path");
            result_str.push_str(&format!("Hit: {} (Score: {:.2})\n", path_value, score));
        }

        if result_str.is_empty() {
            Ok(format!(
                "No search results for keyword '{}'. Number of indexed files: {}",
                params.keyword, indexed_files_count
            ))
        } else {
            Ok(format!(
                "Search results ({} hits):\n{}",
                top_docs.len(),
                result_str
            ))
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for SearchTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server searches for keywords in text files within the specified directory."
                    .into(),
            ),
        }
    }
}
