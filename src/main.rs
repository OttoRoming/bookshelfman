use bookshelfman::storygraph::Storygraph;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storygraph = Storygraph::new().await?;
    let results = storygraph.search("the great gatsby").await?;

    for book in results {
        println!("book: {book:?}");
    }

    Ok(())
}
