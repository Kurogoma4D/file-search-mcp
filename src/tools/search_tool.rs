use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, schemars, tool};
use std::fs;
use std::path::Path;
use tantivy::schema::{STORED, Schema, TEXT, Value};
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

    /// 指定ディレクトリ内の .txt ファイルを対象に、キーワードで全文検索を行う
    #[tool(description = "指定ディレクトリ内のテキストファイルからキーワードを検索します")]
    async fn search(&self, #[tool(aggr)] params: SearchParams) -> Result<String, String> {
        // 1. Tantivy 用のスキーマを定義（ファイルパスと内容）
        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        let schema = schema_builder.build();

        // 2. インメモリインデックスを作成
        let index = Index::create_in_ram(schema.clone());

        // 3. インデックスライターの作成（バッファサイズは適宜調整）
        let mut index_writer = index
            .writer(50_000_000)
            .map_err(|e| format!("Index writer error: {}", e))?;

        // 4. 指定ディレクトリ内の .txt ファイルを読み込み、インデックスに追加
        let dir_path = Path::new(&params.directory);
        if !dir_path.is_dir() {
            return Err("指定されたパスはディレクトリではありません".into());
        }
        for entry in fs::read_dir(dir_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "txt" {
                        let content = fs::read_to_string(&path)
                            .map_err(|e| format!("ファイル読み込みエラー: {}", e))?;
                        index_writer
                            .add_document(doc!(
                                path_field => path.to_string_lossy().to_string(),
                                content_field => content,
                            ))
                            .map_err(|e| format!("ドキュメント追加エラー: {}", e))?;
                    }
                }
            }
        }

        // 5. インデックスをコミット
        index_writer
            .commit()
            .map_err(|e| format!("コミットエラー: {}", e))?;

        // 6. 検索のためにリーダーとサーチャーを生成
        let reader = index.reader().map_err(|e| e.to_string())?;
        let searcher = reader.searcher();

        // 7. キーワードを含むクエリをパース
        let query_parser = tantivy::query::QueryParser::for_index(&index, vec![content_field]);
        let query = query_parser
            .parse_query(&params.keyword)
            .map_err(|e| format!("クエリパースエラー: {}", e))?;

        // 8. 上位10件の検索結果を取得
        let top_docs = searcher
            .search(&query, &tantivy::collector::TopDocs::with_limit(10))
            .map_err(|e| format!("検索エラー: {}", e))?;

        // 9. 検索結果のファイルパスを文字列として連結
        let mut result_str = String::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument =
                searcher.doc(doc_address).map_err(|e| e.to_string())?;
            let path_value = retrieved_doc
                .get_first(path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown path");
            result_str.push_str(&format!("ヒット: {}\n", path_value));
        }

        if result_str.is_empty() {
            Ok("検索結果はありません。".into())
        } else {
            Ok(result_str)
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
