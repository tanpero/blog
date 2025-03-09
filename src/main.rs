use axum::{
    extract::Path,
    http::StatusCode,
    response::Html,
    routing::get,
    Router,
};
use pulldown_cmark::{Options, Parser};
use std::{
    collections::HashMap,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use std::env;
mod helper;


type ArticleStore = Arc<RwLock<HashMap<String, Article>>>;

#[derive(Debug, Clone)]
struct Article {
    content: String,
    file_path: PathBuf,
    last_modified: SystemTime,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化文章存储
    let article_store = init_article_store().await?;

    // 创建路由
    let app = Router::new()
        .route("/articles/{id}", get(article_handler))        
        .nest_service("/public", ServeDir::new("src/public"))
        .with_state(article_store);

    // 启动服务器
    let env = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());

    // 根据环境变量设置端口
    let port = match env.as_str() {
        "production" => 80,
        _ => 3000,
    };

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("Server running on http://localhost:{}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

// 初始化文章存储
async fn init_article_store() -> anyhow::Result<ArticleStore> {
    let mut articles = HashMap::new();
    let articles_dir = FsPath::new("articles");

    if articles_dir.is_dir() {
        for entry in std::fs::read_dir(articles_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    match process_article(&path).await {
                        Ok(article) => {
                            articles.insert(stem.to_string(), article);
                        }
                        Err(e) => eprintln!("Error processing {}: {}", path.display(), e),
                    }
                }
            }
        }
    }

    Ok(Arc::new(RwLock::new(articles)))
}

// 处理单个文章文件
async fn process_article(path: &PathBuf) -> anyhow::Result<Article> {
    let content = std::fs::read_to_string(path)?;
    let metadata = std::fs::metadata(path)?;
    let last_modified = metadata.modified()?;

    let html = markdown_to_html(&content);
    
    Ok(Article {
        content: html.await,
        file_path: path.clone(),
        last_modified,
    })
}

// Markdown转换HTML
async fn markdown_to_html(content: &str) -> String {
    let parser = Parser::new_ext(content,
        Options::ENABLE_MATH |
        Options::ENABLE_GFM |
        Options::ENABLE_STRIKETHROUGH |
        Options::ENABLE_SUBSCRIPT |
        Options::ENABLE_SUPERSCRIPT |
        Options::ENABLE_TABLES |
        Options::ENABLE_TASKLISTS |
        Options::ENABLE_FOOTNOTES
    );
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);

    let head = helper::read_file("src/head.html").await;

    let html = format!(
        r#"<!DOCTYPE html>
<html>
{}
<body>
<main class="container">
{}
</main>
</body>
</html>"#,
        head, html_output);
    html
}

// 文章请求处理
async fn article_handler(
    Path(id): Path<String>,
    state: axum::extract::State<ArticleStore>,
) -> Result<Html<String>, StatusCode> {
    let mut store = state.write().await;
    
    if let Some(article) = store.get_mut(&id) {

        // 检查文件是否被修改
        if let Ok(metadata) = tokio::fs::metadata(&article.file_path).await {
            if let Ok(current_modified) = metadata.modified() {
                if current_modified > article.last_modified {

                    // 重新加载文章
                    if let Ok(content) = tokio::fs::read_to_string(&article.file_path).await {
                        article.content = markdown_to_html(&content).await;
                        article.last_modified = current_modified;
                    }
                }
            }
        }
        
        return Ok(Html(article.content.clone()));
    }

    Err(StatusCode::NOT_FOUND)
}
