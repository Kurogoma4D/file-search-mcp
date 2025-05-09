# File Search MCP

A specialized Model Context Protocol (MCP) server for full-text search within a filesystem, built with Rust.

## 🔍 Overview

File Search MCP is a tool that provides powerful full-text search capabilities for text files in a specified directory. It uses the [Tantivy](https://github.com/quickwit-oss/tantivy) search engine to index and search through text content efficiently.

This project implements the Model Context Protocol (MCP), making it compatible with AI assistants and other systems that support the protocol.

## ✨ Features

- **Full-text search**: Search for keywords in text files across a directory structure
- **File content reader**: Read and display the content of specific text files
- **Smart file detection**: Automatically identifies text files and skips binary files
- **MCP integration**: Works with systems that support the Model Context Protocol
- **In-memory indexing**: Creates fast, temporary indexes for search operations
- **Score-based results**: Returns search hits with relevance scores

## 🛠️ Technology Stack

- **[Rust](https://www.rust-lang.org/)**: For performance, safety, and concurrency
- **[Tantivy](https://github.com/quickwit-oss/tantivy)**: A full-text search engine library in Rust
- **[RMCP](https://github.com/modelcontextprotocol/rust-sdk)**: Rust implementation of the Model Context Protocol
- **[Tokio](https://tokio.rs/)**: Asynchronous runtime for Rust

## 📋 Build and Usage

First, install Rust from [here](https://www.rust-lang.org/).

Clone this repository:
```bash
git clone git@github.com:Kurogoma4D/file-search-mcp.git
cd file-search-mcp
```

Build the project:
```bash
# Build in debug mode (creates binary in target/debug/)
cargo build

# Build in release mode for production (creates optimized binary in target/release/)
cargo build --release
```

Add the executable to your MCP settings (in Cursor, Claude, or other MCP clients):

- **Simple path reference**:
  - Debug build: `<path-to-repo>/target/debug/file-search-mcp`
  - Release build: `<path-to-repo>/target/release/file-search-mcp`

- **Claude Desktop/Claude Code configuration example**:
  ```json
  {
    "mcpServers": {
      "file-search-mcp": {
        "command": "<absolute-path-to-repo>/target/release/file-search-mcp",
        "args": [],
        "env": {}
      }
    }
  }
  ```

Replace `<path-to-repo>` or `<absolute-path-to-repo>` with the full path to your cloned repository.

**Platform-specific path examples**:
- macOS: `/Users/username/projects/file-search-mcp`
- Linux: `/home/username/projects/file-search-mcp`
- Windows: `C:\Users\username\projects\file-search-mcp`

## 🔄 How It Works

1. The server indexes text files in the specified directory, excluding binary files
2. It processes the content of text files and adds them to an in-memory Tantivy index
3. When a search is performed, it queries the index for matches and ranks them by relevance
4. Results are returned with file paths and relevance scores
5. The file content reader tool allows you to view the content of any text file by providing its path

## 🛠️ Available Tools

### Search Tool

- **Description**: Search for keywords in text files within a specified directory
- **Parameters**:
  - `directory`: Path to the directory to search
  - `keyword`: Keyword to search for

### File Content Reader Tool

- **Description**: Read and display the content of a specific file
- **Parameters**:
  - `file_path`: Path to the file to read

## 📄 License

MIT License

## 🙏 Acknowledgements

- [Tantivy](https://github.com/quickwit-oss/tantivy) for the full-text search engine
- [RMCP](https://github.com/modelcontextprotocol/rust-sdk) for the Model Context Protocol implementation
