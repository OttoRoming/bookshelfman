use bookshelfman::{forms::SearchSelect, storygraph::Storygraph};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storygraph = Storygraph::new().await?;
    let results = storygraph.search("the great gatsby").await?;

    let selected = SearchSelect::new("Pick a book", results)
     .with_thumbnail(|book| Some(Thumbnail::from_text(&book.cover)))
     .run()?;

    Ok(())
}
