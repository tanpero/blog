use axum::{
    extract::Path,
    http::StatusCode,
    response::Html,
    routing::get,
    Router,
};
use chrono::{DateTime, Local};
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
    title: String,
    content: String,
    file_path: PathBuf,
    last_modified: SystemTime,
    created_at: SystemTime,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化文章存储
    let article_store = init_article_store().await?;

    // 创建路由
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/articles/{id}", get(article_handler))        
        .nest_service("/public", ServeDir::new("src/public"))
        .fallback(fallback_handler)
        .with_state(article_store);

    // 启动服务器
    let env = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());

    // 根据环境变量设置端口
    let port = match env.as_str() {
        "production" => 80,
        _ => 3000,
    };

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("Server running on http://localhost:{}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler(
    state: axum::extract::State<ArticleStore>,
) -> Result<Html<String>, StatusCode> {
    let store = state.read().await;
    // 按创建时间排序
    let mut articles: Vec<&Article> = store.values().collect();
    articles.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    // 生成 HTML
    let mut html = String::from("<h1>博客文章</h1>");
    for article in articles {
        html.push_str(&format!(
            r#"<div class="card">
                <h2><a href="/articles/{}">{}</a></h2>
                <p>创建时间: {}</p>
            </div>"#,
            article.file_path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
            article.title,
            format_system_time(article.created_at)
        ));
    }
    Ok(Html(html))
}

// 新增：格式化 SystemTime 为可读字符串
fn format_system_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

async fn fallback_handler() -> Html<&'static str> {
    Html(r#"
        <h1>404 - 页面未找到</h1>
        <p>抱歉，您访问的页面不存在。</p>
        <p><a href="/">返回首页</a></p>
    "#)
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
    let created_at = metadata.created()?; // 新增：获取创建时间
    let title = extract_title(&content); // 新增：提取标题

    let html = markdown_to_html(&content);
    
    Ok(Article {
        title: title.await,
        content: html.await,
        file_path: path.clone(),
        last_modified,
        created_at,
    })
}

async fn extract_title(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("");
    let title = first_line.trim_start_matches('#').trim();
    markdown_to_html(title).await
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
                        article.title = extract_title(&content).await; // 更新标题
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
