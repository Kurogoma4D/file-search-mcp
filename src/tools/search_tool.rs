use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, schemars, tool};
use std::fs;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{STORED, Schema, TextFieldIndexing, TextOptions, Value};
use tantivy::{Index, TantivyDocument, doc};

// 検索パラメータ：ディレクトリのパスと検索キーワード
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "検索対象ディレクトリのパス")]
    pub directory: String,
    #[schemars(description = "検索するキーワード")]
    pub keyword: String,
}

// ツール本体の構造体
#[derive(Debug, Clone)]
pub struct SearchTool;

#[tool(tool_box)]
impl SearchTool {
    pub fn new() -> Self {
        Self {}
    }

    /// 指定ディレクトリ内のテキストファイル（.txt, .md など）を対象に、キーワードで全文検索を行う
    #[tool(description = "指定ディレクトリ内のテキストファイルからキーワードを検索します")]
    async fn search(&self, #[tool(aggr)] params: SearchParams) -> Result<String, String> {
        // 1. Tantivy 用のスキーマを定義（ファイルパスと内容）
        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", STORED);

        // コンテンツフィールドの設定を改善: インデックスオプションを明示的に設定
        let text_indexing = TextFieldIndexing::default().set_tokenizer("default");
        let text_options = TextOptions::default()
            .set_indexing_options(text_indexing)
            .set_stored();
        let content_field = schema_builder.add_text_field("content", text_options);

        let schema = schema_builder.build();

        // 2. インメモリインデックスを作成
        let index = Index::create_in_ram(schema.clone());

        // 3. インデックスライターの作成（バッファサイズは適宜調整）
        let mut index_writer = index
            .writer(50_000_000)
            .map_err(|e| format!("Index writer error: {}", e))?;

        // インデックス追加されたファイルの数をカウント
        let mut indexed_files_count = 0;
        // ディレクトリの処理状況を追跡（デバッグ用）
        let mut found_files_count = 0;
        let mut skipped_files_count = 0;

        // 4. 指定ディレクトリ内のテキストファイルを読み込み、インデックスに追加
        let dir_path = Path::new(&params.directory);
        if !dir_path.is_dir() {
            return Err(format!(
                "指定されたパス '{}' はディレクトリではありません",
                params.directory
            ));
        }

        // バイナリファイルの可能性が高い拡張子のブラックリスト
        // 明らかにバイナリファイルの拡張子はスキップする
        let binary_extensions = [
            "exe", "dll", "so", "dylib", "bin", "obj", "o", "a", "lib", "png", "jpg", "jpeg",
            "gif", "bmp", "tiff", "webp", "ico", "mp3", "mp4", "wav", "ogg", "flac", "avi", "mov",
            "mkv", "zip", "gz", "tar", "7z", "rar", "jar", "war", "pdf", "doc", "docx", "xls",
            "xlsx", "ppt", "pptx", "db", "sqlite", "mdb", "iso", "dmg", "class",
        ];

        // テキストファイルを判定する関数
        fn is_text_file(path: &Path, binary_extensions: &[&str]) -> bool {
            // 1. まず明らかにバイナリと思われる拡張子をチェック
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if binary_extensions.iter().any(|&bin_ext| bin_ext == ext_str) {
                    return false;
                }
            }

            // 2. ファイルの先頭部分を読み取り、バイナリかどうかを判定
            match fs::read(path) {
                Ok(bytes) if !bytes.is_empty() => {
                    // サンプルサイズ（最大8KBまで読む）
                    let sample_size = std::cmp::min(bytes.len(), 8192);
                    let sample = &bytes[..sample_size];

                    // バイナリ特性の検出
                    // 1. NULLバイトの検出（テキストファイルにはNULLバイトがない）
                    if sample.iter().any(|&b| b == 0) {
                        return false;
                    }

                    // 2. 制御文字の比率をチェック
                    let control_chars_count = sample
                        .iter()
                        .filter(|&&b| {
                            b < 32 && b != 9 && b != 10 && b != 13 // Tab, LF, CRを除く制御文字
                        })
                        .count();

                    // 制御文字の比率が高すぎる場合はバイナリと判断
                    if (control_chars_count as f32 / sample_size as f32) > 0.3 {
                        return false;
                    }

                    // 3. UTF-8として有効かチェック
                    let is_valid_utf8 = std::str::from_utf8(sample).is_ok();

                    // 4. ASCII比率のチェック
                    let ascii_ratio =
                        sample.iter().filter(|&&b| b <= 127).count() as f32 / sample_size as f32;

                    // 有効なUTF-8で高いASCII比率、または特定のUTF-8以外のエンコーディング特性がある場合
                    is_valid_utf8 || ascii_ratio > 0.8
                }
                _ => false, // 読み取りエラーやサイズ0のファイルの場合はテキストとみなさない
            }
        }

        // ディレクトリのエントリを再帰的に処理する関数
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
            for entry in fs::read_dir(dir_path).map_err(|e| {
                format!("ディレクトリ読み込みエラー '{}': {}", dir_path.display(), e)
            })? {
                let entry = entry.map_err(|e| format!("エントリ読み込みエラー: {}", e))?;
                let path = entry.path();

                if path.is_dir() {
                    // 再帰的にサブディレクトリを処理（必要に応じて深さ制限を追加）
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

                    // より普遍的なテキストファイル判定
                    if is_text_file(&path, binary_extensions) {
                        match fs::read_to_string(&path) {
                            Ok(content) => {
                                if !content.trim().is_empty() {
                                    index_writer
                                        .add_document(doc!(
                                            path_field => path.to_string_lossy().to_string(),
                                            content_field => content,
                                        ))
                                        .map_err(|e| format!("ドキュメント追加エラー: {}", e))?;
                                    *indexed_files_count += 1;
                                    println!("インデックス化: {}", path.display());
                                } else {
                                    *skipped_files_count += 1;
                                    println!("スキップ (空ファイル): {}", path.display());
                                }
                            }
                            Err(e) => {
                                // 読み込みエラーはスキップして続行
                                *skipped_files_count += 1;
                                println!("スキップ (読込エラー): {} - {}", path.display(), e);
                            }
                        }
                    } else {
                        *skipped_files_count += 1;
                        println!("スキップ (非テキスト): {}", path.display());
                    }
                }
            }
            Ok(())
        }

        // ディレクトリ処理の実行
        println!("検索対象ディレクトリ: {}", dir_path.display());
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
            "処理完了: 検出ファイル数={}, インデックス化={}, スキップ={}",
            found_files_count, indexed_files_count, skipped_files_count
        );

        // インデックス化されたファイルが0の場合はエラーを返す
        if indexed_files_count == 0 {
            return Ok(format!(
                "指定されたディレクトリ '{}' にはインデックス化できるテキストファイルが見つかりませんでした。\n検出ファイル数: {}, スキップ: {}\n対応拡張子: {:?}",
                params.directory, found_files_count, skipped_files_count, binary_extensions
            ));
        }

        // 5. インデックスをコミット
        index_writer
            .commit()
            .map_err(|e| format!("コミットエラー: {}", e))?;

        // 6. 検索のためにリーダーとサーチャーを生成
        let reader = index.reader().map_err(|e| e.to_string())?;
        let searcher = reader.searcher();

        // 7. キーワードを含むクエリをパース
        let query_parser = QueryParser::for_index(&index, vec![content_field]);

        // キーワードが空でないことを確認
        if params.keyword.trim().is_empty() {
            return Err("検索キーワードが空です。有効なキーワードを入力してください。".into());
        }

        let query = query_parser
            .parse_query(&params.keyword)
            .map_err(|e| format!("クエリパースエラー: {}", e))?;

        // 8. 上位10件の検索結果を取得
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(10))
            .map_err(|e| format!("検索エラー: {}", e))?;

        // 9. 検索結果のファイルパスを文字列として連結
        let mut result_str = String::new();
        for (score, doc_address) in &top_docs {
            let retrieved_doc: TantivyDocument =
                searcher.doc(*doc_address).map_err(|e| e.to_string())?;
            let path_value = retrieved_doc
                .get_first(path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown path");
            result_str.push_str(&format!("ヒット: {} (スコア: {:.2})\n", path_value, score));
        }

        if result_str.is_empty() {
            Ok(format!(
                "キーワード '{}' の検索結果はありません。インデックス化されたファイル数: {}",
                params.keyword, indexed_files_count
            ))
        } else {
            Ok(format!("検索結果 ({}件):\n{}", top_docs.len(), result_str))
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
            instructions: Some("このサーバーは、指定されたディレクトリ内のテキストファイルからキーワードを検索します。".into()),
        }
    }
}
