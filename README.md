# File Search MCP

A specialized Model Context Protocol (MCP) server for full-text search within a filesystem, built with Rust.

## 🔍 Overview

File Search MCP is a tool that provides powerful full-text search capabilities for text files in a specified directory. It uses the [Tantivy](https://github.com/quickwit-oss/tantivy) search engine to index and search through text content efficiently.

This project implements the Model Context Protocol (MCP), making it compatible with AI assistants and other systems that support the protocol.

## ✨ Features

- **Full-text search**: Search for keywords in text files across a directory structure
- **Smart file detection**: Automatically identifies text files and skips binary files
- **MCP integration**: Works with systems that support the Model Context Protocol
- **In-memory indexing**: Creates fast, temporary indexes for search operations
- **Score-based results**: Returns search hits with relevance scores

## 🛠️ Technology Stack

- **[Rust](https://www.rust-lang.org/)**: For performance, safety, and concurrency
- **[Tantivy](https://github.com/quickwit-oss/tantivy)**: A full-text search engine library in Rust
- **[RMCP](https://crates.io/crates/rmcp)**: Rust implementation of the Model Context Protocol
- **[Tokio](https://tokio.rs/)**: Asynchronous runtime for Rust

## 📋 Usage

First, install Rust sdk from [here](https://www.rust-lang.org/).

Clone this repository.

```bash
git clone git@github.com:Kurogoma4D/file-search-mcp.git
```

And add this to your MCP settings (in Cursor, Claude, ...).

- command: `<path-to-repo>/target/release/file-search-mcp`

Replace `<path-to-repo>` to your cloned repository path.

## 🔄 How It Works

1. The server indexes text files in the specified directory, excluding binary files
2. It processes the content of text files and adds them to an in-memory Tantivy index
3. When a search is performed, it queries the index for matches and ranks them by relevance
4. Results are returned with file paths and relevance scores

## 📄 License

MIT License

## 🙏 Acknowledgements

- [Tantivy](https://github.com/quickwit-oss/tantivy) for the full-text search engine
- [RMCP](https://crates.io/crates/rmcp) for the Model Context Protocol implementation