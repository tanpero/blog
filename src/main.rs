use axum::{
    extract::Path,
    http::StatusCode,
    response::Html,
    routing::get,
    Router,
};
use footnote::process_footnote;
use kuchiki::{parse_html, traits::*, NodeRef};
use pulldown_cmark::{Options, Parser};
use table_of_contents::enable_table_of_contents;
use std::{
    collections::HashMap,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::RwLock;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tower_http::services::ServeDir;
use std::env;
mod helper;
mod table_of_contents;
mod footnote;

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
    
    let article_store = init_article_store().await?;

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/articles", get(index_handler))
        .route("/articles/", get(index_handler))
        .route("/articles/{id}", get(article_handler))        
        .nest_service("/public", ServeDir::new("src/public"))
        .fallback(fallback_handler)
        .with_state(article_store);

    let env = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());

    let port = match env.as_str() {
        "production" => 80,
        _ => 3000,
    };

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("Server running on http://localhost:{}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root_handler() -> Result<Html<String>, StatusCode> {
    let html = helper::read_file("src/public/index.html").await;
    Ok(Html(html))
}

async fn index_handler(
    state: axum::extract::State<ArticleStore>,
) -> Result<Html<String>, StatusCode> {
    let mut store = state.write().await;
    sync_articles_with_filesystem(&mut store).await; // 同步文章存储与文件系统

    let mut articles: Vec<&Article> = store.values().collect();
    articles.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let mut html = String::from("<h1>Articles</h1>");

    for article in articles {
        let (english_time, chinese_time) = helper::format_system_time(article.created_at);

        html.push_str(&format!(
            r#"<div class="card">
                <h2><a href="/articles/{}">{}</a></h2>
                <div class="time-container"><span>{}</span> <span>{}</span></div>
            </div>"#,
            article.file_path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
            article.title,
            english_time,
            chinese_time
        ));
    }

    let head = helper::read_file("src/head.html").await;
    let all_html = format!(
        r#"<!DOCTYPE html>
<html>
{}
<body>
<main class="container">
{}</main>
</body>
</html>"#,
        head, html
    );

    Ok(Html(all_html))
}


async fn fallback_handler() -> Result<Html<String>, StatusCode> {

    let all_html = helper::read_file("src/public/404.html").await;

    Ok(Html(all_html))
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

    let html = generate_page(&content);
    
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

async fn generate_page(source: &String) -> String {
    let head = helper::read_file("src/head.html").await;
    let main = markdown_to_html(&source).await;
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
         head, main);
    
    post_process_html(html)
}

fn post_process_html(origin_html: String) -> String {

    let document = parse_html().one(origin_html);

    let with_toc = enable_table_of_contents(&document);
    let with_footnote = process_footnote(&with_toc);
    let final_document = with_footnote;
    let final_html = final_document.to_string();

    final_html
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
        Options::ENABLE_FOOTNOTES |
        Options::ENABLE_WIKILINKS
    );
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);

    html_output
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
                        article.content = generate_page(&content).await;
                        article.last_modified = current_modified;
                    }
                }
            }        
        }
        
        return Ok(Html(article.content.clone()));
    }

    Err(StatusCode::NOT_FOUND)
}


// 同步文章存储与文件系统
async fn sync_articles_with_filesystem(store: &mut HashMap<String, Article>) {
    let articles_dir = FsPath::new("articles");

    if articles_dir.is_dir() {
        // 检查现有文章
        let existing_ids: Vec<String> = store.keys().cloned().collect();
        for id in existing_ids {
            if let Some(article) = store.get_mut(&id) {
                // 检查文件是否还存在
                if !article.file_path.exists() {
                    store.remove(&id);
                    continue;
                }

                // 检查文件是否被修改
                if let Ok(metadata) = tokio::fs::metadata(&article.file_path).await {
                    if let Ok(current_modified) = metadata.modified() {
                        if current_modified > article.last_modified {
                            if let Ok(content) = tokio::fs::read_to_string(&article.file_path).await {
                                // 更新标题
                                let title = extract_title(&content).await;
                                article.title = title;

                                // 更新内容
                                let html = generate_page(&content).await;
                                article.content = html;

                                // 更新修改时间
                                article.last_modified = current_modified;
                            }
                        }
                    }
                }
            }
        }

        // 检查是否有新文件
        if let Ok(mut entries) = tokio::fs::read_dir(articles_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !store.contains_key(stem) {
                            // 处理新文件
                            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                                let title = extract_title(&content).await;
                                let html = generate_page(&content).await;
                                let metadata = tokio::fs::metadata(&path).await.unwrap();
                                let last_modified = metadata.modified().unwrap();
                                let created_at = metadata.created().unwrap();

                                store.insert(
                                    stem.to_string(),
                                    Article {
                                        title,
                                        content: html,
                                        file_path: path.clone(),
                                        last_modified,
                                        created_at,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
